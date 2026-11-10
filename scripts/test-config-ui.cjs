const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const { test } = require('node:test');
const html = fs.readFileSync('crates/matrix-control/src/web_ui.rs', 'utf8');
const script = html.match(/<script>([\s\S]*?)<\/script>/)[1];
const body = html.slice(html.indexOf('<body>'), html.indexOf('<script>'));
const config = {matrixWidth:2, matrixHeight:2, panelOrder:[0,1,2,3], panelRotation:[0,90,180,270], serpentine:[true,false,true,false], startCorner:0, panelLayout:1, panelSerpentine:true, mirrorX:false, dataPin:4, panelPins:[4,16,17,18], disabledPanels:[false,false,false,false], parallelOutput:true, ledMaxMilliamps:12000, ledBrightness:128, defaultAnimation:'Rainbow', autoCycleMs:0, fsAnimationPath:'/animations/example.lfx'};
function harness(failLoad = false) {
  const elements = new Map(), requests = [];
  const makeElement = () => {
    const children = [];
    const el = {
      _value:'', _id:'', get id(){return this._id}, set id(v){this._id=String(v); if(this._id) elements.set(this._id,this)},
      get value(){return this._value}, set value(v){this._value=String(v)}, checked:false, disabled:false, textContent:'', style:{}, className:'', dataset:{}, title:'', type:'', min:'', max:'', step:'', onclick:null, onchange:null,
      children,
      get firstChild(){return children[0] || null},
      appendChild(child){children.push(child); return child}, append(...items){children.push(...items)}, replaceChildren(...items){children.splice(0,children.length,...items)},
      removeChild(child){const i=children.indexOf(child); if(i>=0) children.splice(i,1); return child},
      set innerHTML(v){if(v==='') children.length=0},
      get innerHTML(){return ''},
      setAttribute(){}, addEventListener(){}, getContext:()=>({createImageData:()=>({data:new Uint8ClampedArray(4096)})})
    };
    return el;
  };
  const element = id => {
    if (!elements.has(id)) elements.set(id, makeElement());
    return elements.get(id);
  };
  const sandbox = {document:{getElementById:element, createElement:makeElement, hidden:false, querySelectorAll:()=>[]}, window:{addEventListener(){}}, Uint8Array, Uint8ClampedArray, TextEncoder, Number, JSON, Array, String, Math,
    fetch:async (url, options) => {requests.push({url,options}); if (!options && failLoad) throw Error('offline'); if (!options) return {ok:true,json:async()=>config}; if (url === '/api/state/save') return {ok:true,json:async()=>({ok:true})}; return {ok:true,json:async()=>({config:{...config,...JSON.parse(options.body)}, restartRequired:requests.length === 2, powerLimitApply:'applied'})};}};
  vm.createContext(sandbox); vm.runInContext(script, sandbox);
  return {sandbox, element, requests};
}
test('configuration round trips all editable fields and preserves other settings', async () => {
  const h=harness(); await h.sandbox.loadPanelConfig();
  assert.equal(h.element('cfgSaveBtn').disabled, false);
  assert.equal(h.element('cfgDataPin').disabled, true);
  assert.equal(h.element('cfgPin2').value, '17');
  assert.equal(h.element('cfgRotation1').value, '90');
  assert.equal(h.element('cfgSerpentine1').checked, false);
  h.element('cfgPin2').value='21'; h.element('cfgMirrorX').checked=true; h.element('cfgBootBrightness').value='100';
  h.element('cfgRotation1').value='180'; h.element('cfgSerpentine1').checked=true;
  h.element('cfgDisabledPanel1').checked=true;
  await h.sandbox.savePanelConfig();
  const sent=JSON.parse(h.requests[1].options.body);
  assert.deepEqual(sent.panelPins,[4,16,21,18]);
  assert.deepEqual(sent.panelRotation,[0,180,180,270]);
  assert.deepEqual(sent.serpentine,[true,true,true,false]);
  assert.deepEqual(sent.disabledPanels,[false,true,false,false]);
  assert.equal(sent.mirrorX,true); assert.equal(sent.ledBrightness,100); assert.equal(sent.defaultAnimation,undefined);
  assert.match(h.element('cfgStatusMsg').textContent,/Restart required/i);
  await h.sandbox.savePanelConfig();
  assert.match(h.element('cfgStatusMsg').textContent,/Restart required/i);
});
test('invalid panel orders and duplicate pins are not saved', async () => {
  const h=harness(); await h.sandbox.loadPanelConfig();
  h.element('cfgPanelOrder').value='0,1,2,2'; await h.sandbox.savePanelConfig();
  assert.equal(h.requests.length,1); assert.match(h.element('cfgStatusMsg').textContent,/exactly once/);
  h.element('cfgPanelOrder').value='0,1,2,3'; h.element('cfgPin1').value='4'; await h.sandbox.savePanelConfig();
  assert.equal(h.requests.length,1); assert.match(h.element('cfgStatusMsg').textContent,/unique/);
});
test('load failure disables save; output mode switches editable pins', async () => {
  const broken=harness(true); await broken.sandbox.loadPanelConfig();
  assert.equal(broken.element('cfgSaveBtn').disabled,true); assert.match(broken.element('cfgStatusMsg').textContent,/Could not load/);
  const h=harness(); await h.sandbox.loadPanelConfig(); h.element('cfgParallelOutput').checked=false; h.sandbox.updateOutputMode();
  assert.equal(h.element('cfgDataPin').disabled,false); assert.equal(h.element('cfgPin0').disabled,true);
});

