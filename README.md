# Rust LED matrix implementation

The Rust port of the LED matrix firmware and simulator (`led_matix_anim`).
It includes ESP-IDF firmware for ESP32 and ESP32-S3, a browser WebAssembly preview,
and a standalone host simulator. See [verification](VERIFICATION.md) for acceptance checks.

## Run on a computer

```sh
cargo run -p matrix-control --bin matrix-simulator -- --listen 127.0.0.1:8080
```

Open the printed address. Output boots **OFF**. The UI supports effects,
parameters, brightness/color, text/scrolling, audio source, microphone settings,
panel configuration, state saving, preview, FPS, and diagnostics. The simulator
has no real microphone or LED output; its sink models WS2812 wire throughput.

Optional arguments:

- `--config PATH`: panel JSON, using the original `panel_config.json` schema.
- `--state PATH`: persistent runtime settings; power always restores OFF.
- `--animation PATH`: stream an existing 32×32 RGB888 `LFX1` animation.
- `--network PATH`: broker configuration using `mqttHost`, `mqttPort`, `mqttUser`,
  and `mqttPassword`. Do not check credentials into Git.
- `--device-id ID`: MQTT identity, using letters, digits, `_`, and `-` only.

MQTT uses `ledmatrix/<id>/...` with retained Home Assistant discovery,
availability, and state. HTTP/API handlers and the broker worker never drive
LEDs directly. The render actor drains bounded command batches, publishes cached
state, and waits for its output adapter; there is no engine target FPS.

## Browser preview

The preview scales up to 448px and runs the same Rust effects in WebAssembly,
independently of state polling. Browser mode does not request pixel frames;
`/api/audio` supplies compact external microphone telemetry when needed.
Synthetic audio is generated locally. Hidden tabs pause polling and drawing.
This is a simulation, not a phase/randomness-exact mirror of the LEDs. Select
**Device frames** for exact output; uploaded **Frames** animations use that
fallback automatically. **Off** disables preview rendering.

After changing core/effects, rebuild the compressed embedded module with
`python3 scripts/build-preview.py` (requires Rust's `wasm32-unknown-unknown`
standard library; `CARGO`/`RUSTC` can select the toolchain). The browser decompresses
it once. Firmware streams the compressed flash asset without a whole-module heap
copy. Recheck firmware OTA fit after rebuilding.

Frontend checks: `node scripts/test-preview-ui.cjs`. For the real Chromium smoke
test, run the simulator at `127.0.0.1:18080` and a disposable headless Chromium
with CDP on port `19223`, then run `node scripts/test-preview-browser.cjs`.

## Structure

| Path | Responsibility |
| --- | --- |
| `crates/matrix-core` | Frame/pixel types, effect contract, engine, commands, geometry, current limiting, audio DSP, LFX streaming, runtime serialization |
| `crates/matrix-effects` | Effect implementations, metadata, shared drawing helpers, builtin registration |
| `crates/matrix-control` | Render actor, bounded commands, shared UI/API, discovery, storage abstractions, host HTTP/MQTT adapters |
| `firmware` | Standalone ESP-IDF application and ESP32/ESP32-S3 hardware adapters |
| `tests` | Independent original-catalog/geometry fixtures and integration/system checks |

## Add an effect

1. Add a module under `crates/matrix-effects/src/` implementing
   `matrix_core::Effect`. Use a unique stable ID and display name.
2. Define parameter metadata and implement validation/getters/setters together.
   Runtime UI controls and Home Assistant entities are generated from that
   metadata; no separate control implementation should be needed.
3. Use `RenderContext::delta_seconds` or elapsed timestamps for motion, not a
   presumed frame count. Bound work after long pauses and handle clock resets.
   Use the supplied logical row-major frame; panel mapping belongs in output.
4. Export/register the effect in `matrix-effects/src/lib.rs` and add tests for
   validation, silence/audio behavior, elapsed time, and visible parameter impact.

Keep per-frame stack use small for ESP32. Avoid large temporary float frames and
unbounded particle/asset allocation. Network or filesystem errors must not turn
into fake successful output. `FramesEffect::from_source` accepts a streaming
`FrameSource`, so long animations need only one decoded frame in RAM.

## Firmware

See [`firmware/README.md`](firmware/README.md) for wiring. Hardware builds use
Espressif's Xtensa Rust toolchain and ESP-IDF, not the host Rust compiler. The build script detects standard espup installations and the local cached Nix
toolchain. Build all hardware profiles (without flashing) with:

```sh
cd firmware
./build_firmware.sh all release
```

It emits application `.bin` images and checks their size against the OTA slots.
Prepare a separate staged asset directory containing `config/network.json`,
optional `config/panel_config.json`, and optional `animations/*.lfx`, then run
`python3 build_assets.py --target esp32 --source /path/to/staged-assets` (or
`--target esp32s3`). Asset generation does not flash anything. Rust uses SPIFFS,
not the original Arduino LittleFS image. NVS overrides the staged panel config
once configuration has been saved through the UI. Raw firmware OTA is
`POST /update` with the binary as the request body and a Content-Length header;
ArduinoOTA upload transport is not supported.

Host and cross-build verification does not replace a physical board test.

The S3 adapter changes the original invalid GPIO46 output assignment to GPIO10;
that requires checking the actual wiring before any flash. Do not power an
unisolated matrix from ordinary USB and an external supply at the same time.

## Tests

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --manifest-path tests/Cargo.toml
cargo build -p matrix-control --bin matrix-simulator
python3 tests/system_test.py  # requires local Mosquitto tools
```

The broker test creates disposable loopback-only services; it does not use the
matrix's saved credentials or attached hardware. Physical LED timing, microphone
capture, OTA recovery, and network memory headroom still need board testing.
