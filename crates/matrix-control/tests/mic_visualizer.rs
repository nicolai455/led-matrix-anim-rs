use matrix_control::WEB_UI;
use std::process::Command;

#[test]
fn mic_bars_hold_steady_between_polls_and_decay_when_stale() {
    let start = WEB_UI.find("let micPlaybackHistory =").unwrap();
    let end = WEB_UI.find("function animatePreview(now)").unwrap();
    let script = format!(
        r#"
const assert = require('node:assert/strict');
let state = {{ audio_source: 'Mic' }};
let now = 0;
const performance = {{ now: () => now }};
const cols = Array.from({{length: 16}}, () => ({{style: {{height: '0%'}}}}));
const document = {{hidden: false, querySelectorAll: () => cols}};
{}
queueMicHistory({{audio_source: 'Mic', bands: Array(16).fill(0.8)}});
now = 16; animateMicBands(now);
assert.ok(parseFloat(cols[0].style.height) < 20, 'new readings should not jump instantly');
for (now = 32; now <= 480; now += 16) animateMicBands(now);
assert.ok(parseFloat(cols[0].style.height) > 70, `steady sample faded: ${{cols[0].style.height}}`);
for (; now <= 3000; now += 16) animateMicBands(now);
assert.ok(parseFloat(cols[0].style.height) < 5, `stale sample did not decay: ${{cols[0].style.height}}`);
"#,
        &WEB_UI[start..end]
    );
    let output = Command::new("node").arg("-e").arg(script).output().expect("node is required for the browser behavior test");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
