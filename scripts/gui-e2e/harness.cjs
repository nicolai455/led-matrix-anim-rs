#!/usr/bin/env node
'use strict';
/**
 * Mini headless-GUI harness for the LED matrix web UI.
 *
 * No npm dependencies: Chromium is driven over the DevTools protocol with
 * Node's built-in global WebSocket, and the simulator is spawned as a child
 * process. Everything runs offline against 127.0.0.1.
 *
 * Usage:
 *   node scripts/gui-e2e/harness.cjs --self-test   # smoke-test harness only
 *   node scripts/gui-e2e/run.cjs                   # full suite
 */
const { spawn } = require('node:child_process');
const fs = require('node:fs');
const net = require('node:net');
const os = require('node:os');
const path = require('node:path');

const REPO_ROOT = path.resolve(__dirname, '..', '..');
const ARTIFACT_DIR = path.join(__dirname, 'artifacts');

const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

async function freePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.unref();
    server.on('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });
}

function simulatorBinary() {
  const candidates = [
    process.env.MATRIX_SIM_BIN,
    path.join(REPO_ROOT, 'target', 'debug', 'matrix-simulator'),
    path.join(REPO_ROOT, 'target', 'release', 'matrix-simulator'),
  ].filter(Boolean);
  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) return candidate;
  }
  throw new Error(
    'matrix-simulator binary not found; run `cargo build -p matrix-control --bin matrix-simulator` first'
  );
}

async function waitForHttp(url, timeoutMs = 20000) {
  const deadline = Date.now() + timeoutMs;
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url, { cache: 'no-store' });
      if (response.ok) return response;
      lastError = new Error(`HTTP ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await sleep(100);
  }
  throw new Error(`timed out waiting for ${url}: ${lastError && lastError.message}`);
}

/** Spawns the host simulator on a free port with throw-away state/config files. */
class Simulator {
  constructor(options = {}) {
    this.options = options;
    this.dir = fs.mkdtempSync(path.join(os.tmpdir(), 'matrix-gui-e2e-'));
    this.statePath = path.join(this.dir, 'state.json');
    this.configPath = path.join(this.dir, 'config.json');
    if (options.config) fs.writeFileSync(this.configPath, JSON.stringify(options.config));
    this.child = null;
    this.base = null;
    this.stdout = '';
    this.stderr = '';
    this.exitCode = undefined;
  }

  async start() {
    const port = await freePort();
    this.base = `http://127.0.0.1:${port}`;
    const args = ['--listen', `127.0.0.1:${port}`, '--state', this.statePath, '--config', this.configPath];
    this.child = spawn(simulatorBinary(), args, { stdio: ['ignore', 'pipe', 'pipe'] });
    this.child.stdout.on('data', chunk => { this.stdout += chunk.toString(); });
    this.child.stderr.on('data', chunk => { this.stderr += chunk.toString(); });
    this.child.on('exit', code => { this.exitCode = code; });
    await waitForHttp(`${this.base}/api/state`);
    return this;
  }

  /** Posts a raw command envelope; returns {status, body}. */
  async command(kind, payload) {
    const response = await fetch(`${this.base}/api/command`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Matrix-Control': '1' },
      body: JSON.stringify({ kind, payload }),
    });
    const text = await response.text();
    let body = null;
    try { body = JSON.parse(text); } catch { body = text; }
    return { status: response.status, body };
  }

  async state() {
    return (await fetch(`${this.base}/api/state`, { cache: 'no-store' })).json();
  }

  async stop() {
    if (this.child && this.exitCode === undefined) {
      this.child.kill('SIGTERM');
      const deadline = Date.now() + 5000;
      while (this.exitCode === undefined && Date.now() < deadline) await sleep(50);
      if (this.exitCode === undefined) this.child.kill('SIGKILL');
    }
    fs.rmSync(this.dir, { recursive: true, force: true, maxRetries: 5, retryDelay: 50 });
  }
}

