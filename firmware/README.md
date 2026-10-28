# Matrix Firmware (ESP32 and ESP32-S3)

Real ESP-IDF std Rust firmware for the 32x32 WS2812B LED Matrix, supporting ESP32-WROOM (standard and bare profiles) and ESP32-S3 (N16R8).

## Architecture & Responsibilities

- **Package**: `matrix-firmware` (standalone workspace under `rust/firmware`).
- **Dependencies**: Reuses `matrix-core`, `matrix-effects`, and `matrix-control` (with `default-features = false` to avoid host networking bloat).
- **Driver Layer**: ESP-IDF (`esp-idf-svc`, `esp-idf-hal`, `esp-idf-sys`).
- **Pacing**: Render is a dedicated task backpressured directly by the completion of physical LED output (RMT transmission). Network (HTTP, MQTT, Wi-Fi) runs on independent tasks and remains responsive.

## Target Profiles & Wiring

### 1. ESP32-WROOM (`esp32`)
- **Board Name**: `"ESP32-WROOM"`
- **4-Pin Parallel LED Output**:
  - Panel 0: GPIO 4
  - Panel 1: GPIO 16
  - Panel 2: GPIO 17
  - Panel 3: GPIO 18
- **Single-Chain Fallback**: GPIO 4 (all 1024 LEDs)
- **I2S Microphone (INMP441 / ICS-43434)**:
  - SD (Data In): GPIO 32
  - WS (Word Select / LRCL): GPIO 15
  - SCK (Bit Clock): GPIO 14
  - L/R: Ground (Left channel)
- **Boot Button**: GPIO 0

### 2. ESP32-WROOM Bare (`esp32-bare`)
- Same pinout as ESP32-WROOM, with microphone disabled (`audio_source` defaults to Off/Synthetic).

### 3. ESP32-S3 N16R8 (`esp32s3`)
- **Board Name**: `"ESP32-S3 N16R8"`
- **Flash/PSRAM**: 16MB Flash, 8MB Octal PSRAM (OPI)
- **Default 4-Pin Parallel Output**:
  - Panel 0: GPIO 8
  - Panel 1: GPIO 3
  - Panel 2: GPIO 10 (Hardware correction: GPIO 46 on ESP32-S3 is input-only / strapping pin)
  - Panel 3: GPIO 9
- **Single-Chain Mode**: GPIO 8
- **Microphone**: Optional / unpopulated on bare S3 (can map to GPIO 4, 5, 6 when wired).

### ESP32-S3 GPIO 46 Hardware Analysis & Correction
In the original PlatformIO `BoardPins.h`, `LED_PANEL_PINS[4]` was defined as `{8, 3, 46, 9}`.
However:
1. On ESP32-S3, **GPIO 46 is strictly an input-only strapping pin** (ROM boot mode control).
2. The silicon lacks output driver circuitry (`SOC_GPIO_VALID_OUTPUT_GPIO_MASK` does not include GPIO 46 for output matrix driving).
3. In Rust `esp-idf-hal`, `Gpio46` only implements `InputPin`, so the Rust type system prevents configuring `Gpio46` as an RMT output pin.
4. Attempting to drive WS2812B data out of GPIO 46 fails silently or causes bus contention during reset.
**Resolution**: The firmware validates all output pins; when targeting ESP32-S3 parallel mode, Panel 2 defaults to GPIO 10 (or user-configured valid output pin) while clearly rejecting invalid input-only pins like GPIO 46.

## Key Technical Adaptations

1. **RMT Memory & Streaming Encoder**:
   - WS2812B requires 24 pulses per LED. Across 4 panels (1024 LEDs total = 256 LEDs/panel), precomputing symbols would require `256 * 24 * 4 * 4 bytes = 98,304 bytes` (96 KB) of DMA RAM. On a WROOM with ~65KB free heap, this crashes or causes allocation failure.
   - We use ESP-IDF 5's streaming bytes encoder (`rmt_new_bytes_encoder`). Only raw GRB byte buffers (256 * 3 = 768 bytes per panel) are buffered; symbols are converted dynamically into small RMT hardware FIFO blocks.
   - Parallel mode starts transmission concurrently across all 4 RMT channels (`rmt_transmit`), then waits for all channels to complete (`rmt_tx_wait_all_done`), providing true parallel transmission and hardware frame backpressure.

2. **Device Identity & Efuse MAC**:
   - Formatted as `format!("matrix_{:012x}", mac_u64)` using the little-endian 64-bit integer representation from `esp_efuse_mac_get_default()`.
   - Matches legacy `snprintf("matrix_%012llx", (unsigned long long)ESP.getEfuseMac())` byte-for-byte to prevent duplicate entities or changed Home Assistant IDs.

3. **Power Budget & Quiescent Current Limiting**:
   - Enforces a 5V power budget (default 12,000 mA, configurable 1,000 mA to 60,000 mA).
   - Accounts for 1mA dark quiescent current per WS2812B pixel (1024 mA total quiescent).
   - Dynamically scales RGB brightness down if requested frame power exceeds limit.
   - **Always boots dark (power state OFF)** to protect USB ports from high initial current draw, including on persistent state restore.

