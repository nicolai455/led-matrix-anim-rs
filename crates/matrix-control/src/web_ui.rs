/// Full usable embedded Web UI for LED Matrix.
///
/// Features:
/// - Responsive dark-themed interface
/// - 32x32 live preview canvas with pixelated rendering
/// - Power, Brightness, Color Picker, Text Message, Scroll Speed, Auto Cycle
/// - Dynamic effect parameter generation (numbers, booleans, selects)
/// - Audio reactivity controls and 16-band audio visualizer
/// - Panel configuration (layout, order, rotation, power budget) with live power updates
/// - Microphone DSP tuning with presets, sliders, and live dBFS telemetry
/// - Diagnostics, real-time FPS counter, and error banner
/// - Flash state save/restore
pub const WEB_UI: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>LED Matrix Control</title>
<style>
:root {
  --bg: #0d1117;
  --card: #161b22;
  --card-hover: #21262d;
  --border: #30363d;
  --text: #c9d1d9;
  --muted: #8b949e;
  --accent: #58a6ff;
  --accent-dim: #1f6feb;
  --green: #2ea043;
  --red: #da3633;
  --yellow: #d29922;
  --radius: 8px;
  --font: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
}
* { margin:0; padding:0; box-sizing:border-box; }
body { background:var(--bg); color:var(--text); font-family:var(--font); padding:16px; max-width:680px; margin:0 auto; }
h1 { font-size:1.3rem; font-weight:600; }
h2 { font-size:0.8rem; font-weight:600; margin-bottom:10px; color:var(--muted); text-transform:uppercase; letter-spacing:1px; }

