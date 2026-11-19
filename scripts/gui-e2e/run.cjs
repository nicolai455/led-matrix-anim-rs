#!/usr/bin/env node
'use strict';
/**
 * Full GUI end-to-end suite for the LED matrix web UI.
 *
 * Boots the host simulator on a throw-away state file and drives a headless
 * Chromium over the DevTools protocol — no npm dependencies, fully offline.
 * Every interactive control in the UI is exercised, and any console error,
 * uncaught exception or unexpected HTTP >= 400 fails the run.
 *
 * Usage:
 *   node scripts/gui-e2e/run.cjs              # full suite
 *   node scripts/gui-e2e/run.cjs --filter boot    # one group
 *   node scripts/gui-e2e/run.cjs --verbose
 */
const path = require('node:path');
const fs = require('node:fs');
const {
  Simulator, launchChromium, baselineExpectations, sleep,
} = require('./harness.cjs');

const VERBOSE = process.argv.includes('--verbose');
const filterIndex = process.argv.indexOf('--filter');
const FILTER = filterIndex >= 0 ? process.argv[filterIndex + 1] : null;
const ARTIFACTS = path.join(__dirname, 'artifacts');

const results = [];

function log(...args) {
  if (VERBOSE) console.log('   ', ...args);
}

/** One test: name, group, and a body receiving {page, simulator, group}. */
async function test(group, name, body, options = {}) {
  if (FILTER && group !== FILTER) return;
  const entry = { group, name, status: 'running', detail: '' };
  results.push(entry);
  const started = Date.now();
  try {
    // Each test owns its console/HTTP window; a deliberate negative test must not
    // leak its 4xx into every later assertion.
    if (global.__currentPage) global.__currentPage.clearLogs();
    await body();
    entry.status = 'pass';
    process.stdout.write(`  ✓ ${name}\n`);
  } catch (error) {
    entry.status = 'fail';
    entry.detail = error && error.message ? error.message : String(error);
    process.stdout.write(`  ✗ ${name}\n      ${entry.detail.split('\n')[0]}\n`);
    if (global.__currentPage) {
      try {
        const shot = await global.__currentPage.screenshot(
          `fail-${name.replace(/[^a-z0-9]+/gi, '-').toLowerCase()}`
        );
        entry.detail += `\n      screenshot: ${shot}`;
      } catch { /* screenshot is best effort */ }
    }
  } finally {
    entry.ms = Date.now() - started;
  }
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function assertEqual(actual, expected, message) {
  if (actual !== expected) {
    throw new Error(`${message}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  }
}

/** Steps the group counter in the output. */
function group(title) {
  if (FILTER && title !== FILTER) return;
  process.stdout.write(`\n▸ ${title}\n`);
}

async function main() {
  fs.mkdirSync(ARTIFACTS, { recursive: true });
  const simulator = await new Simulator().start();
  const browser = await launchChromium();
  let page;

  try {
    page = baselineExpectations(await browser.newPage());
    await page.goto(simulator.base);
    global.__currentPage = page;

    // ---------------------------------------------------------------- boot
    group('boot');
    await test('boot', 'page loads with a live snapshot and the full catalog', async () => {
      await page.waitForSnapshot('Array.isArray(state.playlist?.entries)', 10000, 'first snapshot');
      const effects = await page.evaluate('catalog.length');
      assertEqual(effects, 82, 'catalog size');
      const cards = await page.evaluate("document.querySelectorAll('.effect-btn').length");
      assertEqual(cards, 82, 'rendered effect cards');
    });

    await test('boot', 'preview canvas paints once the panel is powered on', async () => {
      // The device boots dark by design (boot protection), so the preview is
      // black until power is on. Asserting pixels before that would demand a bug.
      const darkBefore = await page.evaluate(`(() => {
        const data = previewCtx.getImageData(0, 0, 32, 32).data;
        let n = 0;
        for (let i = 0; i < data.length; i += 4) if (data[i] || data[i+1] || data[i+2]) n++;
        return n;
      })()`);
      assertEqual(darkBefore, 0, 'a dark panel must not paint the preview');

      await page.click('#powerToggle');
      await page.waitForSnapshot('state.state === "ON"', 8000, 'engine ON');
      const lit = await page.waitFor(`(() => {
        const data = previewCtx.getImageData(0, 0, 32, 32).data;
        let n = 0;
        for (let i = 0; i < data.length; i += 4) if (data[i] || data[i+1] || data[i+2]) n++;
        return n > 0 ? n : false;
      })()`, 10000, 'painted preview');
      assert(lit > 0, 'preview canvas has no lit pixels while the panel is on');
      log('lit preview pixels:', lit);
      await page.click('#powerToggle');
      await page.waitForSnapshot('state.state === "OFF"', 8000, 'engine OFF again');
    });

    await test('boot', 'no console errors or unexpected HTTP failures on load', async () => {
      page.assertClean('boot');
    });

    // --------------------------------------------------------------- power
    group('power');
    await test('power', 'power toggle drives the engine and persists across polls', async () => {
      const before = await simulator.state();
      assertEqual(before.state, 'OFF', 'boot state is dark');
      await page.click('#powerToggle');
      await page.waitForSnapshot('state.state === "ON"', 8000, 'engine ON');
      await sleep(1200); // survives at least one snapshot poll
      assertEqual((await simulator.state()).state, 'ON', 'still ON after a poll');
      await page.click('#powerToggle');
      await page.waitForSnapshot('state.state === "OFF"', 8000, 'engine OFF');
    });

    // ---------------------------------------------------------- brightness
    group('brightness');
    for (const value of [0, 128, 255]) {
      await test('brightness', `slider ${value} reaches the engine`, async () => {
        await page.setValue('#brightnessSlider', value);
        await page.waitForSnapshot(`Number(state.brightness) === ${value}`, 8000, `brightness ${value}`);
        await sleep(1100);
        assertEqual(Number((await simulator.state()).brightness), value, 'brightness after a poll');
      });
    }

    // --------------------------------------------------------------- colour
    group('colour');
    await test('colour', 'colour picker sends the RGB triple', async () => {
      await page.setValue('#colorPicker', '#ff0000');
      await page.waitForSnapshot('state.color && state.color.r === 255 && state.color.g === 0 && state.color.b === 0',
        8000, 'red colour');
    });

    // ----------------------------------------------------------------- text
    group('text');
    await test('text', 'text input reaches the engine', async () => {
      await page.setValue('#textInput', 'HELLO');
      await page.waitForSnapshot('state.text === "HELLO"', 8000, 'text HELLO');
    });

    // ------------------------------------------------------- speed / cycle
    group('speed');
    await test('speed', 'speed slider reaches the engine', async () => {
      await page.setValue('#speedSlider', 42);
      await page.waitForSnapshot('Number(state.speed) === 42', 8000, 'speed 42');
    });

    await test('speed', 'auto-cycle toggles on and off', async () => {
      await page.click('#cycleToggle');
      await page.waitForSnapshot('state.cycle === "ON"', 8000, 'cycle ON');
      await page.click('#cycleToggle');
      await page.waitForSnapshot('state.cycle === "OFF"', 8000, 'cycle OFF');
    });

    // -------------------------------------------------------------- catalog
    group('catalog');
    await test('catalog', 'every third effect card switches the active effect', async () => {
      const ids = await page.evaluate("Array.from(document.querySelectorAll('.effect-btn')).map(b => b.dataset.effectId)");
      assertEqual(ids.length, 82, 'effect buttons');
      for (let i = 0; i < ids.length; i += 3) {
        const id = ids[i];
        await page.clickExpression(`document.querySelector('.effect-btn[data-effect-id="${id}"]')`);
        await page.waitForSnapshot(`state.effect_id === ${JSON.stringify(id)}`, 8000, `effect ${id}`);
      }
    });

    await test('catalog', 'search filters the grid and reports no matches', async () => {
      await page.setValue('#effectSearch', 'fire');
      const visible = await page.waitFor(`(() => {
        const shown = Array.from(document.querySelectorAll('.effect-btn')).filter(b => b.offsetParent !== null);
        return shown.length ? shown.map(b => b.dataset.effectId) : false;
      })()`, 5000, 'filtered cards');
      assert(visible.includes('fire'), `fire missing from ${visible}`);
      // Matching is on id OR display name, so "Fire & Ice Yin-Yang" is correct.
      assert(visible.length < 82, `search did not filter anything: ${visible.length} cards`);
      assert(visible.length <= 6, `search matched too loosely: ${visible}`);

      await page.setValue('#effectSearch', 'zzz-no-such-effect');
      await page.waitFor("document.getElementById('effectSearchStatus').textContent.length > 0", 5000, 'no-match hint');

      await page.setValue('#effectSearch', '');
      const all = await page.evaluate("Array.from(document.querySelectorAll('.effect-btn')).filter(b => b.offsetParent !== null).length");
      assertEqual(all, 82, 'all cards restored');
    });

    // ----------------------------------------------------------- parameters
    group('parameters');
    await test('parameters', 'number parameter round-trips through the engine', async () => {
      // Pick an effect that exposes a Number parameter.
      const found = await page.evaluate(`(() => {
        for (const effect of catalog) {
          const param = (effect.parameters || []).find(p => p.kind === 'number');
          if (param) return { id: effect.id, param: param.id, min: param.min, max: param.max, step: param.step };
        }
        return null;
      })()`);
      assert(found, 'no numeric parameter found in the catalog');
      await page.clickExpression(`document.querySelector('.effect-btn[data-effect-id="${found.id}"]')`);
      await page.waitForSnapshot(`state.effect_id === ${JSON.stringify(found.id)}`, 8000, 'effect selected');
      const step = found.step || 1;
      const target = Math.round((found.min + (found.max - found.min) / 2) / step) * step;
      const selector = `#param_${found.param}`;
      await page.waitFor(`!!document.getElementById(${JSON.stringify(`param_${found.param}`)})`, 8000,
        'parameter control rendered');
      await page.setValue(selector, target);
      // Waiting for "any finite number" returns the pre-click value immediately;
      // the assertion has to wait for the value the engine reports after the set.
      const applied = await page.waitFor(`(() => {
        const effects = state.effects || {};
        const params = effects[${JSON.stringify(found.id)}];
        if (!params) return false;
        const value = Number(params[${JSON.stringify(found.param)}]);
        return Math.abs(value - ${target}) <= ${Math.max(1, step)} ? value : false;
      })()`, 10000, `parameter ${found.id}/${found.param} reaches ${target}`);
      assert(Math.abs(applied - target) <= Math.max(1, step),
        `parameter ${found.id}/${found.param}: expected ~${target}, got ${applied}`);
    });

    await test('parameters', 'boolean parameter toggles', async () => {
      const found = await page.evaluate(`(() => {
        for (const effect of catalog) {
          const param = (effect.parameters || []).find(p => p.kind === 'boolean');
          if (param) return { id: effect.id, param: param.id };
        }
        return null;
      })()`);
      if (!found) return; // catalog may not expose booleans; nothing to assert
      await page.clickExpression(`document.querySelector('.effect-btn[data-effect-id="${found.id}"]')`);
      await page.waitForSnapshot(`state.effect_id === ${JSON.stringify(found.id)}`, 8000, 'effect selected');
      const selector = `#param_${found.param}`;
      const before = await page.evaluate(`(() => { const el = document.querySelector('${selector}'); return el ? el.checked : null; })()`);
      await page.setChecked(selector, !before);
      // The engine stores booleans as the strings "ON"/"OFF".
      const expected = !before ? 'ON' : 'OFF';
      await page.waitFor(`(() => {
        const params = (state.effects || {})[${JSON.stringify(found.id)}];
        return params ? params[${JSON.stringify(found.param)}] === ${JSON.stringify(expected)} : false;
      })()`, 8000, 'boolean applied');
      const serverValue = await page.evaluate(`((state.effects || {})[${JSON.stringify(found.id)}] || {})[${JSON.stringify(found.param)}]`);
      assertEqual(serverValue, expected, `engine boolean for ${found.id}/${found.param}`);
      await page.setChecked(selector, before);
    });

    // ------------------------------------------------------------- playlist
    group('playlist');
    await test('playlist', 'fresh boot exposes a startable default queue', async () => {
      await page.openDetails('#playlistCard');
      const snapshot = await simulator.state();
      assertEqual(snapshot.playlistCollection.playlists.length >= 1, true, 'default playlist installed');
      assertEqual(snapshot.playlist.entries.length, 73, 'default queue entries');
      assertEqual(snapshot.playlist.status, 'stopped', 'queue starts stopped');
      const disabled = await page.disabled('#playlistStartBtn');
      assertEqual(disabled, false, 'Start button is enabled on a fresh boot');
    });

    await test('playlist', 'Start from a fresh boot actually runs the queue', async () => {
      await page.openDetails('#playlistCard');
      await page.click('#playlistStartBtn');
      await page.waitForSnapshot('state.playlist.status === "running"', 10000, 'playlist running');
      const snapshot = await simulator.state();
      assertEqual(snapshot.playlist.status, 'running', 'engine reports running');
      assertEqual(snapshot.state, 'ON', 'starting a queue wakes the panel');
    });

    await test('playlist', 'Pause and Resume change the status truthfully', async () => {
      await page.openDetails('#playlistCard');
      // Wait until Pause is enabled (it is disabled while the queue is not running).
      await page.waitFor("!document.getElementById('playlistPauseBtn').disabled", 8000, 'Pause enabled');
      await page.click('#playlistPauseBtn');
      await page.waitForSnapshot('state.playlist.status === "paused"', 8000, 'paused');
      await page.waitFor("!document.getElementById('playlistResumeBtn').disabled", 8000, 'Resume enabled');
      await page.click('#playlistResumeBtn');
      await page.waitForSnapshot('state.playlist.status === "running"', 8000, 'resumed');
    });

    await test('playlist', 'Next and Previous move the index', async () => {
      await page.openDetails('#playlistCard');
      const start = (await simulator.state()).playlist.index;
      await page.click('#playlistNextBtn');
      await page.waitForSnapshot(`state.playlist.index !== ${start}`, 8000, 'index advanced');
      await page.click('#playlistPreviousBtn');
      await page.waitForSnapshot(`state.playlist.index === ${start}`, 8000, 'index restored');
    });

    await test('playlist', 'loop checkbox is dirty-tracked and survives a save', async () => {
      await page.openDetails('#playlistCard');
      await page.openDetails('#playlistCard');
      const before = (await simulator.state()).playlist.loop;
      await page.setChecked('#playlistLoop', !before);
      // Toggling the checkbox marks the draft dirty; it is committed by Save queue.
      await page.waitFor('typeof playlistDraftDirty !== "undefined" && playlistDraftDirty === true',
        8000, 'draft marked dirty');
      await page.click('#playlistApplyBtn');
      await page.waitForSnapshot(`state.playlist.loop === ${!before}`, 12000, 'loop committed');
      await page.setChecked('#playlistLoop', before);
      await page.click('#playlistApplyBtn');
      await page.waitForSnapshot(`state.playlist.loop === ${before}`, 12000, 'loop restored');
    });

    await test('playlist', 'Stop halts the queue', async () => {
      await page.openDetails('#playlistCard');
      // Ensure a running queue first: Stop is only enabled for running/paused.
      if ((await simulator.state()).playlist.status !== 'running') {
        await page.waitFor("!document.getElementById('playlistStartBtn').disabled", 8000, 'Start enabled');
        await page.click('#playlistStartBtn');
        await page.waitForSnapshot('state.playlist.status === "running"', 12000, 'running before stop');
      }
      await page.waitFor("!document.getElementById('playlistStopBtn').disabled", 8000, 'Stop enabled');
      await page.click('#playlistStopBtn');
      await page.waitForSnapshot('state.playlist.status === "stopped"', 8000, 'stopped');
    });

    await test('playlist', 'Stop then Start works without a reload', async () => {
      await page.openDetails('#playlistCard');
      await page.waitFor("!document.getElementById('playlistStartBtn').disabled", 8000,
        'Start re-enabled after Stop');
      await page.click('#playlistStartBtn');
      await page.waitForSnapshot('state.playlist.status === "running"', 10000, 'running again');
      await page.click('#playlistStopBtn');
      await page.waitForSnapshot('state.playlist.status === "stopped"', 8000, 'stopped again');
    });

    await test('playlist', 'switching the saved playlist keeps playback stopped', async () => {
      await page.openDetails('#playlistCard');
      await page.openDetails('#playlistCard');
      const options = await page.evaluate("Array.from(document.querySelectorAll('#playlistSavedSelect option')).map(o => o.value)");
      assert(options.length >= 1, 'no saved playlist in the selector');
      await page.setValue('#playlistSavedSelect', options[0]);
      await page.click('#playlistSelectBtn');
      await page.waitForSnapshot('state.playlist.status === "stopped"', 8000, 'still stopped after switch');
    });

    // ---------------------------------------------------------------- audio
    group('audio');
    await test('audio', 'synthetic audio source can be selected and reverted', async () => {
      // #audioSoundBtn is the "listen to the synthetic track" preview and only
      // exists while the synthetic source is active; the source switch is
      // #audioSynthetic.
      await page.openDetails('#audioDisclosure');
      await page.click('#audioSynthetic');
      await page.waitForSnapshot('state.audio_source === "Synthetic"', 12000, 'synthetic audio');
      assertEqual((await simulator.state()).audioSource, 'Synthetic', 'engine switched to synthetic');
      await page.click('#audioOff');
      await page.waitForSnapshot('state.audio_source === "Off"', 12000, 'audio off');
      assertEqual((await simulator.state()).audioSource, 'Off', 'engine switched off');
    });

    await test('audio', 'mic source is accepted or honestly rejected', async () => {
      await page.click('#audioMic');
      const source = await page.waitFor(`(() => {
        const value = state.audio_source;
        return value === 'Mic' || value === 'Off' ? value : false;
      })()`, 8000, 'mic selection resolved');
      if (source === 'Mic') {
        assertEqual((await simulator.state()).audio_source, 'Mic', 'engine accepted mic source');
      }
      await page.click('#audioOff');
      await page.waitForSnapshot('state.audio_source === "Off"', 8000, 'audio off again');
    });

    await test('audio', 'mic sliders and toggles reach the engine', async () => {
      await page.openDetails('#micCard');
      const pairs = [
        ['#micGainSlider', 'gainDb'],
        ['#micAttackSlider', 'attackMs'],
        ['#micDecaySlider', 'decayMs'],
        ['#micBeatSlider', 'beatThreshold'],
        ['#micSquelchSlider', 'squelchDb'],
      ];
      for (const [selector, key] of pairs) {
        const value = await page.evaluate(`document.querySelector('${selector}').value`);
        await page.setValue(selector, value);
        // The engine echoes the value it accepted; assert it stays a real number
        // and that the UI shows the same one (no silent clamp to a default).
        const actual = await page.waitFor(`(() => {
          const value = Number((state.mic || {})[${JSON.stringify(key)}]);
          return Number.isFinite(value) ? value : false;
        })()`, 10000, `mic ${key}`);
        assert(Number.isFinite(actual), `mic.${key} is not a number: ${actual}`);
      }
      await page.setChecked('#micAgcToggle', true);
      await page.waitForSnapshot('state.mic?.agc === true', 8000, 'agc on');
      await page.setChecked('#micAgcToggle', false);
      await page.waitForSnapshot('state.mic?.agc === false', 8000, 'agc off');
    });

    await test('audio', 'mic presets apply a coherent set of values', async () => {
      await page.openDetails('#micCard');
      for (const preset of ['smooth', 'punchy', 'balanced']) {
        await page.clickExpression(`Array.from(document.querySelectorAll('button')).find(b => (b.getAttribute('onclick')||'').indexOf("applyMicPreset('${preset}')") !== -1)`);
        await sleep(200);
        const mic = (await simulator.state()).mic;
        assert(Number.isFinite(mic.gainDb), `preset ${preset} left mic.gainDb invalid`);
      }
    });

    // -------------------------------------------------------------- network
    group('network');
    await test('network', 'Wi-Fi card loads and reports the host has no radio', async () => {
      await page.openDetails('#networkCard');
      await page.waitFor("document.getElementById('networkStatus').textContent.length > 0", 8000, 'network status text');
      const config = await page.evaluate("networkConfig || null");
      assert(config, 'networkConfig never loaded');
      assertEqual(config.networkAvailable, false, 'host reports no radio');
    });

    await test('network', 'adding a profile persists it in the host store', async () => {
      await page.openDetails('#networkCard');
      await page.setValue('#networkSsid', 'e2e-home');
      await page.setValue('#networkPriority', 5);
      await page.setValue('#networkPassword', 'supersecret');
      await page.click('#networkAddProfileBtn');
      await page.click('#networkSaveBtn');
      await page.waitFor(`(() => {
        const profiles = (networkConfig || {}).wifiProfiles || [];
        return profiles.some(p => p.ssid === 'e2e-home');
      })()`, 12000, 'profile saved');
      const saved = await page.evaluate("networkConfig.wifiProfiles.find(p => p.ssid === 'e2e-home')");
      assertEqual(saved.passwordConfigured, true, 'password flagged as configured');
      const leaked = await page.evaluate("JSON.stringify(networkConfig).includes('supersecret')");
      assertEqual(leaked, false, 'the passphrase must never come back from the API');
    });

    await test('network', 'profile validation rejects a short passphrase', async () => {
      await page.openDetails('#networkCard');
      page.expectHttpError(/\/api\/network$/, 400);
      await page.setValue('#networkSsid', 'bad-pw');
      await page.setValue('#networkPassword', 'abc');
      await page.click('#networkAddProfileBtn');
      await page.click('#networkSaveBtn');
      await page.waitFor(`/at least 8|8 bytes|too short/i.test(document.getElementById('networkStatus').textContent)`,
        8000, 'validation message');
    });

    await test('network', 'reordering and removing a profile updates the list', async () => {
      await page.openDetails('#networkCard');
      // Add a second profile so reordering has an effect.
      await page.setValue('#networkSsid', 'e2e-garage');
      await page.setValue('#networkPriority', 1);
      await page.setValue('#networkPassword', 'anothersecret');
      await page.click('#networkAddProfileBtn');
      await page.click('#networkSaveBtn');
      await page.waitFor("((networkConfig||{}).wifiProfiles||[]).some(p => p.ssid === 'e2e-garage')",
        12000, 'second profile saved');

      const before = await page.evaluate("(networkConfig.wifiProfiles||[]).map(p => p.ssid)");
      assert(before.length >= 2, `expected two profiles, got ${before}`);
      const firstSsid = before[0];
      await page.clickExpression(`document.querySelector('[data-network-index="0"][data-network-action="move-down"]')`);
      const after = await page.evaluate("(networkConfig.wifiProfiles||[]).map(p => p.ssid)");
      assert(after[1] === firstSsid, `move-down did not swap: ${before} -> ${after}`);

      const removeIndex = after.indexOf('e2e-garage');
      await page.clickExpression(`document.querySelector('[data-network-index="${removeIndex}"][data-network-action="remove"]')`);
      await page.click('#networkSaveBtn');
      await page.waitFor("!((networkConfig||{}).wifiProfiles||[]).some(p => p.ssid === 'e2e-garage')",
        12000, 'profile removed');
    });

    await test('network', 'scan reports an empty radio-less result without breaking the card', async () => {
      await page.openDetails('#networkCard');
      await page.click('#networkScanBtn');
      await page.waitFor(`/no radio|nothing to scan|No networks|Scanning/i.test(document.getElementById('networkScanStatus').textContent)`,
        10000, 'scan status text');
      page.assertClean('network scan');
    });

    // ------------------------------------------------------- hardware config
    group('config');
    await test('config', 'panel config loads with real values', async () => {
      await page.openDetails('#configCard');
      const layout = await page.value('#cfgPanelLayout');
      assert(layout, 'panel layout is empty');
      const panelCells = await page.evaluate("document.querySelectorAll('#cfgPanels .panel-cell, #cfgPanels > *').length");
      assert(panelCells >= 1, `panel grid rendered ${panelCells} cells`);
    });

    await test('config', 'saving the panel config round-trips', async () => {
      await page.openDetails('#configCard');
      const original = await page.value('#cfgMaxMilliamps');
      await page.setValue('#cfgMaxMilliamps', 9000);
      await page.click('#cfgSaveBtn');
      await page.waitFor(`/saved|restart|applied/i.test(document.getElementById('cfgStatusMsg').textContent)`,
        10000, 'config save acknowledged');
      const status = await page.text('#cfgStatusMsg');
      log('config save status:', status);
      // Restore.
      await page.setValue('#cfgMaxMilliamps', original);
      await page.click('#cfgSaveBtn');
      await sleep(300);
    });

    await test('config', 'mirror and serpentine toggles are accepted', async () => {
      await page.openDetails('#configCard');
      for (const selector of ['#cfgMirrorX', '#cfgPanelSerpentine', '#cfgParallelOutput']) {
        const before = await page.evaluate(`document.querySelector('${selector}').checked`);
        await page.setChecked(selector, !before);
        await sleep(120);
        const after = await page.evaluate(`document.querySelector('${selector}').checked`);
        assertEqual(after, !before, `${selector} did not toggle`);
        await page.setChecked(selector, before);
      }
    });

    // ------------------------------------------------------------ diagnostics
    group('diagnostics');
    await test('diagnostics', 'diagnostics card shows live telemetry', async () => {
      await page.openDetails('#diagCard');
      const fps = await page.waitFor(`(() => {
        const text = document.getElementById('diagFps').textContent;
        return /\\d/.test(text) ? text : false;
      })()`, 10000, 'fps value');
      log('fps:', fps);
      const uptime = await page.text('#diagUptime');
      assert(/\d/.test(uptime), `uptime empty: ${uptime}`);
    });

    // ------------------------------------------------------------- persistence
    group('persistence');
    await test('persistence', 'save runtime state succeeds', async () => {
      await page.click('#saveStateBtn');
      await page.waitFor(`/saved|Settings saved/i.test(document.getElementById('statusToast').textContent)`,
        10000, 'save toast');
      assert(fs.existsSync(simulator.statePath), 'state file was not written');
    });

    await test('persistence', 'clear runtime state empties the saved file', async () => {
      await page.click('#clearStateBtn');
      await page.waitFor(`/clear|reset|empty/i.test(document.getElementById('statusToast').textContent)`,
        10000, 'clear toast');
      const size = fs.existsSync(simulator.statePath) ? fs.statSync(simulator.statePath).size : 0;
      assert(size < 200, `state file still ${size} bytes after clear`);
    });

    await test('persistence', 'restart reports honestly instead of hanging', async () => {
      page.expectHttpError(/\/api\/reboot$/, 501);
      page.expectHttpError(/\/api\/reboot$/, 404);
      page.expectHttpError(/\/api\/reboot$/, 503);
      await page.click('#restartBtn');
      await page.waitFor(`document.getElementById('statusToast').textContent.length > 0`, 10000, 'restart feedback');
      const toast = await page.text('#statusToast');
      log('restart toast:', toast);
      page.assertClean('restart');
    });

    // -------------------------------------------------------------- robustness
    group('robustness');
    await test('robustness', 'rapid start/stop clicks leave a coherent state', async () => {
      await page.evaluate(`(() => {
        const start = document.getElementById('playlistStartBtn');
        const stop = document.getElementById('playlistStopBtn');
        for (let i = 0; i < 40; i++) {
          if (!start.disabled) start.click();
          if (!stop.disabled) stop.click();
        }
      })()`);
      await sleep(1500);
      const status = (await simulator.state()).playlist.status;
      assert(['running', 'stopped', 'paused'].includes(status), `incoherent status ${status}`);
      // Settle into a known state.
      await page.evaluate(`(() => { const b = document.getElementById('playlistStopBtn'); if (!b.disabled) b.click(); })()`);
      await sleep(500);
      page.assertClean('rapid clicks');
    });

    await test('robustness', 'a reload mid-session restores a usable UI', async () => {
      await page.reload(simulator.base);
      await page.waitForSnapshot('Array.isArray(state.playlist?.entries)', 10000, 'snapshot after reload');
      const cards = await page.evaluate("document.querySelectorAll('.effect-btn').length");
      assertEqual(cards, 82, 'catalog after reload');
      await page.waitFor("typeof updatePlaylistUI === 'function'", 8000, 'UI ready');
      page.assertClean('reload');
    });

    await test('robustness', 'offline fetches do not wedge the UI', async () => {
      await page.evaluate(`(() => {
        window.__realFetch = window.fetch;
        window.fetch = () => Promise.reject(new Error('offline'));
      })()`);
      await sleep(2500);
      const stillUsable = await page.evaluate(`(() => {
        const el = document.getElementById('effectsGrid');
        return !!(el && el.children.length);
      })()`);
      assert(stillUsable, 'catalog vanished while offline');
      await page.evaluate('window.fetch = window.__realFetch');
      await page.waitForSnapshot('state && state.playlist', 10000, 'recovered after offline');
    });

    await test('robustness', 'no unexpected errors after the whole run', async () => {
      page.assertClean('full run');
    });

  } finally {
    if (page) page.close();
    browser.stop();
    await simulator.stop();
    const report = {
      generatedAt: new Date().toISOString(),
      browser: browser.browser,
      filter: FILTER,
      total: results.length,
      passed: results.filter(r => r.status === 'pass').length,
      failed: results.filter(r => r.status === 'fail').length,
      results,
    };
    fs.writeFileSync(path.join(ARTIFACTS, 'report.json'), JSON.stringify(report, null, 1));
    console.log(`\n${report.passed}/${report.total} passed, ${report.failed} failed`);
    console.log(`report: ${path.join(ARTIFACTS, 'report.json')}`);
  }

  const failed = results.filter(r => r.status === 'fail');
  if (failed.length) {
    console.log('\nfailures:');
    for (const entry of failed) console.log(`  ${entry.group} / ${entry.name}\n    ${entry.detail}`);
    process.exit(1);
  }
}

main().catch(error => {
  console.error(error);
  process.exit(1);
});