/** Minimal Chrome DevTools Protocol client over the global WebSocket. */
class Cdp {
  constructor(socket) {
    this.socket = socket;
    this.nextId = 0;
    this.pending = new Map();
    this.listeners = [];
    socket.onmessage = event => {
      const message = JSON.parse(event.data);
      if (message.id) {
        const entry = this.pending.get(message.id);
        if (entry) {
          this.pending.delete(message.id);
          if (message.error) entry.reject(new Error(`${entry.method}: ${message.error.message}`));
          else entry.resolve(message.result);
        }
        return;
      }
      for (const listener of this.listeners) listener(message);
    };
  }

  on(listener) { this.listeners.push(listener); return () => {
    const index = this.listeners.indexOf(listener);
    if (index >= 0) this.listeners.splice(index, 1);
  }; }

  send(method, params = {}) {
    const id = ++this.nextId;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject, method });
      this.socket.send(JSON.stringify({ id, method, params }));
    });
  }

  close() {
    try { this.socket.close(); } catch { /* already closed */ }
  }
}

/** Spawns headless Chromium with a throw-away profile. */
async function launchChromium() {
  const port = await freePort();
  const userDataDir = fs.mkdtempSync(path.join(os.tmpdir(), 'matrix-chrome-e2e-'));
  const binary = process.env.CHROMIUM_BIN || 'chromium';
  const args = [
    '--headless=new',
    `--remote-debugging-port=${port}`,
    `--user-data-dir=${userDataDir}`,
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-extensions',
    '--disable-component-extensions-with-background-pages',
    '--disable-background-networking',
    '--disable-dev-shm-usage',
    '--no-sandbox',
    '--hide-scrollbars',
    '--window-size=1280,1600',
    'about:blank',
  ];
  const child = spawn(binary, args, { stdio: ['ignore', 'pipe', 'pipe'] });
  let log = '';
  child.stderr.on('data', chunk => { log += chunk.toString(); });
  child.on('exit', code => { log += `\n[chromium exit ${code}]`; });

  const deadline = Date.now() + 20000;
  let version = null;
  while (Date.now() < deadline && !version) {
    try {
      version = await (await fetch(`http://127.0.0.1:${port}/json/version`)).json();
    } catch {
      await sleep(100);
    }
  }
  if (!version) {
    child.kill('SIGKILL');
    throw new Error(`chromium did not expose a DevTools endpoint:\n${log}`);
  }

  return {
    port,
    browser: version.Browser,
    log: () => log,
    /** Opens a fresh tab and returns a Page bound to that tab's own WS. */
    async newPage(url = 'about:blank') {
      const created = await (await fetch(`http://127.0.0.1:${port}/json/new?${encodeURIComponent(url)}`, { method: 'PUT' })).json();
      const socket = new WebSocket(created.webSocketDebuggerUrl);
      await new Promise((resolve, reject) => {
        socket.onopen = resolve;
        socket.onerror = () => reject(new Error('failed to open DevTools websocket'));
      });
      const cdp = new Cdp(socket);
      const page = new Page(cdp, created.targetId);
      await page.enable();
      if (url && url !== 'about:blank') {
        await page.goto(url);
      }
      return page;
    },
    stop() {
      try { child.kill('SIGTERM'); } catch { /* already gone */ }
      setTimeout(() => { try { child.kill('SIGKILL'); } catch { /* gone */ } }, 500).unref();
      fs.rmSync(userDataDir, { recursive: true, force: true, maxRetries: 5, retryDelay: 50 });
    },
  };
}

/**
 * One browser page with the helpers the tests use. Console errors, uncaught
 * exceptions and HTTP responses >= 400 are collected so a test can fail on them.
 */
class Page {
  constructor(cdp, targetId, options = {}) {
    this.cdp = cdp;
    this.targetId = targetId;
    this.consoleErrors = [];
    this.httpErrors = [];
    this.requests = [];
    this.dialogs = [];
    this.expectedHttpErrors = [];
    this.requestBodies = new Map();
    this.options = options;
    this.off = cdp.on(message => this.handle(message));
  }

