const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const { test } = require('node:test');
const html = fs.readFileSync('crates/matrix-control/src/web_ui.rs', 'utf8');
const script = html.match(/<script>([\s\S]*?)<\/script>/)[1];
const config = {matrixWidth:2, matrixHeight:2, panelOrder:[0,1,2,3], panelRotation:[0,90,180,270], serpentine:[true,false,true,false], startCorner:0, panelLayout:1, panelSerpentine:true, mirrorX:false, dataPin:4, panelPins:[4,16,17,18], parallelOutput:true, ledMaxMilliamps:12000, ledBrightness:128, defaultAnimation:'Rainbow', autoCycleMs:0, fsAnimationPath:'/animations/example.lfx'};
function harness(failLoad = false) {
  const elements = new Map(), requests = [];
  const element = id => {
    if (!elements.has(id)) elements.set(id, { _value:'', get value(){return this._value}, set value(v){this._value=String(v)}, checked:false, disabled:false, textContent:'', style:{}, addEventListener(){}, appendChild(){}, getContext:()=>({createImageData:()=>({data:new Uint8ClampedArray(4096)})}) });
    return elements.get(id);
  };
  const sandbox = {document:{getElementById:element, createElement:()=>element(Symbol())}, window:{addEventListener(){}}, Uint8Array, Uint8ClampedArray, TextEncoder,
    fetch:async (url, options) => {requests.push({url,options}); if (!options && failLoad) throw Error('offline'); if (!options) return {ok:true,json:async()=>config}; return {ok:true,json:async()=>({config:{...config,...JSON.parse(options.body)}, restartRequired:requests.length === 2, powerLimitApply:'applied'})};}};
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
  await h.sandbox.savePanelConfig();
  const sent=JSON.parse(h.requests[1].options.body);
  assert.deepEqual(sent.panelPins,[4,16,21,18]);
  assert.deepEqual(sent.panelRotation,[0,180,180,270]);
  assert.deepEqual(sent.serpentine,[true,true,true,false]);
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