test('wifi API UI exposes editable profile fields without echoing saved passphrases', () => {
  for (const id of ['networkSsid','networkPassword','networkPriority','networkHidden','networkAddProfileBtn','networkProfiles','fallbackApSsid','fallbackApPassword','fallbackApPasswordClear','networkSaveBtn','networkStatus']) {
    assert.match(body, new RegExp('id="' + id + '"'), 'missing network control ' + id);
  }
  assert.match(script, /fetch\('\/api\/network'/);
  assert.match(script, /fallbackApEnabled/);
  assert.match(script, /keepPassword\s*=\s*true/);
  assert.match(script, /target\.password\s*=\s*networkPassword/);
  assert.match(body, /Clear saved AP password and disable fallback AP/);
  assert.match(script, /192\.168\.4\.1/);
});

test('custom playlist UI exposes queue editing, playback, and snapshot telemetry', () => {
  for (const id of ['playlistEffectSelect','playlistDurationInput','playlistAddBtn','playlistEntries','playlistLoop','playlistApplyBtn','playlistStartBtn','playlistPauseBtn','playlistResumeBtn','playlistStopBtn','playlistPreviousBtn','playlistNextBtn','playlistCurrent','playlistRemaining','playlistNotice']) {
    assert.match(body, new RegExp('id="' + id + '"'), 'missing playlist control ' + id);
  }
  for (const fn of ['addPlaylistEntry','removePlaylistEntry','movePlaylistEntry','setPlaylistDuration','applyPlaylistConfig','controlPlaylist','updatePlaylistUI']) {
    assert.match(script, new RegExp('function ' + fn + '\\b'), 'missing playlist behavior ' + fn);
  }
  assert.match(script, /snapshot\.remainingMs/);
  assert.match(script, /label\.textContent\s*=/, 'dynamic UI should write text as text');
});

test('playlist inputs enforce engine limits and dynamic entry text uses safe DOM APIs', () => {
  assert.match(body, /id="playlistDurationInput"[^>]*min="1000"[^>]*max="86400000"/);
  assert.match(script, /MAX_PLAYLIST_ENTRIES\s*=\s*32/);
  assert.match(script, /MIN_PLAYLIST_ENTRY_MS\s*=\s*1000/);
  assert.match(script, /MAX_PLAYLIST_ENTRY_MS\s*=\s*86400000/);
  assert.match(script, /label\.textContent\s*=/);
  assert.doesNotMatch(script, /playlistEntries(?:\.innerHTML|\.insertAdjacentHTML)/);
});

test('playlist queue edits validate bounds, reorder safely, and post the Engine command shape', async () => {
  const h=harness();
  vm.runInContext("catalog=[{id:'safe-id',name:'<img src=x onerror=alert(1)>'},{id:'second',name:'Second'}]", h.sandbox);
  h.sandbox.renderCatalog();
  assert.equal(h.element('playlistEffectSelect').children[0].value, 'safe-id');
  assert.equal(h.element('playlistEffectSelect').children[0].textContent, '<img src=x onerror=alert(1)> (safe-id)');
  h.element('playlistEffectSelect').value='safe-id';
  h.element('playlistDurationInput').value='999';
  h.sandbox.addPlaylistEntry();
  assert.match(h.element('playlistNotice').textContent,/1000 to 86400000/);
  assert.equal(vm.runInContext('playlistEntries.length', h.sandbox), 0);
  h.element('playlistDurationInput').value='1000'; h.sandbox.addPlaylistEntry();
  h.element('playlistEffectSelect').value='second'; h.element('playlistDurationInput').value='86400000'; h.sandbox.addPlaylistEntry();
  h.sandbox.movePlaylistEntry(1,-1);
  assert.deepEqual(JSON.parse(JSON.stringify(vm.runInContext('playlistEntries.map(e=>[e.effectId,e.durationMs])', h.sandbox))), [['second',86400000],['safe-id',1000]]);
  h.sandbox.setPlaylistDuration(0,'86400001');
  assert.match(h.element('playlistNotice').textContent,/1000 to 86400000/);
  h.sandbox.removePlaylistEntry(0);
  h.element('playlistLoop').checked=true;
  await h.sandbox.applyPlaylistConfig();
  assert.equal(h.requests.length,1);
  assert.equal(h.requests[0].url,'/api/command');
  assert.deepEqual(JSON.parse(h.requests[0].options.body), {kind:'playlist_config',payload:{entries:[{effectId:'safe-id',durationMs:1000}],loop:true}});
  vm.runInContext("playlistEntries=Array.from({length:32},()=>({effectId:'safe-id',durationMs:1000}))", h.sandbox);
  h.element('playlistEffectSelect').value='safe-id'; h.element('playlistDurationInput').value='1000'; h.sandbox.addPlaylistEntry();
  assert.equal(vm.runInContext('playlistEntries.length', h.sandbox), 32);
  assert.match(h.element('playlistNotice').textContent,/at most 32/);
});

test('playlist Save queue response waits for a field-order-independent matching snapshot', async () => {
  const h=harness();
  vm.runInContext("catalog=[{id:'safe-id',name:'Safe effect'},{id:'second',name:'Second'}]; playlistEntries=[{effectId:'safe-id',durationMs:30000},{effectId:'second',durationMs:45000}]; playlistLoopDraft=true; playlistDraftDirty=true; state={cycle:'OFF',effect_id:'safe-id',playlist:{entries:[],loop:false,status:'stopped',index:null,effectId:null,remainingMs:0}}", h.sandbox);
  h.element('playlistLoop').checked = true;
  await h.sandbox.applyPlaylistConfig();
  assert.equal(vm.runInContext('playlistAwaitingSnapshot', h.sandbox), true);
  const differingSnapshots = [
    {entries:[{effectId:'second',durationMs:45000},{effectId:'safe-id',durationMs:30000}],loop:true},
    {entries:[{effectId:'safe-id',durationMs:30000},{effectId:'other',durationMs:45000}],loop:true},
    {entries:[{effectId:'safe-id',durationMs:30000},{effectId:'second',durationMs:44000}],loop:true},
    {entries:[{effectId:'safe-id',durationMs:30000},{effectId:'second',durationMs:45000}],loop:false}
  ];
  for (const playlist of differingSnapshots) {
    const mismatchingSnapshot = {...playlist,status:'stopped',index:null,effectId:null,remainingMs:0};
    vm.runInContext('state.playlist = JSON.parse(' + JSON.stringify(JSON.stringify(mismatchingSnapshot)) + ')', h.sandbox);
    h.sandbox.updatePlaylistUI();
    assert.equal(vm.runInContext('playlistDraftDirty', h.sandbox), true);
    assert.equal(vm.runInContext('playlistAwaitingSnapshot', h.sandbox), true);
    assert.equal(h.element('playlistStartBtn').disabled, true);
  }
  const matchingSnapshot = {entries:[{durationMs:30000,effectId:'safe-id'},{durationMs:45000,effectId:'second'}],loop:true,status:'stopped',index:null,effectId:null,remainingMs:0};
  vm.runInContext('state.playlist = JSON.parse(' + JSON.stringify(JSON.stringify(matchingSnapshot)) + ')', h.sandbox);
  h.sandbox.updatePlaylistUI();
  assert.equal(vm.runInContext('playlistDraftDirty', h.sandbox), false);
  assert.equal(vm.runInContext('playlistAwaitingSnapshot', h.sandbox), false);
  assert.equal(h.element('playlistStartBtn').disabled, false);
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(h.requests.map(request => request.url), ['/api/command','/api/state/save']);
});

test('editing while Save queue is in flight does not arm the newer draft for an old snapshot', async () => {
  const h=harness();
  const pendingResponse = {};
  pendingResponse.promise = new Promise(resolve => { pendingResponse.resolve = resolve; });
  h.sandbox.postCommand = () => pendingResponse.promise;
  vm.runInContext("catalog=[{id:'first',name:'First'},{id:'second',name:'Second'}]; state={cycle:'OFF',effect_id:'first',playlist:{entries:[],loop:false,status:'stopped',index:null,effectId:null,remainingMs:0}}; playlistEntries=[{effectId:'first',durationMs:30000}]; playlistLoopDraft=true; playlistDraftDirty=true", h.sandbox);
  h.element('playlistLoop').checked = true;
  const saving = h.sandbox.applyPlaylistConfig();
  vm.runInContext("playlistEntries=[{effectId:'second',durationMs:45000}]; playlistLoopDraft=false; playlistSaveVersion++; document.getElementById('playlistLoop').checked=false; playlistDraftDirty=true; playlistAwaitingSnapshot=false", h.sandbox);
  pendingResponse.resolve({pending:false});
  await saving;
  assert.equal(vm.runInContext('playlistDraftDirty', h.sandbox), true);
  assert.equal(vm.runInContext('playlistAwaitingSnapshot', h.sandbox), false);
  assert.match(h.element('playlistNotice').textContent,/changed while save was pending/i);
  vm.runInContext("state.playlist={entries:[{effectId:'first',durationMs:30000}],loop:true,status:'stopped',index:null,effectId:null,remainingMs:0}; updatePlaylistUI()", h.sandbox);
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(h.requests.map(request => request.url), []);
});

test('a newer confirmed queue is persisted after an earlier persistence request completes', async () => {
  const h=harness();
  const saveResolvers = [];
  h.sandbox.fetch = async (url, options) => {
    h.requests.push({url,options});
    if (url === '/api/state/save') return new Promise(resolve => saveResolvers.push(resolve));
    return {ok:true,json:async()=>({pending:false})};
  };
  vm.runInContext("catalog=[{id:'first',name:'First'},{id:'second',name:'Second'}]; state={cycle:'OFF',effect_id:'first',playlist:{entries:[],loop:false,status:'stopped',index:null,effectId:null,remainingMs:0}}; playlistEntries=[{effectId:'first',durationMs:30000}]; playlistLoopDraft=true; playlistDraftDirty=true", h.sandbox);
  h.element('playlistLoop').checked = true;
  await h.sandbox.applyPlaylistConfig();
  vm.runInContext("state.playlist=JSON.parse('{\"entries\":[{\"durationMs\":30000,\"effectId\":\"first\"}],\"loop\":true,\"status\":\"stopped\",\"index\":null,\"effectId\":null,\"remainingMs\":0}'); updatePlaylistUI()", h.sandbox);
  assert.equal(saveResolvers.length, 1);
  vm.runInContext("playlistEntries=[{effectId:'second',durationMs:45000}]; playlistLoopDraft=false; playlistSaveVersion++; document.getElementById('playlistLoop').checked=false; playlistDraftDirty=true; playlistAwaitingSnapshot=false", h.sandbox);
  await h.sandbox.applyPlaylistConfig();
  vm.runInContext("state.playlist=JSON.parse('{\"entries\":[{\"durationMs\":45000,\"effectId\":\"second\"}],\"loop\":false,\"status\":\"stopped\",\"index\":null,\"effectId\":null,\"remainingMs\":0}'); updatePlaylistUI()", h.sandbox);
  assert.equal(saveResolvers.length, 1);
  saveResolvers[0]({ok:true,json:async()=>({ok:true})});
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(saveResolvers.length, 2);
  saveResolvers[1]({ok:true,json:async()=>({ok:true})});
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(h.requests.filter(request => request.url === '/api/state/save').length, 2);
});

test('playlist persistence failure notice remains visible after a later state refresh', async () => {
  const h=harness();
  h.sandbox.fetch = async (url, options) => {
    h.requests.push({url,options});
    if (url === '/api/state/save') return {ok:false,status:500,statusText:'Internal Server Error',json:async()=>({error:'write failed'})};
    return {ok:true,json:async()=>({pending:false})};
  };
  vm.runInContext("catalog=[{id:'safe-id',name:'Safe effect'}]; state={cycle:'OFF',effect_id:'safe-id',playlist:{entries:[],loop:false,status:'stopped',index:null,effectId:null,remainingMs:0}}; playlistEntries=[{effectId:'safe-id',durationMs:30000}]; playlistLoopDraft=true; playlistDraftDirty=true", h.sandbox);
  h.element('playlistLoop').checked = true;
  await h.sandbox.applyPlaylistConfig();
  vm.runInContext("state.playlist=JSON.parse('{\"entries\":[{\"durationMs\":30000,\"effectId\":\"safe-id\"}],\"loop\":true,\"status\":\"stopped\",\"index\":null,\"effectId\":null,\"remainingMs\":0}'); updatePlaylistUI()", h.sandbox);
  await new Promise(resolve => setImmediate(resolve));
  assert.match(h.element('playlistNotice').textContent,/could not be saved to device storage/i);
  h.sandbox.updatePlaylistUI();
  assert.match(h.element('playlistNotice').textContent,/could not be saved to device storage/i);
});

test('Wi-Fi settings status reports an empty saved profile list without throwing', async () => {
  const h=harness();
  vm.runInContext("catalog=[]; state={networkConfig:{}}", h.sandbox);
  vm.runInContext("fetch=async()=>({ok:true,json:async()=>({wifiProfiles:[],fallbackApEnabled:true,fallbackApSsid:'safe-ap'})})", h.sandbox);
  await h.sandbox.loadNetworkConfig();
  assert.equal(h.element('networkStatus').textContent,'No saved Wi-Fi profiles. Fallback AP is enabled; AP credentials remain masked.');
});

test('saving a playlist persists the matching device snapshot across reboot', async () => {
  const h=harness();
  vm.runInContext("catalog=[{id:'safe-id',name:'Safe effect'}]; state={cycle:'OFF',effect_id:'safe-id',playlist:{entries:[],loop:false,status:'stopped',index:null,effectId:null,remainingMs:0}}; playlistEntries=[{effectId:'safe-id',durationMs:30000}]; playlistLoopDraft=true; playlistDraftDirty=true", h.sandbox);
  h.sandbox.renderCatalog();
  h.element('playlistLoop').checked=true;
  await h.sandbox.applyPlaylistConfig();
  assert.deepEqual(h.requests.map(r=>r.url), ['/api/command']);
  vm.runInContext("state.playlist={entries:[{durationMs:30000,effectId:'safe-id'}],loop:true,status:'stopped',index:null,effectId:null,remainingMs:0}; updatePlaylistUI()", h.sandbox);
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(h.requests.map(r=>r.url), ['/api/command','/api/state/save']);
  assert.equal(h.requests[1].options.method, 'POST');
  await new Promise(resolve => setImmediate(resolve));
  assert.match(h.element('playlistNotice').textContent,/Queue saved to device storage/);
});

test('playlist controls send Engine actions and display snapshot time without a client timer', async () => {
  const h=harness();
  vm.runInContext("catalog=[{id:'safe-id',name:'Safe effect'}]; state={cycle:'ON',effect_id:'other',playlist:{entries:[{effectId:'safe-id',durationMs:10000}],loop:true,status:'running',index:0,effectId:'safe-id',remainingMs:1500}}", h.sandbox);
  h.sandbox.renderCatalog(); h.sandbox.updatePlaylistUI();
  assert.equal(h.element('playlistCurrent').textContent, 'Current entry: 1 of 1 · Safe effect (running)');
  assert.equal(h.element('playlistRemaining').textContent, 'Time remaining: 2s');
  assert.match(h.element('playlistNotice').textContent,/Auto Cycle is enabled and takes control from the playlist/);
  await h.sandbox.controlPlaylist('next');
  assert.deepEqual(JSON.parse(h.requests[0].options.body), {kind:'playlist_control',payload:{action:'next'}});
  assert.doesNotMatch(script,/setInterval\(|setTimeout\([^\n]*playlist/i);
});

test('manual effect outside the configured queue is visibly reflected from snapshot fields', () => {
  const h=harness();
  vm.runInContext("catalog=[{id:'queued',name:'Queued'},{id:'manual',name:'Manual effect'}]; state={cycle:'OFF',effect_id:'manual',playlist:{entries:[{effectId:'queued',durationMs:1000}],loop:false,status:'stopped',index:null,effectId:null,remainingMs:0}}", h.sandbox);
  h.sandbox.renderCatalog(); h.sandbox.updatePlaylistUI();
  assert.match(h.element('playlistNotice').textContent,/manual effect selection is active and is not part of this queue/i);
  assert.match(h.element('playlistCurrent').textContent,/active effect: Manual effect/);
});