  handle(message) {
    const { method, params } = message;
    if (method === 'Runtime.consoleAPICalled' && params.type === 'error') {
      this.consoleErrors.push(params.args.map(a => a.value ?? a.description ?? a.type).join(' '));
    }
    if (method === 'Runtime.exceptionThrown') {
      const details = params.exceptionDetails;
      this.consoleErrors.push('Uncaught: ' + (details.exception?.description || details.text));
    }
    if (method === 'Log.entryAdded' && params.entry.level === 'error') {
      this.consoleErrors.push({ text: params.entry.text, url: params.entry.url || '' });
    }
    if (method === 'Network.requestWillBeSent') {
      this.requests.push(params.request.url);
      // Keep the body of POSTs so a 4xx can be diagnosed instead of guessed at.
      if (params.request.postData) {
        this.requestBodies.set(params.requestId, {
          url: params.request.url,
          method: params.request.method,
          body: params.request.postData,
        });
      }
    }
    if (method === 'Network.responseReceived') {
      const { status, url } = params.response;
      if (status >= 400) {
        const sent = this.requestBodies.get(params.requestId);
        this.httpErrors.push({
          status,
          url,
          body: sent && sent.body ? sent.body.slice(0, 400) : undefined,
        });
      }
      this.requestBodies.delete(params.requestId);
    }
    if (method === 'Page.javascriptDialogOpening') {
      this.dialogs.push(params.message);
      this.send('Page.handleJavaScriptDialog', { accept: this.options.acceptDialogs !== false }).catch(() => {});
    }
  }

  send(method, params = {}) {
    return this.cdp.send(method, params);
  }

  async evaluate(expression) {
    const response = await this.send('Runtime.evaluate', {
      expression,
      returnByValue: true,
      awaitPromise: true,
      userGesture: true,
    });
    if (response.exceptionDetails) {
      const details = response.exceptionDetails;
      throw new Error('evaluate failed: ' + (details.exception?.description || details.text) + '\n' + expression);
    }
    return response.result.value;
  }

  async enable() {
    await this.send('Page.enable');
    await this.send('Runtime.enable');
    await this.send('Log.enable');
    await this.send('Network.enable');
  }

  async goto(url) {
    await this.send('Page.navigate', { url });
    await this.waitFor('document.readyState === "complete"', 20000, 'document ready');
    await this.waitFor('typeof updatePlaylistUI === "function"', 20000, 'UI script evaluated');
  }

  async reload(url) {
    await this.send('Page.navigate', { url });
    await this.waitFor('document.readyState === "complete"', 20000, 'document ready after reload');
    await this.waitFor('typeof updatePlaylistUI === "function"', 20000, 'UI script re-evaluated');
  }

  /** Polls a JS predicate until it is truthy. Never sleeps blindly. */
  async waitFor(expression, timeoutMs = 8000, description = expression) {
    const deadline = Date.now() + timeoutMs;
    let last = null;
    while (Date.now() < deadline) {
      try {
        const value = await this.evaluate(expression);
        if (value) return value;
        last = value;
      } catch (error) {
        last = error.message;
      }
      await sleep(50);
    }
    throw new Error(`timed out waiting for ${description} (last: ${JSON.stringify(last)})`);
  }

  /** Waits for the UI's own snapshot state to satisfy a predicate. */
  waitForSnapshot(predicate, timeoutMs = 8000, description = predicate) {
    return this.waitFor(`(() => { try { return !!(${predicate}); } catch (e) { return false; } })()`, timeoutMs, description);
  }

  async click(selector) {
    const clicked = await this.evaluate(`(() => {
      const el = document.querySelector(${JSON.stringify(selector)});
      if (!el) return 'missing';
      if (el.disabled) return 'disabled';
      el.scrollIntoView({block: 'center'});
      el.click();
      return 'ok';
    })()`);
    if (clicked !== 'ok') throw new Error(`cannot click ${selector}: ${clicked}`);
    await sleep(30);
  }

