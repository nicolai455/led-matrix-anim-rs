# Focused firmware review

## Applied and verified

- Replaced serde's direct `f32` microphone-setting visitor with checked `f64`
  deserialization/narrowing. String and `Value` round trips, defaults, negative
  values, overflow rejection, and all effect metadata remain tested. All
  application crates can now use size optimization.
- Corrected `/api/mic` to read the engine's `mic` field, not nonexistent
  `mic_settings`; a regression test verifies actual modified tuning values.
- Bounded host HTTP bodies to 4096 bytes, with 413 rejection and subsequent
  state-request responsiveness exercised against the real simulator.
- Added OTA preflight against the SDK's actual update-partition size, before
  erase/quiesce, rather than accepting any Content-Length up to 16 MB. Unit
  checks cover zero, boundary, and oversized lengths.
- Restored `anyhow::Result` at the user's request: removing it saved only about
  6.5 KB in the intermediate experiment, not the reviewer's 100–150 KB estimate.
- Built-in effect runtime objects are allocated on first render, dropped before
  switching, and preserve parameters while inactive. OFF does not render/reallocate.
- Consolidated LED output buffers into one heap allocation, saving 3072 bytes.
- Split panel config loading from startup to reduce stack pressure. Streamed UI
  HTML directly from flash and cached the catalog as compact JSON rather than
  retaining/cloning a large serde tree; the latter caused a reproduced board OOM.
- Streamed MQTT discovery one record at a time to avoid retaining all 76 payloads.
  Two repeated-switch bare-board runs passed after this and other memory fixes;
  causality for earlier corrupted-pointer crashes is not conclusively established.
- Kept MQTT command buffering bounded. Connection/disconnection state now uses
  an atomic generation/status outside the data queue so reconnect events cannot
  be dropped by a full command queue. Overflow increments a counter, warns,
  discards partial assembly, and requests rediscovery; it is no longer silent.

## Review claims corrected or deliberately not adopted

- The SDK's `esp_partition_write` explicitly rejects writes beyond a partition.
  The reviewer's claim that an oversized OTA upload would corrupt adjacent
  partitions was not supported. Early length rejection is still worthwhile to
  avoid erasing/writing a slot for an image that cannot fit.
- Larger MQTT queues do not guarantee delivery under arbitrary flood and use
  more WROOM RAM. The bounded policy and observable loss are intentional; a
  physical burst/fragment/reconnect test remains necessary.
- Recovering a poisoned NVS mutex without checking invariants could hide a
  partially failed operation. The error remains explicit; no unconditional
  `into_inner` recovery was added. Firmware uses abort-on-panic in release.

## Remaining physical checks

Verify sustained four-channel RMT, microphone capture/channel selection, WROOM
heap/stack high-water marks, actual MQTT burst behavior, and successful/failed
OTA transfers on a safely powered board. A bare ESP32 was flashed and short HTTP/all-effect checks passed; see
`VERIFICATION.md` for measured results and unresolved soak behavior.
