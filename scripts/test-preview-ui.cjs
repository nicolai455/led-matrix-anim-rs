const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const { test } = require('node:test');
const html = fs.readFileSync(new URL('../crates/matrix-control/src/web_ui.rs', `file://${__filename}`), 'utf8');
const script = html.match(/<script>([\s\S]*?)<\/script>/)[1];
function harness() {
  const requests = [], timers = [], paints = [];
  const elements = new Map();
  const context = {
    createImageData: () => ({ data: new Uint8ClampedArray(4096) }),
    putImageData: (...args) => paints.push(args),
  };
  const element = id => {
    if (!elements.has(id)) elements.set(id, {
      value: id === 'previewMode' ? 'browser' : '', textContent: '', style: {},
      classList: { add() {}, remove() {}, toggle() {} },
      dataset: {},
      getContext: () => context, appendChild() {}, addEventListener() {},
    });
    return elements.get(id);
  };
  const sandbox = {
    document: { hidden: false, getElementById: element, createElement: () => element(Symbol()), activeElement: null },
    window: { addEventListener() {} }, TextEncoder, Uint8Array, Uint8ClampedArray,
    performance: { now: () => 100 }, WebAssembly,
    requestAnimationFrame: fn => { timers.push(fn); return timers.length; },
    setTimeout: fn => { timers.push(fn); return timers.length; }, clearTimeout() {},
    fetch: async url => { requests.push(url); return { ok: true, arrayBuffer: async () => new ArrayBuffer(3072) }; },
    console,
  };
  vm.createContext(sandbox);
  vm.runInContext(script, sandbox);
  return { sandbox, requests, timers, paints, element, run: code => vm.runInContext(code, sandbox) };
}
test('preview grows to 448px while fitting narrow screens', () => {
  assert.match(html, /width:min\(100%,\s*448px\)/);
  assert.match(html, /aspect-ratio:1/);
});
test('browser mode never fetches device frames; exact mode remains available', async () => {
  const h = harness();
  await h.run('pollPreview()');
  assert.deepEqual(h.requests, []);
  h.element('previewMode').value = 'device';
  await h.run('pollPreview()');
  assert.deepEqual(h.requests, ['/api/frame']);
  assert.equal(h.paints.length, 1);
});
test('hidden and disabled previews make no frame requests', async () => {
  const h = harness();
  h.element('previewMode').value = 'off';
  await h.run('pollPreview()');
  h.element('previewMode').value = 'device';
  h.sandbox.document.hidden = true;
  await h.run('pollPreview()');
  assert.deepEqual(h.requests, []);
});
test('browser rendering uses reusable canvas image and WASM, not HTTP', () => {
  const h = harness();
  h.run(`previewRenderer = {
    memory: new WebAssembly.Memory({initial: 1}),
    preview_input: () => 8192, preview_input_capacity: () => 16384,
    preview_sync: () => 0, preview_render: () => 0
  }; state = {on:true, renderTimeMs:5000}; syncPreviewState(); animatePreview(100); animatePreview(117);`);
  assert.equal(h.paints.length, 2);
  assert.equal(h.paints[0][0], h.paints[1][0]);
  assert.deepEqual(h.requests, []);
});
test('hidden tabs skip state polling and local paint', async () => {
  const h = harness();
  h.sandbox.document.hidden = true;
  await h.run('loop()');
  h.run('animatePreview(100)');
  assert.deepEqual(h.requests, []);
  assert.equal(h.paints.length, 0);
});
test('external audio requests compact telemetry only in visible browser mode', async () => {
  const h = harness();
  h.sandbox.fetch = async url => { h.requests.push(url); return {ok:true, json:async()=>({level:0.7})}; };
  h.run("state = {audio_source:'Mic'}");
  await h.run('audioPreviewLoop()');
  assert.deepEqual(h.requests, ['/api/audio']);
  assert.equal(h.run('state.level'), 0.7);
  h.sandbox.document.hidden = true;
  await h.run('audioPreviewLoop()');
  h.sandbox.document.hidden = false;
  h.element('previewMode').value = 'off';
  await h.run('audioPreviewLoop()');
  assert.deepEqual(h.requests, ['/api/audio']);
});
test('WASM failure stays local instead of silently adding ESP traffic', async () => {
  const h = harness();
  h.sandbox.fetch = async url => { h.requests.push(url); throw new Error('offline'); };
  await h.run('loadPreviewRenderer()');
  await h.run('pollPreview()');
  assert.deepEqual(h.requests, ['/preview.wasm.gz']);
  assert.match(h.element('previewStatus').textContent, /unavailable/);
});
test('render clock never moves backwards when telemetry arrives late', () => {
  const h = harness();
  const times = [];
  h.sandbox.renderTick = t => { times.push(t); return 0; };
  h.run(`previewRenderer = {
    memory: new WebAssembly.Memory({initial: 1}), preview_input: () => 8192,
    preview_input_capacity: () => 16384, preview_sync: () => 0, preview_render: renderTick
  }; state = {renderTimeMs:5000}; syncPreviewState(); animatePreview(200);
  state.renderTimeMs = 4900; syncPreviewState(); animatePreview(210);`);
  assert.equal(times.length, 2);
  assert.ok(times[1] >= times[0]);
});
test('unsupported local effects use explicit device fallback', async () => {
  const h = harness();
  h.run(`previewRenderer = {
    memory: new WebAssembly.Memory({initial: 1}), preview_input: () => 8192,
    preview_input_capacity: () => 16384, preview_sync: () => 2
  }; state = {on:true}; syncPreviewState();`);
  await h.run('pollPreview()');
  assert.deepEqual(h.requests, ['/api/frame']);
  assert.match(h.element('previewStatus').textContent, /device/i);
});
test('effect parameter labels render correctly from label property and never as undefined', () => {
  const h = harness();
  const created = [];
  h.sandbox.document.createElement = tag => {
    const el = {
      tagName: tag,
      style: {},
      dataset: {},
      children: [],
      classList: { add() {}, remove() {}, toggle() {} },
      appendChild(child) { this.children.push(child); },
      querySelector: sel => ({ style: {}, oninput: null, onchange: null }),
      querySelectorAll: () => [],
    };
    created.push(el);
    return el;
  };
  h.run(`
    catalog = [{
      id: 'test_fx',
      name: 'Test FX',
      parameters: [
        { id: 'spd', label: 'Speed', kind: 'number', min: 0, max: 100, step: 1, default: 50 },
        { id: 'opt', label: 'Palette', kind: 'select', options: ['A', 'B'], default: 0 },
        { id: 'chk', label: 'Audio Reactive', kind: 'boolean', default: 0 }
      ]
    }];
    state = { effect: 'Test FX', effects: {} };
    renderParameters();
  `);
  const paramLabels = created.filter(e => e.className === 'param-label');
  assert.equal(paramLabels.length, 3);
  assert.ok(paramLabels[0].innerHTML.includes('Speed'));
  assert.ok(paramLabels[1].innerHTML.includes('Palette'));
  assert.ok(paramLabels[2].innerHTML.includes('Audio Reactive'));
  for (const pl of paramLabels) {
    assert.ok(!pl.innerHTML.includes('undefined'), 'param label contains undefined');
  }
});
test('synthetic audio enables listen button and wasm updates visualizer bands', () => {
  const h = harness();
  const bandsEl = [];
  for (let i = 0; i < 16; i++) {
    bandsEl.push({ style: { height: '0%' } });
  }
  h.sandbox.document.querySelectorAll = sel => sel === '.band-col' ? bandsEl : [];
  const fakeBands = new Float32Array(16);
  fakeBands[0] = 0.95;
  fakeBands[15] = 0.42;

  h.run(`
    previewRenderer = {
      memory: new WebAssembly.Memory({initial: 1}),
      preview_input: () => 8192,
      preview_input_capacity: () => 16384,
      preview_sync: () => 0,
      preview_render: () => 0,
      preview_audio_bands: () => 1024,
      preview_audio_pcm: () => 2048,
    };
    new Float32Array(previewRenderer.memory.buffer, 1024, 16).set([0.95, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.42]);
    state = { audio_source: 'Synthetic', on: true };
    syncPreviewState();
    updateUI();
  `);
  assert.equal(h.element('audioSoundBtn').style.display, 'inline-block');

  h.run(`animatePreview(100);`);
  assert.equal(bandsEl[0].style.height, '95.0%');
  assert.equal(bandsEl[15].style.height, '42.0%');

  h.run(`
    state.audio_source = 'Off';
    updateUI();
  `);
  assert.equal(h.element('audioSoundBtn').style.display, 'none');
});

test('first sync survives WASM memory growth during input initialization', () => {
  const h = harness();
  h.run(`previewRenderer = { memory: new WebAssembly.Memory({initial: 1}),
    preview_input_capacity: () => 16384,
    preview_input() { this.memory.grow(1); return 8192; }, preview_sync: () => 0 };
    state = {renderTimeMs: 0}; syncPreviewState();`);
  assert.equal(h.run('previewReady'), true);
});

test('config UI distinguishes pending and persisted-but-failed power updates', async () => {
  for (const [ok, response, expected] of [
    [true, {powerLimitApply: 'pending'}, /pending/i],
    [false, {powerLimitApply: 'failed', error: 'queue full'}, /Saved, but power limit not applied/i],
  ]) {
    const h = harness();
    h.sandbox.fetch = async () => ({ok, json: async () => response});
    await h.run('savePanelConfig()');
    assert.match(h.element('cfgStatusMsg').textContent, expected);
  }
});