  /** Clicks an element found by arbitrary JS (used for dynamically rendered rows). */
  async clickExpression(finderExpression) {
    const clicked = await this.evaluate(`(() => {
      const el = ${finderExpression};
      if (!el) return 'missing';
      if (el.disabled) return 'disabled';
      el.scrollIntoView({block: 'center'});
      el.click();
      return 'ok';
    })()`);
    if (clicked !== 'ok') throw new Error(`cannot click ${finderExpression}: ${clicked}`);
    await sleep(30);
  }

  /** Real mouse event via CDP at the element centre (for canvas / overlay cases). */
  async clickAt(selector) {
    const box = await this.evaluate(`(() => {
      const el = document.querySelector(${JSON.stringify(selector)});
      if (!el) return null;
      const r = el.getBoundingClientRect();
      return {x: r.left + r.width / 2, y: r.top + r.height / 2};
    })()`);
    if (!box) throw new Error(`missing element ${selector}`);
    for (const type of ['mousePressed', 'mouseReleased']) {
      await this.send('Input.dispatchMouseEvent', {
        type, x: box.x, y: box.y, button: 'left', clickCount: 1,
      });
    }
    await sleep(30);
  }

  /** Sets a form value and fires input+change so oninput/onchange handlers run. */
  async setValue(selector, value) {
    const outcome = await this.evaluate(`(() => {
      const el = document.querySelector(${JSON.stringify(selector)});
      if (!el) return 'missing';
      const proto = el instanceof HTMLSelectElement ? HTMLSelectElement.prototype
        : el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
      const descriptor = Object.getOwnPropertyDescriptor(proto, 'value');
      if (descriptor && descriptor.set) descriptor.set.call(el, ${JSON.stringify(String(value))});
      else el.value = ${JSON.stringify(String(value))};
      el.dispatchEvent(new Event('input', {bubbles: true}));
      el.dispatchEvent(new Event('change', {bubbles: true}));
      return 'ok';
    })()`);
    if (outcome !== 'ok') throw new Error(`cannot set ${selector}: ${outcome}`);
    await sleep(30);
  }

  async setChecked(selector, checked) {
    const outcome = await this.evaluate(`(() => {
      const el = document.querySelector(${JSON.stringify(selector)});
      if (!el) return 'missing';
      el.checked = ${checked ? 'true' : 'false'};
      el.dispatchEvent(new Event('change', {bubbles: true}));
      return 'ok';
    })()`);
    if (outcome !== 'ok') throw new Error(`cannot check ${selector}: ${outcome}`);
    await sleep(30);
  }

  text(selector) {
    return this.evaluate(`(() => {
      const el = document.querySelector(${JSON.stringify(selector)});
      return el ? (el.textContent || '') : null;
    })()`);
  }

  async value(selector) {
    return this.evaluate(`(() => {
      const el = document.querySelector(${JSON.stringify(selector)});
      return el ? el.value : null;
    })()`);
  }

  visible(selector) {
    return this.evaluate(`(() => {
      const el = document.querySelector(${JSON.stringify(selector)});
      if (!el) return false;
      const style = getComputedStyle(el);
      const rect = el.getBoundingClientRect();
      return style.display !== 'none' && style.visibility !== 'hidden' && rect.width > 0 && rect.height > 0;
    })()`);
  }

  disabled(selector) {
    return this.evaluate(`(() => {
      const el = document.querySelector(${JSON.stringify(selector)});
      return el ? !!el.disabled : null;
    })()`);
  }

  async openDetails(selector) {
    const open = await this.evaluate(`(() => {
      const el = document.querySelector(${JSON.stringify(selector)});
      if (!el) return 'missing';
      el.open = true;
      return 'ok';
    })()`);
    if (open !== 'ok') throw new Error(`cannot open ${selector}: ${open}`);
    await sleep(50);
  }

  artifactsDir() {
    fs.mkdirSync(ARTIFACT_DIR, { recursive: true });
    return ARTIFACT_DIR;
  }

