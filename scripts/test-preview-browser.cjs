// Integration smoke test against a running simulator and headless Chromium CDP.
const assert = require('node:assert/strict');
const base = process.env.MATRIX_URL || 'http://127.0.0.1:18080';
const debug = process.env.CHROME_DEBUG_URL || 'http://127.0.0.1:19223';
const sleep = ms => new Promise(r => setTimeout(r, ms));
(async () => {
  const tabs = await (await fetch(debug + '/json/list')).json();
  const socket = new WebSocket(tabs.find(t => t.type === 'page').webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  let id = 0;
  const pending = new Map(), requests = [], errors = [];
  socket.onmessage = ({ data }) => {
    const msg = JSON.parse(data);
    if (msg.id) {
      const promise = pending.get(msg.id);
      if (promise) { pending.delete(msg.id); msg.error ? promise.reject(msg.error) : promise.resolve(msg.result); }
    }
    if (msg.method === 'Network.requestWillBeSent') requests.push(msg.params.request.url);
    if (msg.method === 'Runtime.exceptionThrown') errors.push(msg.params.exceptionDetails);
  };
  const call = (method, params = {}) => new Promise((resolve, reject) => {
    pending.set(++id, { resolve, reject }); socket.send(JSON.stringify({ id, method, params }));
  });
  const evaluate = async expression => {
    const response = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (response.exceptionDetails) throw new Error(JSON.stringify(response.exceptionDetails));
    return response.result.value;
  };
  try {
    await call('Network.enable'); await call('Runtime.enable');
    await call('Page.navigate', { url: base });
    let ready = false;
    for (let i = 0; i < 100; i++) {
      await sleep(100);
      ready = await evaluate("typeof previewReady !== 'undefined' && previewReady");
      if (ready) break;
    }
    assert.ok(ready, 'browser renderer initialized');
    const command = async (kind, payload) => {
      const r = await fetch(base + '/api/command', {method:'POST', headers:{'Content-Type':'application/json'}, body:JSON.stringify({kind,payload})});
      assert.ok(r.ok, await r.text());
    };
    await command('light', { effect:'Plasma', state:'ON' });
    await sleep(1200);
    const pixels1 = await evaluate("Array.from(previewCtx.getImageData(0,0,32,32).data)");
    await sleep(150);
    const pixels2 = await evaluate("Array.from(previewCtx.getImageData(0,0,32,32).data)");
    assert.notDeepEqual(pixels1, pixels2, 'local effect animates between state polls');
    assert.ok(pixels2.some((v,i) => i%4 !== 3 && v > 0), 'preview is lit');
    assert.equal(requests.filter(u => u.endsWith('/api/frame')).length, 0, 'browser mode sends zero frame requests');
    await command('light', { state:'OFF' });
    await sleep(700);
    const dark = await evaluate("Array.from(previewCtx.getImageData(0,0,32,32).data)");
    assert.ok(dark.every((v,i) => i%4 === 3 || v === 0), 'power-off preview is black');
    await command('audio_source', 'Mic');
    await sleep(700);
    assert.ok(requests.some(u => u.endsWith('/api/audio')), 'microphone uses compact audio telemetry');
    assert.equal(requests.filter(u => u.endsWith('/api/frame')).length, 0);
    await command('audio_source', 'Off');
    const desktop = await evaluate("document.getElementById('previewCanvas').getBoundingClientRect().width");
    assert.equal(desktop, 448);
    await call('Emulation.setDeviceMetricsOverride', {width:375,height:812,deviceScaleFactor:1,mobile:true});
    const mobile = await evaluate("({canvas:document.getElementById('previewCanvas').getBoundingClientRect().width, scroll:document.documentElement.scrollWidth, viewport:innerWidth})");
    assert.ok(mobile.canvas <= 375);
    assert.ok(mobile.scroll <= mobile.viewport, 'no horizontal overflow');
    await evaluate("document.getElementById('previewMode').value='device'; changePreviewMode()");
    await sleep(650);
    assert.ok(requests.some(u => u.endsWith('/api/frame')), 'device mode fetches frames');
    await evaluate("document.getElementById('previewMode').value='off'; changePreviewMode()");
    await sleep(300);
    const count = requests.length;
    await sleep(650);
    assert.equal(requests.slice(count).filter(u => u.endsWith('/api/frame')).length, 0);
    assert.deepEqual(errors, []);
    console.log(JSON.stringify({browser:'Chromium', desktopWidth:desktop, mobile, wasmRequests:requests.filter(u=>u.endsWith('/preview.wasm.gz')).length, browserFrameRequests:0, animationVerified:true, deviceAndOffVerified:true, exceptions:errors.length},null,2));
  } finally { socket.close(); }
})().catch(e => { console.error(e); process.exitCode = 1; });
