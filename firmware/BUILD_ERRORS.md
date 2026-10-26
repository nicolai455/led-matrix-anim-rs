# Build status

The initial ESP-IDF adapter/API errors have been resolved. The reproducible
verification command is `./build_firmware.sh all release`; it must generate ELF
and binary images for ESP32, ESP32-bare, and ESP32-S3 and pass native SDK
partition checks plus real binary-size checks. See `../VERIFICATION.md` for
host acceptance checks and hardware limitations.

Known toolchain workaround: microphone settings deserialize JSON numbers through
a checked `f64` visitor before narrowing to `f32`. This avoids Xtensa's optimized
float-visitor instruction-selection bug while keeping firmware/control packages
size-optimized. Release builds disable LTO. Do not infer that a host compile
verifies the ESP-only adapter code.