  async screenshot(name) {
    const result = await this.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true });
    const file = path.join(this.artifactsDir(), `${name}.png`);
    fs.writeFileSync(file, Buffer.from(result.data, 'base64'));
    return file;
  }

  /** Fails when an unexpected console error or HTTP >= 400 was observed. */
  assertClean(context = '') {
    const isExpected = (url, status) =>
      this.expectedHttpErrors.some(rule => rule.url.test(url) && rule.status === status);
    const unexpectedHttp = this.httpErrors.filter(error => !isExpected(error.url, error.status));
    const unexpectedConsole = this.consoleErrors.filter(entry => {
      // Chrome logs 'Failed to load resource' for every HTTP >= 400; those are
      // already classified through httpErrors, so avoid double reporting.
      if (typeof entry === 'string') return true;
      if (!/Failed to load resource|net::ERR_/.test(entry.text)) return true;
      const match = this.httpErrors.find(error => entry.url === error.url);
      return !match || !isExpected(match.url, match.status);
    });
    const problems = [];
    if (unexpectedConsole.length) problems.push('console errors: ' + JSON.stringify(unexpectedConsole));
    if (unexpectedHttp.length) problems.push('HTTP errors: ' + JSON.stringify(unexpectedHttp));
    if (problems.length) throw new Error((context ? context + ': ' : '') + problems.join('; '));
  }

  /** Marks an HTTP error as expected (negative tests only). */
  expectHttpError(urlPattern, status) {
    this.expectedHttpErrors.push({ url: urlPattern instanceof RegExp ? urlPattern : new RegExp(urlPattern), status });
  }

  clearLogs() {
    this.consoleErrors.length = 0;
    this.httpErrors.length = 0;
    this.requests.length = 0;
    this.dialogs.length = 0;
  }

  close() {
    if (this.off) this.off();
    this.cdp.close();
  }
}

/**
 * Host simulator expectations shared by every test page. The favicon is absent;
 * `/api/network` is served from the in-memory host store, so it is no longer an
 * expected error. `/api/firmware` is device-only and answers 404 on the host.
 */
function baselineExpectations(page) {
  page.expectHttpError(/\/favicon\.ico$/, 404);
  page.expectHttpError(/\/api\/firmware$/, 404);
  return page;
}

/** Self test: boot simulator + browser, load the UI, verify preview pixels. */
async function selfTest() {
  const simulator = await new Simulator().start();
  const browser = await launchChromium();
  try {
    const page = baselineExpectations(await browser.newPage(simulator.base));
    await page.waitForSnapshot('Array.isArray(state.playlist?.entries)', 8000, 'first snapshot');
    const effects = await page.evaluate('catalog.length');
    const shot = await page.screenshot('self-test');
    const painted = await page.evaluate(`(() => {
      const data = previewCtx.getImageData(0, 0, 32, 32).data;
      let lit = 0;
      for (let i = 0; i < data.length; i += 4) if (data[i] || data[i+1] || data[i+2]) lit++;
      return lit;
    })()`);
    page.assertClean('self test');
    console.log(JSON.stringify({
      ok: true,
      browser: browser.browser,
      base: simulator.base,
      effects,
      litPixels: painted,
      screenshot: shot,
      consoleErrors: page.consoleErrors.length,
    }, null, 2));
  } finally {
    browser.stop();
    await simulator.stop();
  }
}

module.exports = {
  ARTIFACT_DIR,
  Cdp,
  Page,
  Simulator,
  baselineExpectations,
  freePort,
  launchChromium,
  selfTest,
  sleep,
  waitForHttp,
};

if (require.main === module) {
  if (process.argv.includes('--self-test')) {
    selfTest().then(() => process.exit(0)).catch(error => {
      console.error(error);
      process.exit(1);
    });
  } else {
    console.log('usage: node scripts/gui-e2e/harness.cjs --self-test');
    process.exit(2);
  }
}
