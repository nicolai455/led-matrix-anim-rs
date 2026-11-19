use matrix_control::WEB_UI;
use std::process::Command;

/// The network card's JavaScript is the only place where the UI decides what to
/// send to `/api/network`. These tests execute exactly that slice of `WEB_UI` in
/// node with a stub DOM, so "open network", the setup-token header and the
/// status suffixes are covered without a browser.
fn network_script(body: &str) -> String {
    let start = WEB_UI
        .find("async function loadNetworkConfig()")
        .expect("loadNetworkConfig must exist");
    let end = WEB_UI
        .find("function renderPlaylistEntries(snapshot)")
        .expect("renderPlaylistEntries must exist");
    assert!(start < end, "network functions must stay contiguous");
    format!(
        r#"
const assert = require('node:assert/strict');
class El {{
  constructor(id) {{
    this.id = id; this.value = ''; this.checked = false; this.dataset = {{}};
    this.style = {{}}; this.textContent = ''; this.placeholder = ''; this.children = [];
    this.firstChild = null; this.className = ''; this.title = ''; this.type = '';
  }}
  appendChild(child) {{ this.children.push(child); this.firstChild = this.children[0]; }}
  removeChild(child) {{ this.children = this.children.filter(item => item !== child); this.firstChild = this.children[0] || null; }}
  setAttribute(name, value) {{ this[name] = value; }}
}}
const elements = {{}};
const document = {{
  hidden: false,
  getElementById(id) {{ return elements[id] || (elements[id] = new El(id)); }},
  createElement() {{ return new El('created'); }},
}};
const requests = [];
let firmwareResponse = {{}};
async function fetchJSON(url) {{
  if (url === '/api/firmware') return firmwareResponse;
  throw new Error('unexpected ' + url);
}}
async function fetch(url, options) {{
  requests.push({{ url, options }});
  if (url === '/api/network/scan') return {{ ok: true, json: async () => scanResponse }};
  return {{ ok: true, json: async () => ({{ config: {{ wifiProfiles: [] }} }}) }};
}}
let scanResponse = {{ networks: [], stale: false }};
let state = {{}};
{}
{body}
"#,
        &WEB_UI[start..end]
    )
}

fn run(script: &str) -> Result<(), String> {
    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .output()
        .expect("node is required for the browser behavior test");
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

#[test]
fn open_networks_are_saved_with_an_explicit_clear_and_no_password() {
    let script = network_script(
        r#"
(async () => {
  document.getElementById('networkSsid').value = 'cafe';
  document.getElementById('networkOpenNetwork').checked = true;
  await saveNetworkConfig();
  const post = requests.find(request => request.url === '/api/network');
  assert.ok(post, 'save must POST the configuration');
  const payload = JSON.parse(post.options.body);
  assert.equal(payload.wifiProfiles.length, 1);
  assert.equal(payload.wifiProfiles[0].ssid, 'cafe');
  assert.equal(payload.wifiProfiles[0].clearPassword, true);
  assert.equal(payload.wifiProfiles[0].password, undefined);
  assert.equal(document.getElementById('networkStatus').textContent.includes('Save failed'), false);
  // Validation must reject a password combined with "open network".
  document.getElementById('networkSsid').value = 'conflict';
  document.getElementById('networkPassword').value = 'long-enough-password';
  document.getElementById('networkOpenNetwork').checked = true;
  await saveNetworkConfig();
  assert.ok(document.getElementById('networkStatus').textContent.includes('open network has no password'),
    document.getElementById('networkStatus').textContent);
})().catch(error => { console.error(error); process.exit(1); });
"#,
    );
    if let Err(error) = run(&script) {
        panic!("{error}");
    }
}

#[test]
fn the_setup_token_is_sent_only_when_the_device_discloses_it() {
    let script = network_script(
        r#"
(async () => {
  firmwareResponse = { setupToken: 'ABCDEFGH' };
  await loadNetworkSetupToken();
  assert.equal(document.getElementById('networkTokenNotice').style.display, 'block');
  document.getElementById('networkSsid').value = 'home';
  document.getElementById('networkPassword').value = 'a-long-password';
  await saveNetworkConfig();
  let post = requests.find(request => request.url === '/api/network');
  assert.equal(post.options.headers['X-Matrix-Setup-Token'], 'ABCDEFGH');
  requests.length = 0;

  // A device outside the station subnet discloses no token: no header, and the
  // card must still be usable through the fallback AP.
  firmwareResponse = {};
  await loadNetworkSetupToken();
  assert.equal(document.getElementById('networkTokenNotice').style.display, 'none');
  document.getElementById('networkSsid').value = 'home2';
  document.getElementById('networkPassword').value = 'a-long-password';
  await saveNetworkConfig();
  post = requests.find(request => request.url === '/api/network');
  assert.equal(post.options.headers['X-Matrix-Setup-Token'], undefined);
})().catch(error => { console.error(error); process.exit(1); });
"#,
    );
    if let Err(error) = run(&script) {
        panic!("{error}");
    }
}

#[test]
fn live_profile_states_and_scan_results_are_rendered() {
    let script = network_script(
        r#"
(async () => {
  state = { wifi: { profiles: [ { ssid: 'home', state: 'connected' }, { ssid: 'garden', state: 'failed' } ] } };
  state.networkConfig = { wifiProfiles: [
    { ssid: 'home', priority: 5, hidden: false, passwordConfigured: true },
    { ssid: 'garden', priority: 1, hidden: false, passwordConfigured: false }
  ] };
  renderNetworkProfiles();
  const rows = document.getElementById('networkProfiles').children;
  assert.equal(rows.length, 2);
  assert.ok(rows[0].children[0].textContent.includes('connected'), rows[0].children[0].textContent);
  assert.ok(rows[1].children[0].textContent.includes('failed'));
  assert.ok(rows[1].children[0].textContent.includes('open network'));

  scanResponse = { stale: false, connectionMayDrop: true, networks: [
    { ssid: 'home', rssi: -48, auth: 'WPA2', channel: 6 },
    { ssid: 'cafe', rssi: -80, auth: 'open', channel: 1 }
  ] };
  await scanNetwork();
  const results = document.getElementById('networkScanResults').children;
  assert.equal(results.length, 2);
  assert.ok(results[0].children[0].textContent.includes('-48 dBm'));
  assert.ok(results[1].children[0].textContent.includes('(open)'));
  assert.ok(document.getElementById('networkScanStatus').textContent.includes('interrupts'));
  // Clicking "Use" fills the SSID and pre-ticks open networks only.
  results[1].children[1].children[0].onclick();
  assert.equal(document.getElementById('networkSsid').value, 'cafe');
  assert.equal(document.getElementById('networkOpenNetwork').checked, true);
  results[0].children[1].children[0].onclick();
  assert.equal(document.getElementById('networkOpenNetwork').checked, false);
})().catch(error => { console.error(error); process.exit(1); });
"#,
    );
    if let Err(error) = run(&script) {
        panic!("{error}");
    }
}

#[test]
fn the_missing_ap_password_warning_is_shown_and_the_text_no_longer_claims_ap_only_access() {
    assert!(
        WEB_UI.contains("Without one there is no recovery access point"),
        "the lockout warning must be present"
    );
    assert!(WEB_UI.contains("id=\"networkApWarning\""));
    assert!(WEB_UI.contains("It then rejoins") || WEB_UI.contains("it then joins the home network"));
    assert!(!WEB_UI.contains("reconnect at <code>http://192.168.4.1/</code>.</p>"));
}