4. **I2S Microphone (IDF 5) & Capture Worker Thread**:
   - Uses ESP-IDF 5 `esp_driver_i2s` standard RX mode.
   - Dedicated audio capture worker thread prevents I2S DMA reads from blocking the render actor loop.
   - Snapshot is non-blocking and detects hardware staleness (>250ms without fresh blocks sets `available: false`).
   - Removes all fake fallback silence mocks masquerading as valid audio data.
   - Dynamically reconfigures I2S slot masks when `right_channel` setting changes.
   - Implements `AudioInput::configure(&mut self, &MicSettings)` hook for runtime parameter delivery.

5. **Safe Real OTA with Hardware Handshake**:
   - Uses ESP-IDF OTA partition switching (`esp_ota_begin`, `esp_ota_write`, `esp_ota_end`, `esp_ota_set_boot_partition`).
   - Two-phase handshake confirms dark LED output and stopped I2S DMA before any flash erase/write operation begins.
   - Validates firmware header magic (`0xE9`) on initial chunk.
   - Safely calls `esp_ota_abort` and restores hardware on any transport or write error.

6. **Storage Persistence (NVS & SPIFFS)**:
   - Mounts VFS SPIFFS filesystem at `/spiffs` for streaming LFX asset files (`/spiffs/animations/example.lfx`).
   - Implements `StorageBackend` trait for `FirmwareNvs` to back `ConfigStore` and `StateStore`.
   - Restores persistent settings and panel configuration on boot while strictly preserving initial OFF power state.

7. **Web UI & REST API**:
   - Server configured with `uri_match_wildcard: true`.
   - Dispatches `GET /`, wildcard `GET /api/*`, and wildcard `POST /api/*` with bounded reads (4KB cap) to `matrix_control::api::handle_api_request`.
   - Streaming OTA endpoint at `POST /update`.

8. **MQTT & Home Assistant Discovery**:
   - Dedicated event queue and worker thread prevents deadlocks inside ESP-IDF MQTT callback.
   - Assembles multi-chunk / fragmented MQTT payloads before dispatch.
   - Configures LWT availability topic (`offline`) and publishes birth status (`online`).

## Building & Partition Verification

### Prerequisites
- ESP Rust toolchain (`nightly` with Xtensa support, installed via `espup` or available in `$RUSTUP_HOME/toolchains/esp`).
- `ldproxy` (`cargo install ldproxy` or in PATH).
- `cmake` (>= 3.24) and `ninja`.
- `esptool.py` (ESP-IDF python environment or PlatformIO tool-esptoolpy).

### Build Script (`./build_firmware.sh`)
The build script automatically discovers toolchains from standard `~/.rustup`, `~/.cargo`, `~/.espressif`, PlatformIO, Nix store caches, or custom environment variables. It enforces partition table validation, converts ELF to flashable binary images via `esptool.py`, and validates real partition fit with zero overlap.

Release builds automatically select size optimization for ESP32/ESP32-bare and speed optimization for ESP32-S3. S3 builds override Rust and application packages to optimization level 3, omit the standard library's `optimize_for_size` feature, and use ESP-IDF performance optimization. Thin LTO and Xtensa compiler workarounds remain enabled. Use this script for target-specific optimization; direct `cargo build --release` retains the size-oriented defaults.

```bash
# Build ESP32-WROOM release image and check partition fit
./build_firmware.sh esp32 release

# Build ESP32-WROOM Bare profile (microphone disabled)
./build_firmware.sh esp32-bare release

# Build ESP32-S3 N16R8 release image
./build_firmware.sh esp32s3 release

# Build all targets in release mode
./build_firmware.sh all release
```

### Partition Layout & Sizing
- **ESP32 4MB (`partitions.csv`)**:
  - `nvs`: `0x9000` (20 KB)
  - `otadata`: `0xe000` (8 KB)
  - `app0` (ota_0): `0x10000`, size `0x1e0000` (1920 KB / 1.875 MB)
  - `app1` (ota_1): `0x1f0000`, size `0x1e0000` (1920 KB / 1.875 MB)
  - `spiffs`: `0x3d0000`, size `0x30000` (192 KB >= 128 KB required for animations/assets)
  - The verified size-optimized application is 1,691,216 bytes, leaving 274,864 bytes per OTA slot. The build rejects oversize images; use its printed counts after future changes.
- **ESP32-S3 16MB (`partitions_16MB.csv`)**:
  - `nvs`: `0x9000` (24 KB)
  - `otadata`: `0xf000` (8 KB)
  - `app0` (ota_0): `0x20000`, size `0x400000` (4096 KB / 4 MB)
  - `app1` (ota_1): `0x420000`, size `0x400000` (4096 KB / 4 MB)
  - `storage` (`spiffs`): `0x820000`, size `0x7e0000` (8064 KB)
  - Real binary image size: ~1,953 KB, safely fitting into the 4,096 KB OTA slots.
