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
.config-card { padding:0; overflow:hidden; }
.config-card > summary { cursor:pointer; list-style:none; padding:18px; display:flex; align-items:center; justify-content:space-between; gap:12px; }
.config-card > summary::-webkit-details-marker { display:none; }
.config-card > summary:after { content:'+'; color:var(--accent); font-size:1.4rem; }
.config-card[open] > summary:after { content:'−'; }
.config-title { display:block; font-weight:600; font-size:1rem; }
.config-subtitle, .config-help { display:block; color:var(--muted); font-size:0.78rem; line-height:1.5; margin-top:4px; }
.config-body { padding:0 18px 18px; }
.config-section { border-top:1px solid var(--border); padding:18px 0; }
.config-section h3 { font-size:0.9rem; margin-bottom:4px; }
.config-grid { display:grid; grid-template-columns:repeat(2,minmax(0,1fr)); gap:12px; margin-top:14px; }
.config-field { display:flex; flex-direction:column; gap:6px; font-size:0.8rem; color:var(--text); min-width:0; }
.config-field input, .config-field select { width:100%; min-width:0; background:var(--bg); border:1px solid var(--border); border-radius:6px; padding:9px; color:var(--text); font:inherit; }
.config-field input:focus, .config-field select:focus { outline:2px solid var(--accent); outline-offset:1px; }
.config-check { display:flex; align-items:center; gap:8px; font-size:0.82rem; margin-top:12px; }
.config-check input { accent-color:var(--accent); }
.panel-grid { display:grid; grid-template-columns:repeat(4,minmax(0,1fr)); gap:8px; margin-top:14px; }
.panel-tile { background:var(--bg); border:1px solid var(--border); border-radius:8px; padding:10px; min-width:0; }
.panel-tile strong { font-size:0.8rem; }
.panel-tile .config-field { margin-top:10px; }
.config-notice { background:rgba(210,153,34,.09); border:1px solid rgba(210,153,34,.3); border-radius:7px; padding:10px 12px; color:var(--text); font-size:0.78rem; line-height:1.5; }
@media (max-width:520px) { .panel-grid { grid-template-columns:repeat(2,minmax(0,1fr)); } }
.preview-wrap { display:flex; flex-direction:column; align-items:center; }
.matrix-preview { width:min(100%, 448px); height:auto; aspect-ratio:1; image-rendering:pixelated; border:2px solid var(--border); border-radius:4px; background:#000; }
.preview-controls { width:min(100%, 448px); margin-top:12px; }
.preview-controls label { display:block; font-size:0.85rem; margin-bottom:6px; color:var(--text); }
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
.playlist-list { display:grid; gap:8px; margin-top:12px; }
.playlist-entry { display:grid; grid-template-columns:minmax(0,1fr) auto; gap:8px; align-items:center; padding:10px; border:1px solid var(--border); border-radius:6px; background:var(--bg); }
.playlist-entry.current { border-color:var(--accent); }
.playlist-entry-info { display:grid; grid-template-columns:minmax(0,1fr) minmax(100px,140px); gap:8px; align-items:center; }
.playlist-entry-title { overflow-wrap:anywhere; }
.playlist-entry-duration { width:100%; min-width:0; padding:6px; background:var(--card-hover); border:1px solid var(--border); border-radius:5px; color:var(--text); }
.playlist-entry-actions { display:flex; gap:4px; }
.playlist-entry-actions .btn { padding:5px 8px; }
.playlist-status { color:var(--muted); font-size:0.82rem; line-height:1.5; margin-top:8px; }

/* Params */
.param-item { margin-bottom:12px; }
.param-item:last-child { margin-bottom:0; }
.param-label { display:flex; justify-content:space-between; font-size:0.85rem; margin-bottom:4px; }
select, .param-select { width:100%; background:var(--card-hover); color:var(--text); border:1px solid var(--border); border-radius:6px; padding:8px 32px 8px 10px; font-family:inherit; font-size:0.85rem; line-height:1.4; cursor:pointer; outline:none; appearance:none; -webkit-appearance:none; -moz-appearance:none; background-image:url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='16' height='16' viewBox='0 0 24 24' fill='none' stroke='%238b949e' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E"); background-repeat:no-repeat; background-position:right 10px center; transition:border-color 0.15s, box-shadow 0.15s; }
select:hover, .param-select:hover { border-color:var(--accent); }
select:focus, .param-select:focus { border-color:var(--accent); box-shadow:0 0 0 2px rgba(88,166,255,0.2); }
select:disabled, .param-select:disabled { opacity:0.5; cursor:not-allowed; }
select option, .param-select option { background:var(--card); color:var(--text); }

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
    <select id="previewMode" class="param-select" onchange="changePreviewMode()">
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

<!-- Custom effect playlist -->
<div class="card" id="playlistCard">
  <h2>Custom Effect Playlist</h2>
  <p class="config-help">Select effects by stable catalog ID and set each duration from 1 second to 24 hours. Saving the queue stops current playback. The device snapshot is authoritative for current entry and time remaining.</p>
  <div class="config-grid">
    <label class="config-field" for="playlistEffectSelect">Effect<select id="playlistEffectSelect"></select></label>
    <label class="config-field" for="playlistDurationInput">Duration (ms)<input type="number" id="playlistDurationInput" min="1000" max="86400000" step="1" value="30000" inputmode="numeric"></label>
  </div>
  <div class="btn-row"><button class="btn" id="playlistAddBtn" onclick="addPlaylistEntry()">Add effect</button></div>
  <div class="playlist-list" id="playlistEntries" aria-live="polite"></div>
  <label class="config-check"><input type="checkbox" id="playlistLoop"> Loop playlist</label>
  <div class="btn-row"><button class="btn btn-primary" id="playlistApplyBtn" onclick="applyPlaylistConfig()">Save queue</button></div>
  <div class="btn-row">
    <button class="btn btn-primary" id="playlistStartBtn" onclick="controlPlaylist('start')">Start</button>
    <button class="btn" id="playlistPauseBtn" onclick="controlPlaylist('pause')">Pause</button>
    <button class="btn" id="playlistResumeBtn" onclick="controlPlaylist('resume')">Resume</button>
    <button class="btn btn-danger" id="playlistStopBtn" onclick="controlPlaylist('stop')">Stop</button>
    <button class="btn" id="playlistPreviousBtn" onclick="controlPlaylist('previous')">Previous</button>
    <button class="btn" id="playlistNextBtn" onclick="controlPlaylist('next')">Next</button>
  </div>
  <div class="playlist-status" id="playlistCurrent" role="status">Current entry: —</div>
  <div class="playlist-status" id="playlistRemaining">Time remaining: —</div>
  <div class="playlist-status" id="playlistNotice" role="status" aria-live="polite"></div>
</div>

<!-- Dynamic Effect Parameters -->
<div class="card" id="paramsCard" style="display:none">
  <h2 id="paramsTitle">Effect Parameters</h2>
  <div id="paramsContainer"></div>
</div>

<!-- Audio -->
<div class="card">
  <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:12px;">
    <h2 style="margin:0">Audio Reactivity</h2>
    <button class="btn" id="audioSoundBtn" style="padding:4px 10px; font-size:0.8rem; display:none" onclick="toggleAudioPreviewSound()">🔊 Listen to Synth</button>
  </div>
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
    <label class="config-field" style="margin-top:12px">Microphone I2S channel
      <select id="micChannel" onchange="submitMicSetting('rightChannel', this.value === 'right')"><option value="left">Left</option><option value="right">Right</option></select>
    </label>
    <div style="margin-top:10px; padding:8px; background:var(--card-hover); border-radius:4px; font-size:0.8rem; display:flex; justify-content:space-between">
      <span>Raw: <strong id="micRawDisplay">--</strong> dBFS</span>
      <span>Applied Gain: <strong id="micGainDisplay">--</strong> dB</span>
      <span>Hardware: <strong id="micHwDisplay">--</strong></span>
    </div>
  </div>
</details>

<details class="card config-card" id="networkCard">
  <summary><span><span class="config-title">Wi-Fi and fallback access point</span><span class="config-subtitle">Configure prioritized station profiles and local setup access</span></span></summary>
  <div class="config-body">
    <p class="config-notice">The AP WPA2 passphrase protects local configuration; the UI has no separate login. Stored passphrases are never returned by the API. Blank fields keep existing secrets; use the explicit clear action to remove one. Changes trigger a controlled restart after save; reconnect at <code>http://192.168.4.1/</code>.</p>
    <div class="config-grid"><label class="config-field">New profile SSID<input id="networkSsid" maxlength="32" autocomplete="off"></label><label class="config-field">New profile password<input id="networkPassword" type="password" autocomplete="new-password"></label><label class="config-field">Priority<input id="networkPriority" type="number" min="0" max="255" value="0"></label><label class="config-check"><input id="networkHidden" type="checkbox"> Hidden SSID</label></div>
    <div class="btn-row"><button class="btn" id="networkAddProfileBtn" type="button">Add Wi-Fi profile</button></div>
    <div class="playlist-list" id="networkProfiles" aria-live="polite"></div>
    <div class="config-grid"><label class="config-field">Fallback AP SSID<input id="fallbackApSsid" maxlength="32"></label><label class="config-field">New AP password<input id="fallbackApPassword" type="password" autocomplete="new-password"></label></div>
    <label class="config-check"><input id="fallbackApPasswordClear" type="checkbox"> Clear saved AP password and disable fallback AP</label>
    <div class="btn-row"><button class="btn btn-primary" id="networkSaveBtn" type="button">Save Wi-Fi settings</button><span id="networkStatus" role="status" aria-live="polite"></span></div>
  </div>
</details>

<!-- Hardware configuration -->
<details class="card config-card" id="configCard">
  <summary><span><span class="config-title">Hardware configuration</span><span class="config-subtitle">Output pins, panel wiring &amp; power</span></span></summary>
  <div class="config-body">
    <div class="config-notice">Wiring and layout changes need a device restart. Check your board's pinout before saving: reserved, input-only or microphone pins cannot be used for LED output. Disabling an output only blacks out its LEDs; it does not disconnect or repair the panel electrically.</div>
    <section class="config-section">
      <h3>LED output</h3><span class="config-help">Choose a single data line or one output per panel. GPIO availability depends on your board.</span>
      <label class="config-check"><input type="checkbox" id="cfgParallelOutput" onchange="updateOutputMode()"> Use four parallel outputs</label>
      <div class="config-grid"><label class="config-field">Single output GPIO<input type="number" id="cfgDataPin" min="0" max="48" step="1"></label>
      <label class="config-field">Power limit (mA)<input type="number" id="cfgMaxMilliamps" min="1000" max="60000" step="1"></label></div>
      <span class="config-help">The power limit is applied live when possible. Microphone pins are fixed by the board profile.</span>
    </section>
    <section class="config-section">
      <h3>Panel mapping</h3><span class="config-help">Four 16 × 16 panels form a fixed 32 × 32 matrix. Panel numbers are 0–3.</span>
      <div class="config-grid">
        <label class="config-field">Start corner<select id="cfgStartCorner"><option value="0">Top left</option><option value="1">Top right</option><option value="2">Bottom left</option><option value="3">Bottom right</option></select></label>
        <label class="config-field">Wiring direction<select id="cfgPanelLayout"><option value="0">Horizontal</option><option value="1">Vertical</option></select></label>
        <label class="config-field">Panel order<input type="text" id="cfgPanelOrder" placeholder="0, 1, 2, 3"></label>
      </div>
      <label class="config-check"><input type="checkbox" id="cfgPanelSerpentine"> Serpentine panel arrangement</label>
      <label class="config-check"><input type="checkbox" id="cfgMirrorX"> Mirror horizontally</label>
      <div class="panel-grid" id="cfgPanels"></div>
      <span class="config-help">Rotation and pixel serpentine are per physical panel; GPIOs are used in parallel mode.</span>
    </section>
    <section class="config-section">
      <h3>Startup defaults</h3><span class="config-help">Used after a restart; saved runtime state can override brightness, effect and cycle settings. The LEDs always start off.</span>
      <div class="config-grid">
        <label class="config-field">Default brightness (0–255)<input type="number" id="cfgBootBrightness" min="0" max="255" step="1"></label>
        <label class="config-field">Default effect ID<input type="text" id="cfgBootEffect"></label>
        <label class="config-field">Auto-cycle interval (ms; 0 = off)<input type="number" id="cfgBootCycle" min="0" max="4294967295" step="1"></label>
        <label class="config-field">LFX asset path (within SPIFFS)<input type="text" id="cfgBootPath"></label>
      </div>
      <span class="config-help">LED protocol: WS2812B · Color order: GRB</span>
    </section>
    <div class="btn-row"><button class="btn btn-primary" id="cfgSaveBtn" onclick="savePanelConfig()" disabled>Save configuration</button><span id="cfgStatusMsg" role="status" aria-live="polite" style="font-size:0.8rem; color:var(--muted); align-self:center"></span></div>
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
    <div>Firmware: <strong id="firmwareVersion">--</strong></div>
    <div>OTA state: <strong id="firmwareOtaState">--</strong></div>
    <div>Partition: <strong id="firmwarePartition">--</strong></div>
    <div style="grid-column:1 / -1; overflow-wrap:anywhere">ELF SHA-256: <strong id="firmwareHash">--</strong></div>
  </div>
</details>

<!-- Persistence & Config -->
<div class="card">
  <h2>Runtime state &amp; device</h2>
  <p class="config-help" style="margin-bottom:10px">Save or clear the current runtime state separately from hardware configuration.</p>
  <div class="btn-row">
    <button class="btn btn-primary" onclick="saveState()">Save runtime state</button>
    <button class="btn btn-danger" onclick="clearState()">Clear runtime state</button>
    <button class="btn" id="restartBtn" onclick="restartDevice()">Restart Device</button>
  </div>
</div>

<div class="footer" id="footer">
  LED Matrix · ESP32/S3 & Host
</div>

<div id="statusToast"></div>

<script>
let state = {};
let catalog = [];
let playlistEntries = [];
let playlistDraftDirty = false;
let playlistLoopDraft = false;
let playlistAwaitingSnapshot = false;
let playlistStatePersisting = false;
let playlistSaveVersion = 0;
let playlistPendingPersistence = null;
let playlistPersistenceNotice = '';
let lastRenderedPlaylistView = '';
const MAX_PLAYLIST_ENTRIES = 32;
const MIN_PLAYLIST_ENTRY_MS = 1000;
const MAX_PLAYLIST_ENTRY_MS = 86400000;
let loadedConfig = {};
let configRestartPending = false;
let micSettings = {};
const previewCtx = document.getElementById('previewCanvas').getContext('2d');
const previewImage = previewCtx.createImageData(32, 32);
const previewEncoder = new TextEncoder();
let previewRenderer = null;
let previewFallback = false;
let previewLoading = false;
let previewClockOffset = null;
let lastSeenRenderTimeMs = -1;
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
    const input = previewRenderer.preview_input();
    new Uint8Array(previewRenderer.memory.buffer, input, bytes.length).set(bytes);
    previewFallback = previewRenderer.preview_sync(bytes.length) !== 0;
    previewReady = !previewFallback;
    previewConnected = true;
    const serverTime = Number(state.renderTimeMs || 0);
    if (serverTime > 0) {
      const now = performance.now();
      if (previewClockOffset === null || serverTime < lastSeenRenderTimeMs || Math.abs(serverTime - (now + previewClockOffset)) > 2000) {
        previewClockOffset = serverTime - now;
        lastSeenRenderTimeMs = serverTime;
      } else if (serverTime > lastSeenRenderTimeMs) {
        const targetOffset = serverTime - now;
        previewClockOffset += (targetOffset - previewClockOffset) * 0.1;
        lastSeenRenderTimeMs = serverTime;
      }
    }
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

let micPlaybackHistory = [];
let micDisplayedBands = Array(16).fill(0);
let micTargetBands = Array(16).fill(0);
let micReceivedAt = 0;
let micPaintAt = 0;
function queueMicHistory(s) {
  if (s.audio_source !== 'Mic') {
    micPlaybackHistory = [];
    micDisplayedBands.fill(0);
    micTargetBands.fill(0);
    micPaintAt = 0;
    return;
  }
  const history = Array.isArray(s.audioHistory) ? s.audioHistory : [];
  const latest = history.filter(x => Array.isArray(x) && Array.isArray(x[1]) && x[1].length === 16).pop();
  const bands = latest ? latest[1].map(v => v / 255) : s.bands;
  if (!Array.isArray(bands) || bands.length !== 16) return;
  micTargetBands = bands.map(v => Number.isFinite(v) ? Math.max(0, Math.min(1, v)) : 0);
  micPlaybackHistory = [micTargetBands];
  micReceivedAt = performance.now();
}
function animateMicBands(now) {
  if (document.hidden || state.audio_source !== 'Mic' || !micPlaybackHistory.length) {
    micPaintAt = 0;
    return;
  }
  const dt = micPaintAt ? Math.max(0, Math.min(100, now - micPaintAt)) : 16;
  micPaintAt = now;
  // Only fade after updates have genuinely gone stale.
  const decay = Math.exp(-Math.max(0, now - micReceivedAt - 1000) / 250);
  // Audio updates arrive about every 200 ms; smooth between those updates.
  const blend = 1 - Math.exp(-dt / 100);
  const cols = document.querySelectorAll('.band-col');
  for (let i = 0; i < 16; i++) {
    micDisplayedBands[i] += (micTargetBands[i] * decay - micDisplayedBands[i]) * blend;
    if (cols[i]) cols[i].style.height = (micDisplayedBands[i] * 100) + '%';
  }
}

function animatePreview(now) {
  animateMicBands(now);
  if (!document.hidden && previewMode() === 'browser' && previewReady && previewConnected) {
    try {
      previewLastTime = Math.max(previewLastTime, now + previewClockOffset, 0);
      const ptr = previewRenderer.preview_render(previewLastTime);
      previewImage.data.set(new Uint8Array(previewRenderer.memory.buffer, ptr, 4096));
      previewCtx.putImageData(previewImage, 0, 0);

      if (typeof previewRenderer.preview_audio_bands === 'function' && state.audio_source === 'Synthetic' && typeof document.querySelectorAll === 'function') {
        const bandsPtr = previewRenderer.preview_audio_bands();
        if (bandsPtr && previewRenderer.memory) {
          const bands = new Float32Array(previewRenderer.memory.buffer, bandsPtr, 16);
          const cols = document.querySelectorAll('.band-col');
          for (let i = 0; i < 16 && i < cols.length; i++) {
            if (cols[i] && cols[i].style) {
              const h = Math.min(100, Math.max(0, bands[i] * 100));
              cols[i].style.height = h.toFixed(1) + '%';
            }
          }
        }
      }
    } catch (e) {
      previewReady = false;
      previewStatus('Browser renderer stopped. Select Device frames for the ESP preview.');
    }
  }
  requestAnimationFrame(animatePreview);
}

let audioPreviewCtx = null;
let audioPreviewNode = null;
let audioPreviewPlaying = false;
let audioPreviewTime = 0;

function toggleAudioPreviewSound() {
  if (audioPreviewPlaying) {
    stopAudioPreviewSound();
  } else {
    startAudioPreviewSound();
  }
}

function startAudioPreviewSound() {
  if (!previewRenderer || typeof previewRenderer.preview_audio_pcm !== 'function') return;
  try {
    const AudioCtx = window.AudioContext || window.webkitAudioContext;
    if (!AudioCtx) return;
    if (!audioPreviewCtx) {
      audioPreviewCtx = new AudioCtx();
    }
    if (audioPreviewCtx.state === 'suspended') {
      audioPreviewCtx.resume();
    }
    const sampleRate = audioPreviewCtx.sampleRate;
    const bufferSize = 2048;
    audioPreviewNode = audioPreviewCtx.createScriptProcessor(bufferSize, 0, 1);
    audioPreviewTime = (performance.now() + (previewClockOffset || 0)) / 1000.0;
    audioPreviewNode.onaudioprocess = function(e) {
      if (!audioPreviewPlaying || !previewRenderer || typeof previewRenderer.preview_audio_pcm !== 'function') return;
      const out = e.outputBuffer.getChannelData(0);
      const ptr = previewRenderer.preview_audio_pcm(audioPreviewTime, out.length, sampleRate);
      if (ptr && previewRenderer.memory) {
        const wasmSamples = new Float32Array(previewRenderer.memory.buffer, ptr, out.length);
        out.set(wasmSamples);
      }
      audioPreviewTime += out.length / sampleRate;
    };
    audioPreviewNode.connect(audioPreviewCtx.destination);
    audioPreviewPlaying = true;
    const btn = document.getElementById('audioSoundBtn');
    if (btn) {
      btn.textContent = '⏹ Stop Sound';
      btn.style.borderColor = 'var(--accent)';
      btn.style.color = 'var(--accent)';
    }
  } catch (e) {
    console.error('Audio preview sound error:', e);
  }
}

function stopAudioPreviewSound() {
  audioPreviewPlaying = false;
  if (audioPreviewNode) {
    try { audioPreviewNode.disconnect(); } catch (e) {}
    audioPreviewNode = null;
  }
  const btn = document.getElementById('audioSoundBtn');
  if (btn) {
    btn.textContent = '🔊 Listen to Synth';
    btn.style.borderColor = '';
    btn.style.color = '';
  }
}

async function restartDevice() {
  if (!confirm('Restart device now? Unsaved runtime changes may be lost.')) return;
  const button = document.getElementById('restartBtn');
  button.disabled = true;
  try {
    const response = await fetch('/api/reboot', { method: 'POST', headers: { 'X-Matrix-Control': '1' } });
    if (!response.ok) throw new Error('Restart rejected');
    showToast('Restarting device… Reconnecting shortly.');
    setTimeout(() => window.location.reload(), 8000);
  } catch (error) {
    button.disabled = false;
    showToast('Restart failed: ' + error.message);
  }
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

async function loadFirmwareInfo() {
  const set = (id, value) => { document.getElementById(id).textContent = value; };
  try {
    const info = await fetchJSON('/api/firmware');
    set('firmwareVersion', info.version || 'Unknown');
    set('firmwareHash', info.elfSha256 || '--');
    const address = Number.isInteger(info.partitionAddress) ? ' (0x' + info.partitionAddress.toString(16) + ')' : '';
    set('firmwarePartition', (info.partition || 'Unknown') + address);
    set('firmwareOtaState', info.otaState || 'Unknown');
  } catch (e) {
    set('firmwareVersion', 'Unavailable');
    set('firmwareHash', '--');
    set('firmwarePartition', '--');
    set('firmwareOtaState', '--');
  }
}

async function postCommand(kind, payload) {
  let pending = false;
  try {
    const r = await fetch('/api/command', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ kind, payload })
    });
    const data = await r.json();
    if (r.status === 202 || data.pending) {
      pending = true;
      showToast('Command queued (processing)...', 'var(--yellow)');
      setTimeout(clearToast, 1200);
    } else if (!r.ok) {
      showToast('Error: ' + (data.error || r.statusText), 'var(--red)');
      setTimeout(clearToast, 3500);
    } else {
      clearToast();
    }
    return { ...data, pending: pending || !!data.pending };
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

function playlistEffect(effectId) {
  return catalog.find(effect => effect.id === effectId);
}

async function loadNetworkConfig() {
  const status = document.getElementById('networkStatus');
  try {
    const response = await fetch('/api/network', { cache: 'no-store' });
    const value = await response.json();
    if (!response.ok) throw new Error(value.error || response.statusText);
    const profiles = Array.isArray(value.wifiProfiles) ? value.wifiProfiles : [];
    state.networkConfig = value;
    document.getElementById('fallbackApSsid').value = value.fallbackApSsid || 'JDJLIGHT';
    document.getElementById('fallbackApPasswordClear').checked = false;
    renderNetworkProfiles();
    status.textContent = value.networkAvailable === false
      ? 'Wi-Fi configuration loaded; hardware networking unavailable in this host adapter.'
      : profiles.length
        ? 'Loaded ' + profiles.length + ' saved Wi-Fi profile(s); credentials remain masked.'
        : value.fallbackApEnabled === false
          ? 'No saved Wi-Fi profiles. Fallback AP is disabled until an AP passphrase is set.'
          : 'No saved Wi-Fi profiles. Fallback AP is enabled; AP credentials remain masked.';
  } catch (error) {
    status.textContent = 'Wi-Fi configuration unavailable: ' + String(error.message || error);
  }
}

function renderNetworkProfiles() {
  const container = document.getElementById('networkProfiles');
  const config = state.networkConfig || {};
  const profiles = Array.isArray(config.wifiProfiles) ? config.wifiProfiles : [];
  while (container.firstChild) container.removeChild(container.firstChild);
  profiles.forEach((profile, index) => {
    const row = document.createElement('div');
    row.className = 'playlist-entry';
    const label = document.createElement('span');
    label.className = 'playlist-entry-title';
    label.textContent = profile.ssid + ' · priority ' + profile.priority + (profile.hidden ? ' · hidden' : '') + (profile.passwordConfigured ? ' · password saved' : ' · open network');
    row.appendChild(label);
    const actions = document.createElement('div');
    actions.className = 'playlist-entry-actions';
    const button = (text, title, action) => {
      const item = document.createElement('button');
      item.type = 'button'; item.className = 'btn'; item.textContent = text; item.title = title; item.setAttribute('aria-label', title); item.onclick = action;
      return item;
    };
    actions.appendChild(button('↑', 'Move Wi-Fi profile up', () => moveNetworkProfile(index, -1)));
    actions.appendChild(button('↓', 'Move Wi-Fi profile down', () => moveNetworkProfile(index, 1)));
    actions.appendChild(button('Edit', 'Edit Wi-Fi profile', () => editNetworkProfile(index)));
    actions.appendChild(button('Remove', 'Remove Wi-Fi profile', () => { profiles.splice(index, 1); renderNetworkProfiles(); }));
    row.appendChild(actions); container.appendChild(row);
  });
}

function moveNetworkProfile(index, direction) {
  const profiles = state.networkConfig && state.networkConfig.wifiProfiles;
  const target = index + direction;
  if (!Array.isArray(profiles) || target < 0 || target >= profiles.length) return;
  [profiles[index], profiles[target]] = [profiles[target], profiles[index]];
  profiles.forEach((profile, position) => { profile.priority = profiles.length - position; });
  renderNetworkProfiles();
}

function editNetworkProfile(index) {
  const profile = state.networkConfig.wifiProfiles[index];
  if (!profile) return;
  document.getElementById('networkSsid').value = profile.ssid;
  document.getElementById('networkPassword').value = '';
  document.getElementById('networkPriority').value = String(profile.priority);
  document.getElementById('networkHidden').checked = !!profile.hidden;
  document.getElementById('networkSsid').dataset.editIndex = String(index);
}

async function saveNetworkConfig() {
  const status = document.getElementById('networkStatus');
  const config = state.networkConfig || { wifiProfiles: [] };
  const password = document.getElementById('fallbackApPassword').value;
  const networkPassword = document.getElementById('networkPassword').value;
  const payload = {
    wifiProfiles: (config.wifiProfiles || []).map(profile => {
      const { passwordConfigured, ...safeProfile } = profile;
      if (passwordConfigured && !safeProfile.password && !safeProfile.clearPassword) safeProfile.keepPassword = true;
      return safeProfile;
    }),
    fallbackApSsid: document.getElementById('fallbackApSsid').value,
    mqttHost: config.mqttHost || '', mqttPort: config.mqttPort || 1883,
    mqttUser: config.mqttUser || '',
  };
  if (password) payload.fallbackApPassword = password;
  if (networkPassword) {
    const ssid = document.getElementById('networkSsid').value;
    const index = Number(document.getElementById('networkSsid').dataset.editIndex);
    let target = Number.isInteger(index) && index >= 0 ? payload.wifiProfiles[index] : null;
    if (!target) {
      const priority = Number(document.getElementById('networkPriority').value);
      const hidden = document.getElementById('networkHidden').checked;
      target = { ssid, priority, hidden };
      payload.wifiProfiles.push(target);
    }
    if (!ssid.trim() || new TextEncoder().encode(ssid).length > 32 || new TextEncoder().encode(networkPassword).length < 8 || !Number.isInteger(target.priority) || target.priority < 0 || target.priority > 255) {
      status.textContent = 'Finish the Wi-Fi profile fields with a valid SSID, password, and priority before saving.';
      return;
    }
    target.password = networkPassword;
    delete target.keepPassword;
  }
  if (document.getElementById('fallbackApPasswordClear').checked) payload.clearFallbackApPassword = true;
  try {
    const response = await fetch('/api/network', { method: 'POST', headers: { 'Content-Type': 'application/json', 'X-Matrix-Control': '1' }, body: JSON.stringify(payload) });
    const result = await response.json();
    if (!response.ok) throw new Error(result.error || response.statusText);
    state.networkConfig = result.config;
    document.getElementById('fallbackApPassword').value = '';
    document.getElementById('networkPassword').value = '';
    document.getElementById('networkSsid').value = '';
    delete document.getElementById('networkSsid').dataset.editIndex;
    document.getElementById('fallbackApPasswordClear').checked = false;
    status.textContent = 'Saved. The device will restart; reconnect at ' + (result.address || 'http://192.168.4.1/');
    renderNetworkProfiles();
  } catch (error) { status.textContent = 'Save failed: ' + String(error.message || error); }
}

function addNetworkProfile() {
  const ssidEl = document.getElementById('networkSsid');
  const ssid = ssidEl.value;
  const priority = Number(document.getElementById('networkPriority').value);
  const password = document.getElementById('networkPassword').value;
  const hidden = document.getElementById('networkHidden').checked;
  if (!ssid.trim() || new TextEncoder().encode(ssid).length > 32 || !Number.isInteger(priority) || priority < 0 || priority > 255 || (password && new TextEncoder().encode(password).length < 8)) {
    document.getElementById('networkStatus').textContent = 'SSID must be 1–32 UTF-8 bytes, priority 0–255, password empty or at least 8 bytes.';
    return;
  }
  const config = state.networkConfig || (state.networkConfig = { wifiProfiles: [] });
  if (!Array.isArray(config.wifiProfiles)) config.wifiProfiles = [];
  const editIndex = Number(ssidEl.dataset.editIndex);
  if ((!Number.isInteger(editIndex) || editIndex < 0) && (config.wifiProfiles.length >= 5 || config.wifiProfiles.some(profile => profile.ssid === ssid))) {
    document.getElementById('networkStatus').textContent = config.wifiProfiles.length >= 5 ? 'At most five Wi-Fi profiles are supported.' : 'That SSID is already configured; edit its existing entry.';
    return;
  }
  const profile = { ssid, priority, hidden };
  if (password) profile.password = password;
  if (Number.isInteger(editIndex) && editIndex >= 0 && editIndex < config.wifiProfiles.length) {
    if (!password && config.wifiProfiles[editIndex].passwordConfigured) profile.keepPassword = true;
    config.wifiProfiles[editIndex] = profile;
    delete ssidEl.dataset.editIndex;
  } else config.wifiProfiles.push(profile);
  ssidEl.value = ''; document.getElementById('networkPassword').value = '';
  renderNetworkProfiles();
}


function renderPlaylistEntries(snapshot) {
  const container = document.getElementById('playlistEntries');
  const currentIndex = !playlistDraftDirty && snapshot && Number.isInteger(snapshot.index) ? snapshot.index : -1;
  const view = JSON.stringify({ entries: playlistEntries, currentIndex });
  if (view === lastRenderedPlaylistView) return;
  lastRenderedPlaylistView = view;
  while (container.firstChild) container.removeChild(container.firstChild);
  playlistEntries.forEach((entry, index) => {
    const row = document.createElement('div');
    row.className = 'playlist-entry' + (currentIndex === index ? ' current' : '');
    const label = document.createElement('span');
    label.className = 'playlist-entry-title';
    const effect = playlistEffect(entry.effectId);
    label.textContent = effect ? effect.name : entry.effectId;
    const info = document.createElement('div');
    info.className = 'playlist-entry-info';
    info.appendChild(label);
    const duration = document.createElement('input');
    duration.type = 'number';
    duration.className = 'playlist-entry-duration';
    duration.min = String(MIN_PLAYLIST_ENTRY_MS);
    duration.max = String(MAX_PLAYLIST_ENTRY_MS);
    duration.step = '1';
    duration.value = String(entry.durationMs);
    duration.setAttribute('aria-label', 'Duration in milliseconds');
    duration.onchange = event => setPlaylistDuration(index, event.target.value, event.target);
    info.appendChild(duration);
    row.appendChild(info);

    const actions = document.createElement('div');
    actions.className = 'playlist-entry-actions';
    const button = (text, title, action) => {
      const item = document.createElement('button');
      item.type = 'button';
      item.className = 'btn';
      item.textContent = text;
      item.title = title;
      item.setAttribute('aria-label', title);
      item.onclick = action;
      return item;
    };
    actions.appendChild(button('↑', 'Move entry up', () => movePlaylistEntry(index, -1)));
    actions.appendChild(button('↓', 'Move entry down', () => movePlaylistEntry(index, 1)));
    actions.appendChild(button('−', 'Remove entry', () => removePlaylistEntry(index)));
    row.appendChild(actions);
    container.appendChild(row);
  });
  if (!playlistEntries.length) {
    const empty = document.createElement('p');
    empty.className = 'playlist-status';
    empty.textContent = 'Queue is empty. Add an effect to create a playlist.';
    container.appendChild(empty);
  }
}

function addPlaylistEntry() {
  const select = document.getElementById('playlistEffectSelect');
  const duration = document.getElementById('playlistDurationInput').value.trim();
  const effectId = select.value;
  const notice = document.getElementById('playlistNotice');
  if (!effectId || !playlistEffect(effectId)) {
    notice.textContent = 'Choose a valid catalog effect.';
    return;
  }
  if (!/^\d+$/.test(duration) || !Number.isSafeInteger(Number(duration)) || Number(duration) < MIN_PLAYLIST_ENTRY_MS || Number(duration) > MAX_PLAYLIST_ENTRY_MS) {
    notice.textContent = 'Duration must be a whole number from 1000 to 86400000 ms.';
    return;
  }
  if (playlistEntries.length >= MAX_PLAYLIST_ENTRIES) {
    notice.textContent = 'A playlist can contain at most 32 entries.';
    return;
  }
  playlistEntries.push({ effectId, durationMs: Number(duration) });
  playlistSaveVersion++;
  playlistPersistenceNotice = '';
  document.getElementById('playlistNotice').textContent = '';
  playlistDraftDirty = true;
  playlistAwaitingSnapshot = false;
  notice.textContent = '';
  updatePlaylistUI();
}

function removePlaylistEntry(index) {
  if (!Number.isInteger(index) || index < 0 || index >= playlistEntries.length) return;
  playlistEntries.splice(index, 1);
  playlistSaveVersion++;
  playlistPersistenceNotice = '';
  playlistDraftDirty = true;
  playlistAwaitingSnapshot = false;
  updatePlaylistUI();
}

function movePlaylistEntry(index, offset) {
  const target = index + offset;
  if (!Number.isInteger(index) || !Number.isInteger(offset) || target < 0 || target >= playlistEntries.length) return;
  [playlistEntries[index], playlistEntries[target]] = [playlistEntries[target], playlistEntries[index]];
  playlistSaveVersion++;
  playlistPersistenceNotice = '';
  playlistDraftDirty = true;
  playlistAwaitingSnapshot = false;
  updatePlaylistUI();
}

function setPlaylistDuration(index, value, input) {
  const entry = playlistEntries[index];
  if (!entry) return;
  const trimmed = String(value).trim();
  const notice = document.getElementById('playlistNotice');
  if (!/^\d+$/.test(trimmed) || !Number.isSafeInteger(Number(trimmed)) || Number(trimmed) < MIN_PLAYLIST_ENTRY_MS || Number(trimmed) > MAX_PLAYLIST_ENTRY_MS) {
    notice.textContent = 'Duration must be a whole number from 1000 to 86400000 ms.';
    if (input) input.value = String(entry.durationMs);
    return;
  }
  entry.durationMs = Number(trimmed);
  playlistSaveVersion++;
  playlistPersistenceNotice = '';
  playlistDraftDirty = true;
  playlistAwaitingSnapshot = false;
  notice.textContent = '';
  lastRenderedPlaylistView = '';
  updatePlaylistUI();
}

async function applyPlaylistConfig() {
  const notice = document.getElementById('playlistNotice');
  playlistLoopDraft = document.getElementById('playlistLoop').checked;
  if (playlistEntries.length > MAX_PLAYLIST_ENTRIES || playlistEntries.some(entry =>
    !playlistEffect(entry.effectId) || !Number.isSafeInteger(entry.durationMs) || entry.durationMs < MIN_PLAYLIST_ENTRY_MS || entry.durationMs > MAX_PLAYLIST_ENTRY_MS)) {
    notice.textContent = 'Queue contains an invalid effect or duration. Correct the queue before saving.';
    return;
  }
  const submittedEntries = playlistEntries.map(entry => ({ effectId: entry.effectId, durationMs: entry.durationMs }));
  const submittedLoop = playlistLoopDraft;
  const submittedVersion = playlistSaveVersion;
  try {
    const result = await postCommand('playlist_config', { entries: submittedEntries, loop: submittedLoop });
    if (result && result.error) throw new Error(result.error);
    if (submittedVersion !== playlistSaveVersion) {
      playlistPersistenceNotice = 'Queue changed while save was pending; save the current draft again.';
      playlistDraftDirty = true;
      playlistAwaitingSnapshot = false;
      document.getElementById('playlistLoop').checked = playlistLoopDraft;
      updatePlaylistUI();
      return;
    }
    playlistDraftDirty = true;
    playlistAwaitingSnapshot = true;
    playlistPersistenceNotice = '';
    notice.textContent = 'Queue command accepted' + (result && result.pending ? ' (queued)' : '') + '; waiting for device snapshot.';
    updatePlaylistUI();
  } catch (error) {
    notice.textContent = 'Queue save failed.';
  }
}

async function controlPlaylist(action) {
  if (!['start', 'pause', 'resume', 'stop', 'next', 'previous'].includes(action)) return;
  const notice = document.getElementById('playlistNotice');
  try {
    const result = await postCommand('playlist_control', { action });
    if (result && result.error) throw new Error(result.error);
    playlistPersistenceNotice = '';
    notice.textContent = '';
  } catch (error) {
    playlistPersistenceNotice = '';
    notice.textContent = 'Playlist ' + action + ' failed.';
  }
}

function formatPlaylistTime(value) {
  if (!Number.isSafeInteger(value) || value < 0) return '—';
  const totalSeconds = Math.ceil(value / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  return hours ? hours + 'h ' + String(minutes).padStart(2, '0') + 'm ' + String(seconds).padStart(2, '0') + 's'
    : minutes ? minutes + 'm ' + String(seconds).padStart(2, '0') + 's' : seconds + 's';
}

function playlistSnapshotMatchesDraft(snapshot, entries = playlistEntries, loop = playlistLoopDraft) {
  if (!snapshot || !Array.isArray(snapshot.entries) || snapshot.entries.length !== entries.length || loop !== !!snapshot.loop) return false;
  return entries.every((entry, index) => {
    const saved = snapshot.entries[index];
    return !!saved && entry.effectId === saved.effectId && entry.durationMs === saved.durationMs;
  });
}

async function persistSavedPlaylistSnapshot(snapshot, entries = playlistEntries, loop = playlistLoopDraft, version = playlistSaveVersion) {
  if (!playlistSnapshotMatchesDraft(snapshot, entries, loop)) return;
  if (playlistStatePersisting) {
    playlistPendingPersistence = { snapshot, entries, loop, version };
    return;
  }
  playlistStatePersisting = true;
  const notice = document.getElementById('playlistNotice');
  updatePlaylistUI();
  let succeeded = false;
  try {
    const response = await fetch('/api/state/save', { method: 'POST' });
    const result = await response.json();
    if (!response.ok) throw new Error(result.error || response.statusText);
    succeeded = true;
    if (version === playlistSaveVersion && playlistSnapshotMatchesDraft(snapshot, entries, loop)) {
      playlistPersistenceNotice = 'Queue saved to device storage. Playback remains stopped until Start.';
    }
  } catch (error) {
    if (version === playlistSaveVersion) {
      playlistPersistenceNotice = 'Queue is active but could not be saved to device storage. Use Save runtime state to retry.';
    }
  } finally {
    playlistStatePersisting = false;
    const pending = playlistPendingPersistence;
    playlistPendingPersistence = null;
    if (pending && pending.version === playlistSaveVersion && playlistSnapshotMatchesDraft(pending.snapshot, pending.entries, pending.loop)) {
      persistSavedPlaylistSnapshot(pending.snapshot, pending.entries, pending.loop, pending.version);
    } else if (succeeded && version === playlistSaveVersion && !playlistPersistenceNotice) {
      playlistPersistenceNotice = 'Queue save completed, but the current draft changed; save the current queue again.';
    }
    updatePlaylistUI();
  }
}

function updatePlaylistUI() {
  const snapshot = state.playlist;
  if (!snapshot || !Array.isArray(snapshot.entries)) {
    document.getElementById('playlistCurrent').textContent = 'Current entry: unavailable';
    document.getElementById('playlistRemaining').textContent = 'Time remaining: unavailable';
    return;
  }
  if (!playlistDraftDirty) {
    playlistEntries = snapshot.entries.map(entry => ({ effectId: entry.effectId, durationMs: entry.durationMs }));
    playlistLoopDraft = !!snapshot.loop;
    document.getElementById('playlistLoop').checked = playlistLoopDraft;
    playlistDraftDirty = false;
    playlistAwaitingSnapshot = false;
  } else if (playlistSnapshotMatchesDraft(snapshot)) {
    const shouldPersist = playlistAwaitingSnapshot;
    playlistDraftDirty = false;
    playlistAwaitingSnapshot = false;
    document.getElementById('playlistLoop').checked = playlistLoopDraft;
    if (shouldPersist) {
      const savedEntries = snapshot.entries.map(entry => ({ effectId: entry.effectId, durationMs: entry.durationMs }));
      const savedLoop = !!snapshot.loop;
      const savedVersion = playlistSaveVersion;
      persistSavedPlaylistSnapshot(snapshot, savedEntries, savedLoop, savedVersion);
    }
  }
  renderPlaylistEntries(snapshot);
  const index = Number.isInteger(snapshot.index) ? snapshot.index : -1;
  const effectId = snapshot.effectId || (index >= 0 && snapshot.entries[index] && snapshot.entries[index].effectId);
  const effect = effectId && playlistEffect(effectId);
  const status = String(snapshot.status || 'stopped');
  document.getElementById('playlistCurrent').textContent = index >= 0
    ? 'Current entry: ' + (index + 1) + ' of ' + snapshot.entries.length + ' · ' + (effect ? effect.name : (effectId || 'unknown')) + ' (' + status + ')'
    : 'Current entry: none (' + status + ')' + (status === 'stopped' && state.effect_id
      ? ' · active effect: ' + ((playlistEffect(state.effect_id) || {}).name || state.effect_id) : '');
  document.getElementById('playlistRemaining').textContent = 'Time remaining: ' + formatPlaylistTime(snapshot.remainingMs);

  const cycleOn = state.cycle === 'ON';
  const notice = document.getElementById('playlistNotice');
  const messages = [];
  if (cycleOn) messages.push('Auto Cycle is enabled and takes control from the playlist; starting this playlist disables Auto Cycle.');
  if (playlistDraftDirty) messages.push(playlistAwaitingSnapshot
    ? 'Queue save pending; waiting for device snapshot before playback controls are enabled.'
    : 'Unsaved queue changes; save the queue before using playback controls.');
  if (playlistStatePersisting) messages.push('Queue matched the device snapshot; saving it to persistent device storage.');
  if (!playlistDraftDirty && status === 'stopped' && snapshot.entries.length && state.effect_id && snapshot.entries.some(entry => entry.effectId === state.effect_id))
    messages.push('Playback is stopped; the current effect matches a queued entry. Start the playlist to play the queue.');
  else if (!playlistDraftDirty && status === 'stopped' && snapshot.entries.length && state.effect_id)
    messages.push('Playback stopped; manual effect selection is active and is not part of this queue.');
  if (status === 'running' && index < 0) messages.push('Playlist status is inconsistent: running without a current entry.');
  if (playlistPersistenceNotice) messages.unshift(playlistPersistenceNotice);
  if (messages.length) notice.textContent = messages.join(' ');
  else if (playlistPersistenceNotice) notice.textContent = playlistPersistenceNotice;
  else if (notice.textContent.startsWith('Queue save failed') || notice.textContent.startsWith('Playlist ') || notice.textContent.startsWith('Queue command accepted')) {
    // Preserve a transient command status until replaced by the next operation.
  } else notice.textContent = '';
  document.getElementById('playlistStartBtn').disabled = playlistDraftDirty || status === 'running' || !snapshot.entries.length;
  document.getElementById('playlistPauseBtn').disabled = playlistDraftDirty || status !== 'running';
  document.getElementById('playlistResumeBtn').disabled = playlistDraftDirty || status !== 'paused';
  document.getElementById('playlistStopBtn').disabled = playlistDraftDirty || status === 'stopped';
  document.getElementById('playlistPreviousBtn').disabled = playlistDraftDirty || !['running', 'paused'].includes(status);
  document.getElementById('playlistNextBtn').disabled = playlistDraftDirty || !['running', 'paused'].includes(status);
}

function setParam(effectId, paramId, val) {
  postCommand('effect/' + effectId + '/' + paramId, String(val));
}

async function saveState() {
  const response = await fetch('/api/state/save', { method: 'POST' });
  if (!response.ok) { showToast('Save failed: ' + response.status, 'var(--red)'); return false; }
  showToast('Settings saved to flash', 'var(--green)');
  setTimeout(clearToast, 2000);
  return true;
}

async function clearState() {
  if (confirm('Clear saved settings?')) {
    const response = await fetch('/api/state/clear', { method: 'POST' });
    if (!response.ok) { showToast('Clear failed: ' + response.status, 'var(--red)'); return; }
    showToast('Saved settings cleared', 'var(--muted)');
    setTimeout(clearToast, 2000);
  }
}

function updateOutputMode() {
  const parallel = document.getElementById('cfgParallelOutput').checked;
  document.getElementById('cfgDataPin').disabled = parallel;
  for (let i = 0; i < 4; i++) document.getElementById('cfgPin' + i).disabled = !parallel;
}

async function loadPanelConfig() {
  const msg = document.getElementById('cfgStatusMsg');
  try {
    const cfg = await fetchJSON('/api/config');
    loadedConfig = cfg;
    const panels = document.getElementById('cfgPanels');
    panels.innerHTML = Array.from({length: 4}, (_, i) =>
      '<div class="panel-tile"><strong>Panel ' + i + '</strong>' +
      '<label class="config-field">GPIO<input type="number" min="0" max="48" step="1" id="cfgPin' + i + '"></label>' +
      '<label class="config-field">Rotation<select id="cfgRotation' + i + '"><option value="0">0°</option><option value="90">90°</option><option value="180">180°</option><option value="270">270°</option></select></label>' +
      '<label class="config-check"><input type="checkbox" id="cfgSerpentine' + i + '"> Zigzag</label>' +
      '<label class="config-check"><input type="checkbox" id="cfgDisabledPanel' + i + '"> Disable output</label></div>').join('');
    document.getElementById('cfgStartCorner').value = cfg.startCorner;
    document.getElementById('cfgPanelLayout').value = cfg.panelLayout;
    document.getElementById('cfgPanelOrder').value = cfg.panelOrder.join(', ');
    document.getElementById('cfgDataPin').value = cfg.dataPin;
    document.getElementById('cfgParallelOutput').checked = cfg.parallelOutput;
    document.getElementById('cfgPanelSerpentine').checked = cfg.panelSerpentine;
    document.getElementById('cfgMirrorX').checked = cfg.mirrorX;
    document.getElementById('cfgMaxMilliamps').value = cfg.ledMaxMilliamps;
    document.getElementById('cfgBootBrightness').value = cfg.ledBrightness;
    document.getElementById('cfgBootEffect').value = cfg.defaultAnimation;
    document.getElementById('cfgBootCycle').value = cfg.autoCycleMs;
    document.getElementById('cfgBootPath').value = cfg.fsAnimationPath;
    for (let i = 0; i < 4; i++) {
      document.getElementById('cfgPin' + i).value = cfg.panelPins[i];
      document.getElementById('cfgRotation' + i).value = cfg.panelRotation[i];
      document.getElementById('cfgSerpentine' + i).checked = cfg.serpentine[i];
      document.getElementById('cfgDisabledPanel' + i).checked = !!cfg.disabledPanels?.[i];
      document.getElementById('cfgPin' + i).disabled = !cfg.parallelOutput;
    }
    updateOutputMode();
    document.getElementById('cfgSaveBtn').disabled = false;
    msg.textContent = '';
  } catch(e) {
    document.getElementById('cfgSaveBtn').disabled = true;
    msg.textContent = 'Could not load configuration. Reload the page to retry.';
    msg.style.color = 'var(--red)';
  }
}

async function savePanelConfig() {
  const msg = document.getElementById('cfgStatusMsg');
  const number = (id, min, max) => {
    const value = document.getElementById(id).value.trim();
    if (!/^\d+$/.test(value) || Number(value) < min || Number(value) > max) throw new Error('Check ' + id.replace('cfg', '') + ': enter a whole number from ' + min + ' to ' + max + '.');
    return Number(value);
  };
  let payload;
  try {
    const parts = document.getElementById('cfgPanelOrder').value.split(',').map(s => s.trim());
    if (parts.length !== 4 || parts.some(s => !/^[0-3]$/.test(s)) || new Set(parts).size !== 4) throw new Error('Panel order must contain 0, 1, 2 and 3 exactly once.');
    const panelPins = Array.from({length: 4}, (_, i) => number('cfgPin' + i, 0, 48));
    const disabledPanels = Array.from({length: 4}, (_, i) => document.getElementById('cfgDisabledPanel' + i).checked);
    const parallel = document.getElementById('cfgParallelOutput').checked;
    if (parallel && new Set(panelPins).size !== 4) throw new Error('Parallel output GPIOs must be unique.');
    if (!parallel && disabledPanels.some(Boolean)) throw new Error('Disable a panel only when four parallel outputs are enabled.');
    const effect = document.getElementById('cfgBootEffect').value.trim();
    const path = document.getElementById('cfgBootPath').value.trim();
    if (!effect) throw new Error('Default effect ID cannot be empty.');
    if (path.split('/').includes('..')) throw new Error('LFX path cannot contain parent directory segments.');
    payload = {
      startCorner: number('cfgStartCorner', 0, 3), panelLayout: number('cfgPanelLayout', 0, 1),
      panelOrder: parts.map(Number), panelPins,
      panelRotation: Array.from({length: 4}, (_, i) => number('cfgRotation' + i, 0, 270)),
      serpentine: Array.from({length: 4}, (_, i) => document.getElementById('cfgSerpentine' + i).checked),
      panelSerpentine: document.getElementById('cfgPanelSerpentine').checked,
      mirrorX: document.getElementById('cfgMirrorX').checked,
      dataPin: number('cfgDataPin', 0, 48), parallelOutput: parallel, disabledPanels,
      ledMaxMilliamps: number('cfgMaxMilliamps', 1000, 60000),
      ledBrightness: number('cfgBootBrightness', 0, 255), defaultAnimation: effect,
      autoCycleMs: number('cfgBootCycle', 0, 4294967295), fsAnimationPath: path
    };
    payload = Object.fromEntries(Object.entries(payload).filter(([key, value]) => JSON.stringify(value) !== JSON.stringify(loadedConfig[key])));
  } catch (e) {
    msg.textContent = e.message;
    msg.style.color = 'var(--red)';
    return;
  }
  const bootChanged = ['ledBrightness', 'defaultAnimation', 'autoCycleMs', 'fsAnimationPath'].some(key => key in payload);
  msg.textContent = 'Saving...';
  document.getElementById('cfgSaveBtn').disabled = true;
  try {
    const r = await fetch('/api/config', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    const res = await r.json();
    if (res.config) loadedConfig = res.config;
    configRestartPending = configRestartPending || (res.config && bootChanged) || !!res.restartRequired || res.restart === 'required';
    if (r.ok) {
      if (res.powerLimitApply === 'pending') {
        msg.textContent = 'Saved; power limit update pending' + (configRestartPending ? ' (restart required for configuration changes)' : '');
        msg.style.color = 'var(--yellow)';
      } else if (configRestartPending) {
        msg.textContent = 'Saved. Restart required for configuration changes.';
        msg.style.color = 'var(--yellow)';
      } else {
        msg.textContent = 'Saved. Power limit applied live.';
        msg.style.color = 'var(--green)';
      }
    } else {
      msg.textContent = (res.powerLimitApply === 'failed' ? 'Saved, but power limit not applied: ' : 'Error: ') + (res.error || r.statusText) + (configRestartPending ? ' Restart required for configuration changes.' : '');
      msg.style.color = 'var(--red)';
    }
  } catch (e) {
    msg.textContent = 'Save failed; configuration may have been saved. Reload to check.';
    msg.style.color = 'var(--red)';
  } finally {
    document.getElementById('cfgSaveBtn').disabled = false;
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
      document.getElementById('micChannel').value = d.settings.rightChannel ? 'right' : 'left';
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
  updatePlaylistUI();

  // Audio buttons
  ['Off', 'Synthetic', 'Mic'].forEach(s => {
    const el = document.getElementById('audio' + s);
    if (el) el.classList.toggle('active', state.audio_source === s);
  });
  document.getElementById('audioVisualizer').style.display = (state.audio_source !== 'Off') ? 'block' : 'none';
  const soundBtn = document.getElementById('audioSoundBtn');
  if (soundBtn) soundBtn.style.display = (state.audio_source === 'Synthetic') ? 'inline-block' : 'none';
  if (state.audio_source !== 'Synthetic' && typeof audioPreviewPlaying !== 'undefined' && audioPreviewPlaying) {
    stopAudioPreviewSound();
  }

  // Update visualizer bands
  if (state.bands && state.bands.length === 16 && !micPlaybackHistory.length) {
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
  const select = document.getElementById('playlistEffectSelect');
  while (select.firstChild) select.removeChild(select.firstChild);
  catalog.forEach(eff => {
    if (typeof eff.id !== 'string' || !eff.id) return;
    const option = document.createElement('option');
    option.value = eff.id;
    option.textContent = eff.name + ' (' + eff.id + ')';
    select.appendChild(option);
  });
  lastRenderedPlaylistView = '';
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

      const paramLabel = p.label || p.name || p.id;
      const label = document.createElement('div');
      label.className = 'param-label';
      label.innerHTML = '<span>' + paramLabel + '</span><span id="pval_' + p.id + '"></span>';
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

// Audio arrives with state; there is no separate audio polling loop.

async function loop() {
  if (document.hidden) {
    setTimeout(loop, 2000);
    return;
  }
  try {
    const s = await fetchJSON('/api/state');
    state = s;
    queueMicHistory(s);
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
  // Match the device's audio telemetry cadence without overlapping requests.
  setTimeout(loop, state.audio_source === 'Mic' ? 200 : 500);
}

async function init() {
  document.getElementById('playlistLoop').addEventListener('change', () => {
    playlistLoopDraft = document.getElementById('playlistLoop').checked;
    playlistSaveVersion++;
    playlistPersistenceNotice = '';
    playlistDraftDirty = true;
    playlistAwaitingSnapshot = false;
    document.getElementById('playlistNotice').textContent = '';
    updatePlaylistUI();
  });
  loadPreviewRenderer();
  requestAnimationFrame(animatePreview);
  previewLoop();
  document.getElementById('diagCard').addEventListener('toggle', () => {
    if (document.getElementById('diagCard').open) loadFirmwareInfo();
  });
  document.getElementById('playlistNextBtn').onclick = () => controlPlaylist('next');
  document.getElementById('networkAddProfileBtn').onclick = addNetworkProfile;
  document.getElementById('networkSaveBtn').onclick = saveNetworkConfig;

  try {
    let effects = await fetchJSON('/api/effects');
    if (effects && effects.effects) effects = effects.effects;
    if (Array.isArray(effects)) {
      catalog = effects;
      renderCatalog();
    }
  } catch (e) {}
  await loadPanelConfig();
  await loadNetworkConfig();
  await loadMicData();
  loop();
}

window.addEventListener('DOMContentLoaded', init);
</script>
</body>
</html>
"##;
