# Verification notes for network/audio/playlist work

## Current host verification

- `cargo test -p matrix-effects`: 25 unit tests plus catalog, timing, render/validation and lifecycle integration suites passed. The Space Invaders regressions cover the removed bunker and lower green invader row, live-alien shot origin, hit-frame removal, and edge-reversal march/wave reset.
- `cargo test -p matrix-core -p matrix-effects -p matrix-control` passed, including 10 playlist tests (explicit Play powers on, boot/restore remain off), 15 Control hardening tests and all Core/Effects suites.
- Firmware host tests: 35 unit tests and all integration suites passed; only existing `fetch_update` deprecation warnings. Heap-pressure regression tests confirm the direct `/api/network` POST route is shed under low internal/DMA memory before buffering its body.
- Node UI/preview suite: 24 tests passed. Chromium browser smoke passed (448 px desktop canvas; 309 px canvas on 375 px viewport; 1 WASM request, 0 frame requests, no exceptions).
- `cargo build --target wasm32-unknown-unknown --release -p matrix-preview` and `scripts/build-preview.py` passed after supplying Binaryen/Zopfli from a temporary Nix shell; checked-in `preview.wasm.gz` was rebuilt.
- Host simulator `/api/network` returned 503 as designed; it has no radio adapter.

`cargo clippy -p matrix-core -p matrix-effects -p matrix-control --all-targets -- -D warnings` is blocked by existing `field_reassign_with_default` findings in effect tests. Firmware Clippy also flags existing deprecations and new style lint findings; resolve/classify before release.

Core PCM tests cover deterministic 80/120/160 BPM, multiple sample rates, tempo change, silence/BPM expiry and non-finite samples. Playlist tests cover validation, timed transitions, loop/fake-time, pause/resume/power-off, auto-cycle coordination, persistence and boot OFF. Shooter collision ordering has a regression test.

## Known gaps / blockers

- Standard ESP32 release cross-build generated an image of 2,120,064 bytes, 153,984 bytes over the 1,966,080-byte OTA slot. ESP32-bare generated 2,101,776 bytes, 135,696 bytes over the same slot. Firmware release acceptance remains blocked on those profiles; only S3 is currently supported.
- The ESP32-S3 release cross-build completed; the current generated image is 2,733,168 bytes and fits the 4,194,304-byte app slot with 1,461,136 bytes headroom (65.2% used). The active hardware uses S3. This verifies compilation and partition fit only—not device startup, LED behavior, networking, microphone, or OTA at runtime. No hardware flash or runtime test was done.
- Firmware simulator has no radio-backed configuration adapter; `/api/network` returns 503 in host mode rather than fabricating Wi-Fi status.
- Boot code now converts stored ordered profiles and explicit AP config through `WifiConfig::from_network_config`; final source build/runtime verification remains target-blocked.
- A fresh unprovisioned device has no fallback AP until an owner provisioned WPA2 passphrase exists. No secure bootstrap mechanism is implemented; manufacturing or private local provisioning must cover first boot.
- Workspace `cargo fmt --check` is not clean due to existing formatting drift; only the touched effect was formatted. The combined formatter/test pipeline was re-run with targeted formatting; all tests passed.
- Workspace Clippy baseline has pre-existing effect-test `field_reassign_with_default` findings; earlier targeted core Clippy passed. Re-run and classify before release.
- `AUDIO_EFFECT_REVIEW.md` is a static source inventory, not per-effect signal-quality approval. Unified selectable reaction modes remain unimplemented.

The matrix engine stays OFF on power-up and never automatically resumes a saved playlist; an explicit playlist `Play` turns it on. Hardware release support is currently ESP32-S3 only; classic ESP32 release images exceed OTA slots. Automated builds/tests do not replace an on-device test; no device runtime was tested or flashed.
