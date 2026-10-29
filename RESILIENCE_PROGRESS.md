# Resilience implementation state

Requested order: **5 → 4 → 3 → 2**. Code/build only; no device flash or hardware verification authorized in this implementation run.

| Phase | Scope | State | Verification |
|---|---|---|---|
| 5 | ESP32-S3 parallel DMA LED output | Implemented; hardware validation pending. | Host waveform (ASan/UBSan), firmware host tests, S3 release cross-build and partition fit pass. |
| 4 | Predictable output path and cached HTTP telemetry | Implemented; hardware validation pending. | Workspace and firmware host tests pass; S3 release cross-build and partition fit pass. |
| 3 | Startup-validated OTA rollback | Implemented; hardware validation pending. | Policy and full host suites pass; S3 release cross-build and partition fit pass. |
| 2 | Independent progress supervisor and repeated-failure safe mode | Implemented; hardware validation pending. | Recovery state-machine and inactive-output tests, full host suites and S3 cross-build pass. |

## Constraints

- Preserve user changes, configured panel orientation/pins, restart controls and WROOM support.
- Keep S3 parallel output truly parallel; no serialized-panel or unchanged-frame workaround.
- Do not claim physical flicker resolution from builds or software counters.
- Preserve NVS and filesystem layout. Bootloader rollback changes require explicit deployment guidance; application OTA cannot upgrade an installed bootloader.
- Use nix-shell for builds. Add tests before implementation and cross-build the S3 target.

## Phase 4 checkpoint (verified in software)

The render actor captures typed, immutable state when configuration changes and sends bounded telemetry samples to a worker, which constructs and serializes JSON and publishes the cache. Periodic samples are skipped when the worker queue is full; command replies are retained and completed only after publication without blocking output. Capturing changed state and audio history still allocates/clones. Preview writes skip contention; no lock spans physical output. Existing external-audio cadence and history semantics are retained.

Verified in nix-shell: workspace and firmware host tests pass, including detached snapshot equivalence, synchronous readback, and output progress with a saturated worker and retained replies. S3 release image is 2,559,360 bytes, fitting the 4,194,304-byte slot with 1,634,944 bytes headroom. No device flashed; physical behavior remains unverified.

## Current checkpoint

All requested phases are implemented in order and software-verified. Final workspace and firmware host suites pass in nix-shell. Final S3 image: 2,584,752 bytes, 1,609,552 bytes OTA headroom. No device access or flash performed. Physical waveform, fault injection, safe-mode startup and bootloader rollback remain deployment gates.

## Phase 3 checkpoint

Pending OTA images are independently supervised from startup with a 30-second deadline. Confirmation requires boot dark transfer, local Wi-Fi driver and HTTP initialization, and five seconds of output progress with no output errors; association, internet, and MQTT are not required. Failure requests rollback when IDF reports a viable previous image, otherwise leaves the image unconfirmed and attempts to pause output in recovery. Confirmation/rollback errors are logged. Host policy tests cover deadlines, pending versus non-pending, stalled progress and errors; they do not emulate the IDF bootloader or real hardware.

The S3 build's resolved sdkconfig has `CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE=y`; release image is 2,566,400 bytes with 1,627,904 bytes slot headroom. Firmware and workspace host suites passed in nix-shell, as did S3 cross-build and `git diff --check`. The installed bootloader is unknown: app-only OTA does not upgrade it. Serial installation of a compatible bootloader and actual healthy/faulted pending-boot, outage and no-previous-image recovery tests remain explicit hardware gates; no flash was performed.

Phase 5 implementation: S3 parallel mode now uses a native LCD_CAM/GDMA component, reserving the IDF LCD platform slot and routing only the four configured data pins. Four precomputed lanes use 3.2 MHz samples, 1000/1110 coding and 320 µs reset; DMA descriptors and waveform reside in internal DMA-capable memory. Completion is bounded at 1 second; an abort stops both engines, drives pins low and requires reinitialization. WROOM and single-chain retain the existing hardened RMT backend. No API or physical pin is borrowed for LCD control.

Verified: host C waveform test passes with ASan/UBSan (test first failed to compile before encoder existed); `cargo test --manifest-path firmware/Cargo.toml --target x86_64-unknown-linux-gnu --tests` passes; nix-shell S3 release cross-build passes, image 2,532,400 bytes with 1,661,904 bytes slot headroom; `git diff --check` passes. No flash. Hardware waveform timing, actual parallel order, abort behavior, microphone coexistence and physical blackout are unverified and require device/logic analyzer testing. The waveform tests alone do not establish LED compatibility or physical flicker resolution.

## Phase 2 checkpoint (verified in software)

An independent supervisor watches atomic render and successful-output counters. Five seconds without progress triggers a restart; explicitly acknowledged output pauses are excluded from output-stall detection. A separate 60-second startup deadline covers initialization before the actor is attached. Neither polling path takes telemetry or NVS locks. Stable-run journal maintenance runs in a one-shot worker.

The NVS recovery journal arms each boot; three consecutive incomplete/unplanned runs latch safe mode on the next start. Planned API/OTA restarts disarm it; a stable minute clears the consecutive-failure count without disarming runtime fault tracking. A physical power cycle is conservatively counted as unplanned. Journal writes occur only at lifecycle transitions, not per frame. Failures before NVS initialization are outside this guard.

Safe mode skips LED and microphone peripheral initialization, retains networking and OTA, and reports output disabled rather than physical transfer success. `GET /api/recovery` exposes the latched state; `POST /api/recovery/retry` explicitly clears it and restarts. Normal reboot preserves the latch. Recovery upload needs no hardware quiesce because no LED peripheral was started; this does NOT guarantee previously latched LEDs are dark. Power-cycle panel power if needed. Configuration and geometry are retained.

The pending-image validator remembers a previously registered output handle, so a contended health read cannot discard the ability to request pause at its deadline. Pending images in inactive-output recovery are never falsely confirmed as healthy.

Review fixes: recovery OTA arms a partition-bound single normal-output trial while retaining failure history; startup deadline stays armed through network/HTTP initialization; stable journal maintenance retries until persisted. Recovery/reboot controls require `X-Matrix-Control: 1` and reject foreign browser origins (CSRF protection, not LAN authentication).

Next: hardware commissioning of parallel waveform/blackout and HTTP load, followed by induced stalls, safe-mode/retry/OTA, and rollback-capable bootloader deployment. Do not equate successful compilation with physical flicker resolution.
