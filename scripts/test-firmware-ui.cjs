const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const { test } = require('node:test');
const html = fs.readFileSync('crates/matrix-control/src/web_ui.rs', 'utf8');
const script = html.match(/<script>([\s\S]*?)<\/script>/)[1];
function harness(response) {
  const elements = new Map(), requests = [];
  const element = id => {
    if (!elements.has(id)) elements.set(id, { textContent: '', style: {}, addEventListener() {}, appendChild() {}, getContext: () => ({createImageData: () => ({data: new Uint8ClampedArray(4096)})}) });
    return elements.get(id);
  };
  const sandbox = { document: { getElementById: element, createElement: () => element(Symbol()) }, window: {addEventListener() {}}, TextEncoder, Uint8Array, Uint8ClampedArray,
    fetch: async url => { requests.push(url); if (response instanceof Error) throw response; return response; } };
  vm.createContext(sandbox);
  vm.runInContext(script, sandbox);
  return {sandbox, element, requests};
}
test('diagnostics displays running firmware identity and OTA state safely', async () => {
  for (const id of ['firmwareVersion', 'firmwareHash', 'firmwarePartition', 'firmwareOtaState']) assert.match(html, new RegExp(`id="${id}"`));
  const h = harness({ok:true, json:async()=>({version:'v1',elfSha256:'abc',partition:'app0',partitionAddress:131072,otaState:'valid'})});
  await h.sandbox.loadFirmwareInfo();
  assert.deepEqual(h.requests, ['/api/firmware']);
  assert.equal(h.element('firmwareVersion').textContent, 'v1');
  assert.equal(h.element('firmwareHash').textContent, 'abc');
  assert.equal(h.element('firmwarePartition').textContent, 'app0 (0x20000)');
  assert.equal(h.element('firmwareOtaState').textContent, 'valid');
});
test('unsupported endpoint and unknown OTA state are explicit', async () => {
  const h = harness({ok:false, statusText:'Not Found'});
  await h.sandbox.loadFirmwareInfo();
  assert.equal(h.element('firmwareVersion').textContent, 'Unavailable');
  assert.equal(h.element('firmwareHash').textContent, '--');
  const h2 = harness({ok:true, json:async()=>({version:'v1',partition:'app1',partitionAddress:0,otaState:null})});
  await h2.sandbox.loadFirmwareInfo();
  assert.equal(h2.element('firmwareOtaState').textContent, 'Unknown');
});