.header { display:flex; align-items:center; justify-content:space-between; padding:8px 0 16px; }
.status-pill { display:inline-flex; align-items:center; gap:6px; font-size:0.8rem; color:var(--muted); }
.status-dot { width:8px; height:8px; border-radius:50%; background:#484f58; transition:all 0.3s; }
.status-dot.connected { background:var(--green); box-shadow:0 0 8px var(--green); }

.card { background:var(--card); border:1px solid var(--border); border-radius:var(--radius); padding:16px; margin-bottom:14px; }
.preview-wrap { display:flex; flex-direction:column; align-items:center; }
.matrix-preview { width:min(100%, 448px); height:auto; aspect-ratio:1; image-rendering:pixelated; border:2px solid var(--border); border-radius:4px; background:#000; }
.preview-controls { width:min(100%, 448px); margin-top:12px; }
.preview-note { font-size:0.75rem; color:var(--muted); margin-top:8px; line-height:1.5; }

.toggle { position:relative; display:inline-block; width:48px; height:26px; }
.toggle input { opacity:0; width:0; height:0; }
.toggle-track { position:absolute; inset:0; background:var(--border); border-radius:26px; transition:0.2s; cursor:pointer; }
.toggle-track:before { position:absolute; content:""; height:20px; width:20px; left:3px; bottom:3px; background:#fff; border-radius:50%; transition:0.2s; }
input:checked + .toggle-track { background:var(--green); }
input:checked + .toggle-track:before { transform:translateX(22px); }

.slider-row { display:flex; align-items:center; gap:12px; margin-top:8px; }
.slider-row input[type="range"] { flex:1; accent-color:var(--accent); }
.slider-val { min-width:3ch; text-align:right; font-variant-numeric:tabular-nums; font-size:0.9rem; }

/* Grid */
.effects-grid { display:grid; grid-template-columns:repeat(auto-fill, minmax(130px, 1fr)); gap:8px; }
.effect-btn { background:var(--card-hover); border:1px solid var(--border); border-radius:6px; padding:10px 8px; text-align:center; cursor:pointer; font-size:0.85rem; user-select:none; transition:0.15s; }
.effect-btn:hover { border-color:var(--accent); }
.effect-btn.active { background:var(--accent-dim); border-color:var(--accent); color:#fff; font-weight:600; }

/* Params */
.param-item { margin-bottom:12px; }
.param-item:last-child { margin-bottom:0; }
.param-label { display:flex; justify-content:space-between; font-size:0.85rem; margin-bottom:4px; }
.param-select { width:100%; background:var(--card-hover); color:var(--text); border:1px solid var(--border); border-radius:6px; padding:6px 8px; }

/* Buttons */
.btn-row { display:flex; gap:8px; margin-top:10px; flex-wrap:wrap; }
.btn { background:var(--card-hover); border:1px solid var(--border); color:var(--text); border-radius:6px; padding:8px 14px; font-size:0.85rem; cursor:pointer; transition:0.15s; }
.btn:hover { border-color:var(--accent); }
.btn-primary { background:var(--accent-dim); border-color:var(--accent); color:#fff; }
.btn-danger { background:#490202; border-color:var(--red); color:#ff7b72; }

/* Audio visualizer */
.audio-btns { display:grid; grid-template-columns:1fr 1fr 1fr; gap:8px; margin-bottom:12px; }
.audio-btn { background:var(--card-hover); border:1px solid var(--border); border-radius:6px; padding:8px; text-align:center; cursor:pointer; font-size:0.85rem; }
.audio-btn.active { background:var(--accent-dim); border-color:var(--accent); color:#fff; }
.bands-grid { display:flex; gap:3px; height:48px; align-items:flex-end; background:#000; padding:4px; border-radius:4px; margin-top:8px; }
.band-col { flex:1; background:var(--accent); border-radius:2px; height:0%; transition:height 0.05s; }

.footer { text-align:center; font-size:0.75rem; color:var(--muted); padding:16px 0; }
#statusToast { display:none; position:fixed; bottom:20px; left:50%; transform:translateX(-50%); padding:8px 16px; border-radius:6px; font-size:0.85rem; font-weight:600; z-index:9999; box-shadow:0 4px 12px rgba(0,0,0,0.5); }
</style>
</head>
<body>

<div class="header">
  <div>
    <h1>LED Matrix</h1>
    <span class="status-pill">
      <span class="status-dot" id="statusDot"></span>
      <span id="statusText">Connecting...</span> |
      <span id="fpsDisplay">-- FPS</span>
    </span>
  </div>
  <div class="power-row">
    <label class="toggle">
      <input type="checkbox" id="powerToggle" onchange="setPower(this.checked)">
      <span class="toggle-track"></span>
    </label>
  </div>
</div>

<!-- Preview -->
<div class="card preview-wrap">
  <h2>Live Preview</h2>
  <canvas class="matrix-preview" id="previewCanvas" width="32" height="32" aria-label="LED matrix preview"></canvas>
  <div class="preview-controls">
    <label for="previewMode">Preview rendering</label>
    <select id="previewMode" onchange="changePreviewMode()">
      <option value="browser">Browser · smooth, low ESP load</option>
      <option value="device">Device frames · exact, higher ESP load</option>
      <option value="off">Off</option>
    </select>
    <p class="preview-note" id="previewStatus" role="status">Loading browser renderer…</p>
  </div>
</div>

<!-- Brightness & Global Settings -->
<div class="card">
  <h2>Brightness & Global Settings</h2>
  <div class="slider-row">
    <label style="min-width:70px; font-size:0.85rem">Brightness</label>
    <input type="range" id="brightnessSlider" min="0" max="255" value="128" oninput="brightVal.textContent=this.value" onchange="setBrightness(this.value)">
    <span class="slider-val" id="brightVal">128</span>
  </div>
  <div class="slider-row" style="margin-top:12px">
    <label style="min-width:70px; font-size:0.85rem">Color</label>
    <input type="color" id="colorPicker" value="#ffffff" onchange="setColorHex(this.value)">
  </div>
  <div class="slider-row" style="margin-top:12px">
    <label style="min-width:70px; font-size:0.85rem">Message</label>
    <input type="text" id="textInput" style="flex:1; background:var(--card-hover); border:1px solid var(--border); border-radius:6px; padding:6px 8px; color:var(--text)" placeholder="HELLO" onchange="setText(this.value)">
  </div>
  <div class="slider-row" style="margin-top:12px">
    <label style="min-width:70px; font-size:0.85rem">Speed (px/s)</label>
    <input type="range" id="speedSlider" min="0" max="120" value="30" oninput="speedVal.textContent=this.value" onchange="setSpeed(this.value)">
    <span class="slider-val" id="speedVal">30</span>
  </div>
  <div class="slider-row" style="margin-top:12px">
    <label style="min-width:70px; font-size:0.85rem">Auto Cycle</label>
    <label class="toggle">
      <input type="checkbox" id="cycleToggle" onchange="setCycle(this.checked)">
      <span class="toggle-track"></span>
    </label>
  </div>
</div>

<!-- Effects -->
<div class="card">
  <h2>Effects</h2>
  <div class="effects-grid" id="effectsGrid"></div>
</div>

<!-- Dynamic Effect Parameters -->
<div class="card" id="paramsCard" style="display:none">
  <h2 id="paramsTitle">Effect Parameters</h2>
  <div id="paramsContainer"></div>
</div>

<!-- Audio -->
<div class="card">
  <h2>Audio Reactivity</h2>
  <div class="audio-btns">
    <div class="audio-btn" id="audioOff" onclick="setAudio('Off')">Off</div>
    <div class="audio-btn" id="audioSynthetic" onclick="setAudio('Synthetic')">Synthetic</div>
    <div class="audio-btn" id="audioMic" onclick="setAudio('Mic')">Microphone</div>
  </div>
  <div id="audioVisualizer" style="display:none">
    <div class="bands-grid" id="bandsGrid"></div>
  </div>
</div>

<!-- Microphone Tuning & DSP -->
<details class="card" id="micCard">
  <summary style="cursor:pointer; font-weight:600; color:var(--text); font-size:0.9rem;">MICROPHONE TUNING & DSP</summary>
  <div style="margin-top:12px">
    <div style="display:flex; gap:6px; margin-bottom:10px">
      <button class="btn" style="flex:1" onclick="applyMicPreset('balanced')">Balanced</button>
      <button class="btn" style="flex:1" onclick="applyMicPreset('punchy')">Punchy</button>
      <button class="btn" style="flex:1" onclick="applyMicPreset('smooth')">Smooth</button>
    </div>
    <div class="slider-row">
      <label style="min-width:110px; font-size:0.85rem">Gain (dB)</label>
      <input type="range" id="micGainSlider" min="0" max="60" value="30" oninput="micGainVal.textContent=this.value" onchange="submitMicSetting('gainDb', parseFloat(this.value))">
      <span class="slider-val" id="micGainVal">30</span>
    </div>
    <div class="slider-row" style="margin-top:8px">
      <label style="min-width:110px; font-size:0.85rem">Squelch (dB)</label>
      <input type="range" id="micSquelchSlider" min="-100" max="-10" value="-60" oninput="micSquelchVal.textContent=this.value" onchange="submitMicSetting('squelchDb', parseFloat(this.value))">
      <span class="slider-val" id="micSquelchVal">-60</span>
    </div>
    <div class="slider-row" style="margin-top:8px">
      <label style="min-width:110px; font-size:0.85rem">Beat Sensitivity</label>
      <input type="range" id="micBeatSlider" min="1.0" max="5.0" step="0.1" value="1.6" oninput="micBeatVal.textContent=this.value" onchange="submitMicSetting('beatThreshold', parseFloat(this.value))">
      <span class="slider-val" id="micBeatVal">1.6</span>
    </div>
    <div class="slider-row" style="margin-top:8px">
      <label style="min-width:110px; font-size:0.85rem">Attack (ms)</label>
      <input type="range" id="micAttackSlider" min="1" max="1000" value="15" oninput="micAttackVal.textContent=this.value" onchange="submitMicSetting('attackMs', parseFloat(this.value))">
      <span class="slider-val" id="micAttackVal">15</span>
    </div>
    <div class="slider-row" style="margin-top:8px">
      <label style="min-width:110px; font-size:0.85rem">Decay (ms)</label>
      <input type="range" id="micDecaySlider" min="10" max="5000" value="100" oninput="micDecayVal.textContent=this.value" onchange="submitMicSetting('decayMs', parseFloat(this.value))">
      <span class="slider-val" id="micDecayVal">100</span>
    </div>
    <div class="slider-row" style="margin-top:8px">
      <label style="min-width:110px; font-size:0.85rem">AGC (Auto Gain)</label>
      <label class="toggle">
        <input type="checkbox" id="micAgcToggle" onchange="submitMicSetting('agc', this.checked)">
        <span class="toggle-track"></span>
      </label>
    </div>
    <div style="margin-top:10px; padding:8px; background:var(--card-hover); border-radius:4px; font-size:0.8rem; display:flex; justify-content:space-between">
      <span>Raw: <strong id="micRawDisplay">--</strong> dBFS</span>
      <span>Applied Gain: <strong id="micGainDisplay">--</strong> dB</span>
      <span>Hardware: <strong id="micHwDisplay">--</strong></span>
    </div>
  </div>
</details>

<!-- Panel Configuration -->
<details class="card" id="configCard">
  <summary style="cursor:pointer; font-weight:600; color:var(--text); font-size:0.9rem;">PANEL CONFIGURATION</summary>
  <div style="margin-top:12px">
    <div class="slider-row">
      <label style="min-width:100px; font-size:0.85rem">Matrix Size</label>
      <input type="number" id="cfgWidth" min="1" max="8" value="2" style="width:60px; background:var(--card-hover); border:1px solid var(--border); border-radius:4px; padding:4px 6px; color:var(--text)">
      <span style="font-size:0.85rem">×</span>
      <input type="number" id="cfgHeight" min="1" max="8" value="2" style="width:60px; background:var(--card-hover); border:1px solid var(--border); border-radius:4px; padding:4px 6px; color:var(--text)">
    </div>
    <div class="slider-row" style="margin-top:8px">
      <label style="min-width:100px; font-size:0.85rem">Start Corner</label>
      <select id="cfgStartCorner" class="param-select" style="flex:1">
        <option value="0">Top-Left (0)</option>
        <option value="1">Top-Right (1)</option>
        <option value="2">Bottom-Left (2)</option>
        <option value="3">Bottom-Right (3)</option>
      </select>
    </div>
    <div class="slider-row" style="margin-top:8px">
      <label style="min-width:100px; font-size:0.85rem">Panel Layout</label>
      <select id="cfgPanelLayout" class="param-select" style="flex:1">
        <option value="0">Horizontal (0)</option>
        <option value="1">Vertical (1)</option>
      </select>
    </div>
    <div class="slider-row" style="margin-top:8px">
      <label style="min-width:100px; font-size:0.85rem">Panel Order</label>
      <input type="text" id="cfgPanelOrder" style="flex:1; background:var(--card-hover); border:1px solid var(--border); border-radius:4px; padding:4px 6px; color:var(--text)" placeholder="0,1,2,3">
    </div>
    <div class="slider-row" style="margin-top:8px">
      <label style="min-width:100px; font-size:0.85rem">Data Pin</label>
      <input type="number" id="cfgDataPin" min="0" max="48" value="8" style="width:70px; background:var(--card-hover); border:1px solid var(--border); border-radius:4px; padding:4px 6px; color:var(--text)">
      <label style="margin-left:12px; min-width:90px; font-size:0.85rem">Parallel Out</label>
      <input type="checkbox" id="cfgParallelOutput">
    </div>
    <div class="slider-row" style="margin-top:8px">
      <label style="min-width:100px; font-size:0.85rem">Max Power (mA)</label>
      <input type="number" id="cfgMaxMilliamps" min="500" max="60000" step="500" value="12000" style="flex:1; background:var(--card-hover); border:1px solid var(--border); border-radius:4px; padding:4px 6px; color:var(--text)">
    </div>
    <div class="btn-row" style="margin-top:12px">
      <button class="btn btn-primary" onclick="savePanelConfig()">Save Configuration</button>
      <span id="cfgStatusMsg" style="font-size:0.8rem; color:var(--muted); align-self:center"></span>
    </div>
  </div>
</details>

<!-- Diagnostics & Telemetry -->
<details class="card" id="diagCard">
  <summary style="cursor:pointer; font-weight:600; color:var(--text); font-size:0.9rem;">DIAGNOSTICS & TELEMETRY</summary>
  <div style="margin-top:10px; font-size:0.85rem; display:grid; grid-template-columns:1fr 1fr; gap:8px">
    <div>FPS: <strong id="diagFps">--</strong></div>
    <div>Uptime: <strong id="diagUptime">--</strong></div>
    <div>Output Errors: <strong id="diagErrors">0</strong></div>
    <div>Last Error: <strong id="diagLastError">None</strong></div>
  </div>
</details>

<!-- Persistence & Config -->
<div class="card">
  <h2>Settings & Persistence</h2>
  <div class="btn-row">
    <button class="btn btn-primary" onclick="saveState()">Save to Flash</button>
    <button class="btn btn-danger" onclick="clearState()">Clear Saved</button>
  </div>
</div>

<div class="footer" id="footer">
  LED Matrix · ESP32/S3 & Host
</div>

<div id="statusToast"></div>

<script>
let state = {};
let catalog = [];
let loadedConfig = {};
let micSettings = {};
const previewCtx = document.getElementById('previewCanvas').getContext('2d');
const previewImage = previewCtx.createImageData(32, 32);
const previewEncoder = new TextEncoder();
let previewRenderer = null;
let previewFallback = false;
let previewLoading = false;
let previewClockOffset = 0;
let previewLastTime = 0;
let previewReady = false;
let previewConnected = false;

function previewMode() { return document.getElementById('previewMode').value; }
function previewStatus(text) { document.getElementById('previewStatus').textContent = text; }

function syncPreviewState() {
  if (!previewRenderer || previewMode() !== 'browser') return;
  try {
    const bytes = previewEncoder.encode(JSON.stringify(state));
    const capacity = previewRenderer.preview_input_capacity();
    if (bytes.length > capacity) throw new Error('State exceeds renderer capacity');
    new Uint8Array(previewRenderer.memory.buffer, previewRenderer.preview_input(), bytes.length).set(bytes);
    previewFallback = previewRenderer.preview_sync(bytes.length) !== 0;
    previewReady = !previewFallback;
    previewConnected = true;
    previewClockOffset = Number(state.renderTimeMs || 0) - performance.now();
    previewStatus(previewFallback
      ? 'Device frames: this effect cannot be simulated locally (higher ESP load).'
      : 'Rendered in your browser. Simulation may differ in phase or random detail from the LEDs.');
  } catch (e) {
    previewReady = false;
    previewFallback = false;
    previewStatus('Browser renderer error. Select Device frames to use the ESP preview.');
  }
}

async function loadPreviewRenderer() {
  if (previewRenderer || previewLoading || previewMode() !== 'browser') return;
  previewLoading = true;
  previewStatus('Loading browser renderer…');
  try {
    const response = await fetch('/preview.wasm.gz', { cache: 'no-cache' });
    if (!response.ok) throw new Error(response.statusText);
    const decompressed = response.body.pipeThrough(new DecompressionStream('gzip'));
    const module = await WebAssembly.instantiate(await new Response(decompressed).arrayBuffer(), {});
    previewRenderer = module.instance.exports;
    syncPreviewState();
  } catch (e) {
    previewStatus('Browser renderer unavailable. Select Device frames for the ESP preview.');
  } finally {
    previewLoading = false;
  }
}

function changePreviewMode() {
  previewFallback = false;
  previewReady = false;
  if (previewMode() === 'off') {
    previewImage.data.fill(0);
    previewCtx.putImageData(previewImage, 0, 0);
    previewStatus('Preview off.');
  } else if (previewMode() === 'device') {
    previewStatus('Exact device frames · limited to 5 FPS to reduce ESP load.');
  } else {
    syncPreviewState();
    loadPreviewRenderer();
  }
}

function animatePreview(now) {
  if (!document.hidden && previewMode() === 'browser' && previewReady && previewConnected) {
    try {
      previewLastTime = Math.max(previewLastTime, now + previewClockOffset, 0);
      const ptr = previewRenderer.preview_render(previewLastTime);
      previewImage.data.set(new Uint8Array(previewRenderer.memory.buffer, ptr, 4096));
      previewCtx.putImageData(previewImage, 0, 0);
    } catch (e) {
      previewReady = false;
      previewStatus('Browser renderer stopped. Select Device frames for the ESP preview.');
    }
  }
  requestAnimationFrame(animatePreview);
}

function showToast(msg, bg) {
  const t = document.getElementById('statusToast');
  t.textContent = msg;
  t.style.background = bg || 'var(--card-hover)';
  t.style.color = '#fff';
  t.style.display = 'block';
}

function clearToast() {
  document.getElementById('statusToast').style.display = 'none';
}

async function fetchJSON(url) {
  const r = await fetch(url);
  if (!r.ok) throw new Error(r.statusText);
  return await r.json();
}

async function postCommand(kind, payload) {
  try {
    const r = await fetch('/api/command', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ kind, payload })
    });
    const data = await r.json();
    if (r.status === 202 || data.pending) {
      showToast('Command queued (processing)...', 'var(--yellow)');
      setTimeout(clearToast, 1200);
    } else if (!r.ok) {
      showToast('Error: ' + (data.error || r.statusText), 'var(--red)');
      setTimeout(clearToast, 3500);
    } else {
      clearToast();
    }
    return data;
  } catch (e) {
    showToast('Network error submitting command', 'var(--red)');
    setTimeout(clearToast, 3000);
    throw e;
  }
}

function setPower(on) {
  postCommand('light', { state: on ? 'ON' : 'OFF' });
}

function setBrightness(v) {
  postCommand('light', { brightness: parseInt(v) });
}

function setColorHex(hex) {
  const r = parseInt(hex.slice(1,3), 16);
  const g = parseInt(hex.slice(3,5), 16);
  const b = parseInt(hex.slice(5,7), 16);
  postCommand('light', { color: { r, g, b } });
}

function setText(text) {
  postCommand('text', text);
}

function setSpeed(speed) {
  postCommand('speed', String(speed));
}

function setCycle(on) {
  postCommand('cycle', on ? 'ON' : 'OFF');
}

function setAudio(src) {
  postCommand('audio_source', src);
}

function selectEffect(name) {
  postCommand('light', { effect: name });
}

function setParam(effectId, paramId, val) {
  postCommand('effect/' + effectId + '/' + paramId, String(val));
}

async function saveState() {
  const response = await fetch('/api/state/save', { method: 'POST' });
  if (!response.ok) { showToast('Save failed: ' + response.status, 'var(--red)'); return; }
  showToast('Settings saved to flash', 'var(--green)');
  setTimeout(clearToast, 2000);
}

async function clearState() {
  if (confirm('Clear saved settings?')) {
    const response = await fetch('/api/state/clear', { method: 'POST' });
    if (!response.ok) { showToast('Clear failed: ' + response.status, 'var(--red)'); return; }
    showToast('Saved settings cleared', 'var(--muted)');
    setTimeout(clearToast, 2000);
  }
}

async function loadPanelConfig() {
  try {
    const cfg = await fetchJSON('/api/config');
    loadedConfig = cfg;
    document.getElementById('cfgWidth').value = cfg.matrixWidth || 2;
    document.getElementById('cfgHeight').value = cfg.matrixHeight || 2;
    document.getElementById('cfgStartCorner').value = cfg.startCorner !== undefined ? cfg.startCorner : 0;
    document.getElementById('cfgPanelLayout').value = cfg.panelLayout !== undefined ? cfg.panelLayout : 0;
    if (cfg.panelOrder) document.getElementById('cfgPanelOrder').value = cfg.panelOrder.join(',');
    document.getElementById('cfgDataPin').value = cfg.dataPin !== undefined ? cfg.dataPin : 8;
    document.getElementById('cfgParallelOutput').checked = !!cfg.parallelOutput;
    document.getElementById('cfgMaxMilliamps').value = cfg.ledMaxMilliamps || 12000;
  } catch(e) {}
}

async function savePanelConfig() {
  const msg = document.getElementById('cfgStatusMsg');
  msg.textContent = 'Saving...';
  const orderStr = document.getElementById('cfgPanelOrder').value;
  const order = orderStr.split(',').map(s => parseInt(s.trim())).filter(n => !isNaN(n));
  const payload = {
    ...loadedConfig,
    matrixWidth: parseInt(document.getElementById('cfgWidth').value),
    matrixHeight: parseInt(document.getElementById('cfgHeight').value),
    startCorner: parseInt(document.getElementById('cfgStartCorner').value),
    panelLayout: parseInt(document.getElementById('cfgPanelLayout').value),
    panelOrder: order.length === 4 ? order : [0, 1, 2, 3],
    dataPin: parseInt(document.getElementById('cfgDataPin').value),
    parallelOutput: document.getElementById('cfgParallelOutput').checked,
    ledMaxMilliamps: parseInt(document.getElementById('cfgMaxMilliamps').value)
  };
  try {
    const r = await fetch('/api/config', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    const res = await r.json();
    if (r.ok) {
      if (res.restartRequired || res.restart === 'required') {
        msg.textContent = 'Saved! (Reboot required for pin/layout changes)';
        msg.style.color = 'var(--yellow)';
      } else {
        msg.textContent = 'Saved! (Power limit applied live)';
        msg.style.color = 'var(--green)';
      }
    } else {
      msg.textContent = 'Error: ' + (res.error || r.statusText);
      msg.style.color = 'var(--red)';
    }
  } catch (e) {
    msg.textContent = 'Save failed';
    msg.style.color = 'var(--red)';
  }
}

async function loadMicData() {
  try {
    const d = await fetchJSON('/api/mic');
    if (d && d.settings) {
      micSettings = d.settings;
      document.getElementById('micGainSlider').value = d.settings.gainDb;
      document.getElementById('micGainVal').textContent = d.settings.gainDb;
      document.getElementById('micSquelchSlider').value = d.settings.squelchDb;
      document.getElementById('micSquelchVal').textContent = d.settings.squelchDb;
      document.getElementById('micBeatSlider').value = d.settings.beatThreshold;
      document.getElementById('micBeatVal').textContent = d.settings.beatThreshold;
      document.getElementById('micAttackSlider').value = d.settings.attackMs;
      document.getElementById('micAttackVal').textContent = d.settings.attackMs;
      document.getElementById('micDecaySlider').value = d.settings.decayMs;
      document.getElementById('micDecayVal').textContent = d.settings.decayMs;
      document.getElementById('micAgcToggle').checked = !!d.settings.agc;
    }
    if (d.rawDbfs !== null && d.rawDbfs !== undefined) {
      document.getElementById('micRawDisplay').textContent = d.rawDbfs.toFixed(1);
    }
    if (d.gainDb !== null && d.gainDb !== undefined) {
      document.getElementById('micGainDisplay').textContent = d.gainDb.toFixed(1);
    }
    document.getElementById('micHwDisplay').textContent = d.available ? 'Connected' : 'None';
  } catch(e) {}
}

async function submitMicSetting(field, val) {
  const payload = { ...micSettings, [field]: val };
  await postCommand('mic', payload);
  micSettings[field] = val;
}

function applyMicPreset(preset) {
  if (preset === 'balanced') {
    submitMicSetting('gainDb', 30.0);
    submitMicSetting('squelchDb', -60.0);
    submitMicSetting('beatThreshold', 1.6);
  } else if (preset === 'punchy') {
    submitMicSetting('gainDb', 36.0);
    submitMicSetting('squelchDb', -50.0);
    submitMicSetting('beatThreshold', 2.0);
  } else if (preset === 'smooth') {
    submitMicSetting('gainDb', 24.0);
    submitMicSetting('squelchDb', -70.0);
    submitMicSetting('beatThreshold', 1.3);
  }
  loadMicData();
}

function updateUI() {
  document.getElementById('powerToggle').checked = state.state === 'ON';
  document.getElementById('brightnessSlider').value = state.brightness ?? 128;
  document.getElementById('brightVal').textContent = state.brightness ?? 128;
  if (state.color) {
    const hex = '#' + [state.color.r, state.color.g, state.color.b].map(x => x.toString(16).padStart(2,'0')).join('');
    document.getElementById('colorPicker').value = hex;
  }
  if (state.text !== undefined && document.activeElement !== document.getElementById('textInput')) {
    document.getElementById('textInput').value = state.text;
  }
  if (state.speed !== undefined && document.activeElement !== document.getElementById('speedSlider')) {
    document.getElementById('speedSlider').value = state.speed;
    document.getElementById('speedVal').textContent = state.speed;
  }
  document.getElementById('cycleToggle').checked = state.cycle === 'ON';

  // Audio buttons
  ['Off', 'Synthetic', 'Mic'].forEach(s => {
    const el = document.getElementById('audio' + s);
    if (el) el.classList.toggle('active', state.audio_source === s);
  });
  document.getElementById('audioVisualizer').style.display = (state.audio_source !== 'Off') ? 'block' : 'none';

  // Update visualizer bands
  if (state.bands && state.bands.length === 16) {
    const cols = document.querySelectorAll('.band-col');
    for (let i = 0; i < 16 && i < cols.length; i++) {
      const h = Math.min(100, Math.max(0, state.bands[i] * 100));
      cols[i].style.height = h + '%';
    }
  }

  // Update FPS and Diagnostics
  if (state.fps !== undefined) {
    const fpsStr = state.fps.toFixed(1) + ' FPS';
    document.getElementById('fpsDisplay').textContent = fpsStr;
    document.getElementById('diagFps').textContent = fpsStr;
  }
  if (state.uptime !== undefined) {
    document.getElementById('diagUptime').textContent = state.uptime + 's';
  }
  if (state.outputErrors !== undefined) {
    document.getElementById('diagErrors').textContent = state.outputErrors;
  }
  if (state.micRawDbfs !== undefined) {
    document.getElementById('micRawDisplay').textContent = state.micRawDbfs.toFixed(1);
  }
  if (state.micGainDb !== undefined) {
    document.getElementById('micGainDisplay').textContent = state.micGainDb.toFixed(1);
  }

  // Highlight active effect
  document.querySelectorAll('.effect-btn').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.name === state.effect);
  });

  renderParameters();
}

function renderCatalog() {
  const grid = document.getElementById('effectsGrid');
  grid.innerHTML = '';
  catalog.forEach(eff => {
    const btn = document.createElement('div');
    btn.className = 'effect-btn';
    btn.dataset.name = eff.name;
    btn.textContent = eff.name;
    btn.onclick = () => selectEffect(eff.name);
    grid.appendChild(btn);
  });
}

function renderParameters() {
  const card = document.getElementById('paramsCard');
  const container = document.getElementById('paramsContainer');
  const currentEff = catalog.find(e => e.name === state.effect);

  if (!currentEff || !currentEff.parameters || currentEff.parameters.length === 0) {
    card.style.display = 'none';
    return;
  }

  card.style.display = 'block';
  document.getElementById('paramsTitle').textContent = currentEff.name + ' Parameters';

  if (container.dataset.effectId !== currentEff.id) {
    container.innerHTML = '';
    container.dataset.effectId = currentEff.id;

    currentEff.parameters.forEach(p => {
      const item = document.createElement('div');
      item.className = 'param-item';

      const label = document.createElement('div');
      label.className = 'param-label';
      label.innerHTML = '<span>' + p.name + '</span><span id="pval_' + p.id + '"></span>';
      item.appendChild(label);

      if (p.kind === 'boolean') {
        const toggle = document.createElement('label');
        toggle.className = 'toggle';
        toggle.innerHTML = '<input type="checkbox" id="param_' + p.id + '" onchange="setParam(\'' + currentEff.id + '\', \'' + p.id + '\', this.checked)"><span class="toggle-track"></span>';
        item.appendChild(toggle);
      } else if (p.kind === 'select') {
        const sel = document.createElement('select');
        sel.id = 'param_' + p.id;
        sel.className = 'param-select';
        (p.options || []).forEach(opt => {
          const optEl = document.createElement('option');
          optEl.value = opt;
          optEl.textContent = opt;
          sel.appendChild(optEl);
        });
        sel.onchange = (e) => setParam(currentEff.id, p.id, e.target.value);
        item.appendChild(sel);
      } else {
        const row = document.createElement('div');
        row.className = 'slider-row';
        row.innerHTML = '<input type="range" id="param_' + p.id + '" min="' + p.min + '" max="' + p.max + '" step="' + (p.step || 1) + '">' +
          '<span class="slider-val" id="pval_num_' + p.id + '"></span>';
        const slider = row.querySelector('input');
        slider.oninput = (e) => {
          const valLabel = document.getElementById('pval_num_' + p.id);
          if (valLabel) valLabel.textContent = e.target.value + (p.unit || '');
        };
        slider.onchange = (e) => setParam(currentEff.id, p.id, e.target.value);
        item.appendChild(row);
      }
      container.appendChild(item);
    });
  }

  // Update existing control values from state
  const effParams = (state.effects && state.effects[currentEff.id]) || {};
  currentEff.parameters.forEach(p => {
    const el = document.getElementById('param_' + p.id);
    if (!el) return;
    const val = effParams[p.id];
    if (val !== undefined && document.activeElement !== el) {
      if (p.kind === 'boolean') {
        el.checked = (val === 'ON' || val === true);
      } else if (p.kind === 'select') {
        el.value = val;
      } else {
        el.value = val;
        const valLabel = document.getElementById('pval_num_' + p.id);
        if (valLabel) valLabel.textContent = val + (p.unit || '');
      }
    }
  });
}

// Visualizer setup
const bandsContainer = document.getElementById('bandsGrid');
for (let i = 0; i < 16; i++) {
  const col = document.createElement('div');
  col.className = 'band-col';
  bandsContainer.appendChild(col);
}

async function pollPreview() {
  if (document.hidden || (previewMode() !== 'device' && !previewFallback)) return;
  try {
    const res = await fetch('/api/frame', { cache: 'no-store' });
    if (res.ok) {
      const buf = new Uint8Array(await res.arrayBuffer());
      if (buf.length === 3072) {
        for (let i = 0, j = 0; i < 3072; i += 3, j += 4) {
          previewImage.data[j] = buf[i];
          previewImage.data[j+1] = buf[i+1];
          previewImage.data[j+2] = buf[i+2];
          previewImage.data[j+3] = 255;
        }
        previewCtx.putImageData(previewImage, 0, 0);
      }
    }
  } catch(e) {}
}

async function previewLoop() {
  await pollPreview();
  setTimeout(previewLoop, document.hidden ? 2000 : 200);
}

async function audioPreviewLoop() {
  if (!document.hidden && previewMode() === 'browser' && state.audio_source === 'Mic') {
    try {
      Object.assign(state, await fetchJSON('/api/audio'));
      syncPreviewState();
    } catch (e) {} // Keep controls usable if audio telemetry is temporarily unavailable.
  }
  setTimeout(audioPreviewLoop, document.hidden ? 2000 : 100);
}

async function loop() {
  if (document.hidden) {
    setTimeout(loop, 2000);
    return;
  }
  try {
    const s = await fetchJSON('/api/state');
    state = s;
    previewConnected = true;
    syncPreviewState();
    document.getElementById('statusDot').classList.add('connected');
    document.getElementById('statusText').textContent = 'Online';
    updateUI();
  } catch (e) {
    previewConnected = false;
    document.getElementById('statusDot').classList.remove('connected');
    document.getElementById('statusText').textContent = 'Offline';
    document.getElementById('fpsDisplay').textContent = '-- FPS';
  }
  // Local animations stay smooth independently of telemetry/network latency.
  setTimeout(loop, 500);
}

async function init() {
  loadPreviewRenderer();
  requestAnimationFrame(animatePreview);
  previewLoop();
  audioPreviewLoop();
  try {
    let effects = await fetchJSON('/api/effects');
    if (effects && effects.effects) effects = effects.effects;
    if (Array.isArray(effects)) {
      catalog = effects;
      renderCatalog();
    }
  } catch (e) {}
  await loadPanelConfig();
  await loadMicData();
  loop();
}

window.addEventListener('DOMContentLoaded', init);
</script>
</body>
</html>
"##;
