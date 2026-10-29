const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const { test } = require('node:test');
const html = fs.readFileSync('crates/matrix-control/src/web_ui.rs', 'utf8');
function harness({ confirmed = true, ok = true, throws = false } = {}) {
  const requests = [], timers = [], messages = [];
  const button = { disabled: false };
  const sandbox = {
    confirm: () => confirmed,
    document: { getElementById: () => button },
    fetch: async (...args) => { requests.push(args); if (throws) throw Error('offline'); return { ok }; },
    showToast: msg => messages.push(msg),
    setTimeout: fn => timers.push(fn),
    window: { location: { reload() {} } },
  };
  const fn = html.match(/async function restartDevice\(\) \{[\s\S]*?\n\}/);
  assert.ok(fn, 'restartDevice exists');
  vm.createContext(sandbox);
  vm.runInContext(fn[0], sandbox);
  return { sandbox, requests, timers, messages, button };
}
test('restart button calls direct recovery endpoint and schedules reload', async () => {
  assert.match(html, /id="restartBtn"[^>]*onclick="restartDevice\(\)"/);
  const h = harness();
  await h.sandbox.restartDevice();
  assert.equal(h.requests[0][0], '/api/reboot');
  assert.equal(h.requests[0][1].method, 'POST');
  assert.equal(h.button.disabled, true);
  assert.equal(h.timers.length, 1);
});
test('cancel does not reboot', async () => {
  const h = harness({ confirmed: false });
  await h.sandbox.restartDevice();
  assert.equal(h.requests.length, 0);
});
for (const options of [{ ok: false }, { throws: true }]) {
  test(`failed restart re-enables button: ${JSON.stringify(options)}`, async () => {
    const h = harness(options);
    await h.sandbox.restartDevice();
    assert.equal(h.button.disabled, false);
    assert.equal(h.timers.length, 0);
  });
}
