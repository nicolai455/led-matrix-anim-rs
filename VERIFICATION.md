# Rust migration verification

## Verified automated checks

Run from `rust/`:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --manifest-path tests/Cargo.toml
cargo test --manifest-path firmware/Cargo.toml
cargo clippy --manifest-path firmware/Cargo.toml --all-targets -- -D warnings
cargo build -p matrix-control --bin matrix-simulator
python3 tests/system_test.py
```

The workspace has 53 tests; the independent test workspace has eight; firmware
host checks have six. These cover catalog/parameter parity, original geometry,
command atomicity, configuration validation, audio DSP/staleness, variable-time
effects, streamed frame playback, restart persistence/boot OFF, bounded queues,
slow output isolation, settings hooks, and blackout timeout behavior.

The real Mosquitto test launches disposable loopback-only services. It verifies
21 effects, 70 parameters, 77 retained discovery entities, HTTP/MQTT commands,
reconnect, HA-online rediscovery, and responsiveness while the broker is down.
The latest parent run measured a 32.74 ms maximum command/state round trip with
the broker offline. This is a local simulator result, not a board benchmark.

System Chromium also rendered the live simulator UI, including its catalog,
connection status, FPS, microphone controls, and panel settings.

## Cross-build acceptance

`firmware/build_firmware.sh all release` builds ESP32, ESP32 bare, and ESP32-S3,
creates real application images with Espressif esptool, checks native SDK custom
partition configuration, and rejects application images larger than the OTA
slot. Both ESP32 variants use 1,966,080-byte slots; S3 uses 4,194,304-byte slots.
The final parent-run release builds and real image checks passed:

| Target | Application bytes | Slot bytes | Remaining bytes |
| --- | ---: | ---: | ---: |
| ESP32 | 1,736,112 | 1,966,080 | 229,968 |
| ESP32 bare | 1,736,112 | 1,966,080 | 229,968 |
| ESP32-S3 | 1,731,264 | 4,194,304 | 2,463,040 |

Image paths and SHA-256 values are recorded in ignored build output
`firmware/target/verified-images.json`. These images include restored `anyhow`,
lazy effect runtimes, compact catalog caching, and streamed HTML/catalog replies.
Future additions still run the image-fit check rather than assuming they fit.

Microphone settings use checked `f64` deserialization followed by narrowing to
`f32`, avoiding an Xtensa compiler bug in serde's optimized `f32` visitor.
This preserves the external JSON schema and allows all application crates to
be size-optimized instead of disabling optimization for firmware/control.
All three size-optimized profiles were rebuilt successfully; their final sizes
and hashes are recorded in `firmware/target/verified-images.json`. Regression
checks cover deserialization defaults/round trips/overflow, live microphone
settings reporting, and oversized HTTP request rejection.

SPIFFS image generation was exercised on both partition sizes using disposable
configuration and an actual LFX1 file. No user's credentials were read.

## Not yet verified on hardware

A safely isolated USB-only ESP32-D0WD-V3 was flashed with the bare profile.
Boot, Wi-Fi, HTTP state/catalog/UI, all 21 effect selections and OFF readback passed:
48 requests, maximum 196.5 ms in that short run. Serial logs showed approximately
90 KB free heap and 53.6 KB minimum free heap. Lazy effect host accounting measured
36,371 bytes of runtime allocation released on deactivation (host sizes, not ESP32).

Further repeated-switch probes reproduced Load/StoreProhibited and allocation
failures. Increasing the HTTP stack alone did not solve them; a 24 KB diagnostic
stack retained over 18 KB of headroom. HTTP now uses 16 KB. MQTT discovery emits
one payload at a time instead of retaining all 76 records; parameter settings and
metadata remain unchanged. Direct switch_to_effect now also deactivates the old
runtime (covered by a regression test).

After those changes, two consecutive independent boots completed 40 alternating
Rainbow/Black Hole switches and 100 HTTP requests each: 80 switches / 200 requests,
no mismatches or crash markers, persistent particle_count=16, UI/catalog served,
and OFF readback. Maximum request times were 273.2 and 372.1 ms. Minimum free heap
was 28,292 and 29,680 bytes. These short tests are not long-duration acceptance;
the earlier failures are not conclusively attributed to discovery allocation.
Physical RMT waveform/LEDs, microphone capture and OTA recovery remain unverified.

The S3 parallel pin assignment changes GPIO46 (input-only) to GPIO10. Check the
wiring before flashing. Do not connect ordinary USB and external LED power
together on an unisolated power path.

## Preservation

The existing Arduino tree and pre-existing uncommitted changes are retained.
The original native tests passed except an existing `render_loop_native` C++
harness compilation failure against diagnostic calls. The original files were
not edited to conceal it. Rust API paths/state-save formats intentionally differ
where documented; raw HTTP firmware OTA replaces ArduinoOTA transport, and
SPIFFS replaces LittleFS for Rust assets. Do not upload an Arduino filesystem
image to the Rust partition.
