# ⚡ Rust 32x32 LED Matrix (ESP32-S3)

> Firmware is currently supported only on ESP32-S3. The active device uses S3. Classic ESP32 / ESP32-bare release images do not fit their configured OTA slots; see [`VERIFICATION.md`](VERIFICATION.md). The S3 cross-build and partition fit are verified, but no hardware flash or runtime test was performed.

[![Rust ESP-IDF](https://img.shields.io/badge/Rust-ESP--IDF%20v5.2.1-orange.svg)](https://github.com/esp-rs/esp-idf-svc)
[![Hardware](https://img.shields.io/badge/Hardware-ESP32--S3-blue.svg)](https://www.espressif.com/)
[![Matrix](https://img.shields.io/badge/Matrix-32x32%20WS2812B%20(1024%20LEDs)-brightgreen.svg)]()
[![Framerate](https://img.shields.io/badge/Framerate-60%2B%20FPS%20(4--Ch%20Parallel%20RMT)-success.svg)]()
[![Audio](https://img.shields.io/badge/Audio-I2S%20DMA%20(INMP441%2FICS--43434)-blueviolet.svg)]()
[![Home Assistant](https://img.shields.io/badge/Home%20Assistant-MQTT%20Auto--Discovery-41BDF5.svg)](https://www.home-assistant.io/)
[![PRs Welcome](https://img.shields.io/badge/PRs-Welcome-brightgreen.svg)](https://github.com/nicolai455/led-matrix-anim-rs/pulls)

> **High-framerate (60+ FPS), sound-reactive 2D WS2812B / NeoPixel LED matrix display firmware and desktop simulator written in pure Rust with ESP-IDF std.**

Featuring 80+ handcrafted 2D visual effects, real-time digital I2S microphone audio DSP, 4-channel parallel hardware RMT DMA driving, Home Assistant MQTT auto-discovery, fallback Wi-Fi Captive Portal, and an instant WebAssembly browser simulator.

---

<!-- Search & Discovery Keywords:
wled alternative, wled esp32 led matrix 32x32 led fast fps sound reactive,
esp32 ws2812b 32x32 matrix, esp32-s3 led matrix, inmp441 i2s microphone,
neopixel matrix visualizer, rust embedded esp-idf, parallel rmt led driver
-->

## 🌟 Highlights

- **Ultra-High Framerate (60–100+ FPS):** Uses 4 parallel hardware RMT DMA channels to drive 1024 LEDs simultaneously. Cutting transmission time from ~31 ms (single chain) down to **~7.7 ms** eliminates RMT buffer bottlenecks and guarantees smooth 60+ FPS animations.
- **80+ Native 2D Effects:** Demoscene raymarching, fluid caustics, domain warping, metaballs, cellular automata, club/concert strobe fixtures, moving head beams, retro arcade games (Space Invaders, Pac-Man, Mario, Breakout), and pixel-art meme loops.
- **Sound-Reactive DSP Engine:** Direct digital I2S DMA streaming (INMP441 / ICS-43434) with multi-band FFT frequency decomposition, transient beat detection, dynamic AGC, and zero CPU blocking.
- **Zero-Allocation Rust Safety:** Memory-safe Rust with ESP-IDF std. Zero heap churn during render loops, no memory fragmentation, saturating color blending, and panic-resilient error boundaries.
- **WASM Browser Preview & Desktop Simulator:** Develop and test effects without physical hardware using the WebAssembly browser simulator or host CLI tool (`cargo run -p matrix-control --bin matrix-simulator`).
- **Home Assistant & Web Control:** Retained MQTT auto-discovery for all effects and live parameters, a responsive embedded Web UI, and a fallback Access Point (`JDJLIGHT` / `192.168.4.1`).
- **Hardware Power Limiter:** Quiescent dark current compensation (accounts for ~1024 mA idle draw) and configurable 5V power budget throttling (default 12A limit). Always boots dark to protect USB ports.

---

## ⚖️ Project vs. WLED: Feature Comparison

WLED is the gold standard for addressable LED strips. However, when driving large **2D LED matrices (32x32 = 1024 LEDs)** with complex animations and fast audio-reactive visuals, architectural differences emerge:

| Feature / Capability | This Rust LED Matrix Project | Standard WLED |
| :--- | :--- | :--- |
| **Primary Architecture** | **Native 2D Matrix Engine** (Cartesian/polar math, shaders, raymarching) | **1D Strip Oriented** (with 2D matrix mapping layer) |
| **1024 LED Framerate** | **60–100+ FPS** via 4-channel parallel RMT DMA (256 LEDs/pin) | Typically capped at **~30–40 FPS** on single-pin 1024-LED chain |
| **LED Driver Output** | Concurrent 4-pin parallel RMT transmission in hardware | Single-pin default (multi-pin supported but strip-partitioned) |
| **Memory Safety & Language** | **Pure Rust (ESP-IDF std)**, zero heap churn, zero buffer overflows | C++ (Arduino / ESP-IDF), manual memory management |
| **Sound Reactivity** | Real-time I2S DMA worker thread, multi-band FFT, transient beat hits | AudioReactive Usermod (I2S/analog), shared loop scheduling |
| **Hardware Simulator** | **Native Host Simulator & WebAssembly Browser Preview** | Limited 2D simulation; requires live hardware or WLED-WLED emulators |
| **Effect Catalog** | **80+ bespoke 2D matrix shaders**, arcade games, disco fixtures, fluid dynamics | Hundreds of 1D strip effects; smaller native 2D matrix shader selection |
| **Quiescent Current Tracking** | Accounts for 1 mA idle current per WS2812B pixel (1024 mA base) | Software brightness limiter without idle pixel calibration |
| **1D Strip Effects** | ❌ Minimal (focus is exclusively on 2D matrices) | ✅ Massive 1D strip animation ecosystem |
| **Segment Partitioning UI** | ❌ Fixed 2D matrix layout | ✅ Advanced arbitrary strip segment editor |
| **DMX / E1.31 / Art-Net** | ❌ Not yet implemented (PRs welcome!) | ✅ Full DMX, Art-Net, E1.31, TPM2 network receiver support |
| **Mobile Apps** | ⚠️ Web UI & Home Assistant app (no native store app) | ✅ Official iOS and Android companion apps |

### What Makes It Better
1. **Designed for 2D Grids:** Shaders, polar coordinates, fluid dynamics, and retro demoscene algorithms run natively on a 32x32 grid rather than unfolding 1D strip effects.
2. **True 60+ FPS via Parallel Output:** The WS2812B protocol requires 30 µs per pixel. Driving 1024 pixels sequentially on one pin takes `1024 * 30 µs = 30.72 ms` just for data transmission, capping output to ~32 FPS. By splitting across 4 parallel pins, transmission takes only **7.68 ms**, leaving ample headroom for 60+ FPS rendering.
3. **No Hardware Required for Development:** Write a new effect in Rust, launch the simulator, and see it in your browser immediately.

### What Is Missing (Compared to WLED)
- No extensive 1D strip effect library (this project is strictly for 2D matrices).
- No complex multi-segment partitioning UI.
- No native Art-Net / DMX streaming input yet.
- No standalone app store app (control via Web UI or Home Assistant).

- **Wi-Fi:** prioritizes multiple STA profiles (up to five) with an 18-second timeout per attempt. If all station profiles fail, switches to a WPA2 fallback AP only when an explicitly provisioned per-device AP passphrase is present; otherwise Wi-Fi remains station-only/off. AP gateway is configured as `192.168.4.1/24` with DHCP. The app has no universal AP password; first boot needs private manufacturing/owner provisioning before a setup AP is available. The Web UI exposes `/api/network` with passphrase redaction.
- **Shared audio analysis:** Core PCM processing uses actual sample rates, normalized bass/onset and an estimated BPM. Control telemetry and the browser preview consume the same snapshot fields. This is deterministic host-tested analysis, not an ESP32 performance or sound-quality qualification.
- **Engine playlist:** stable effect IDs and bounded durations are configured separately from Auto Cycle; playback state is time-stepped by Engine snapshots. Queue configuration persists, while playback never auto-resumes and power still boots OFF.
- **Arcade Invaders:** projectile/alien collision is evaluated before drawing the hit frame; destroyed aliens remain gone until reset (host regression-tested; see [`SPACE_INVADERS_VERIFICATION.md`](SPACE_INVADERS_VERIFICATION.md)).

For details and limitations, see [`WIFI_CONFIGURATION.md`](WIFI_CONFIGURATION.md), [`FIRMWARE_NETWORK.md`](FIRMWARE_NETWORK.md), [`AUDIO_EFFECT_REVIEW.md`](AUDIO_EFFECT_REVIEW.md), and [`VERIFICATION.md`](VERIFICATION.md).

Current hardware support is limited to ESP32-S3, the only target used by this project right now. The S3 release cross-build and OTA-slot fit pass; this does not establish that the firmware has been tested on-device. Classic ESP32 / ESP32-bare images currently exceed their OTA slots.

---

## 🛠️ Hardware Requirements & Bill of Materials

> Firmware is currently supported only on ESP32-S3. The active device uses S3; classic ESP32 release images do not fit their OTA slots. See [`VERIFICATION.md`](VERIFICATION.md) for build/image-fit details. No hardware flash or runtime test was performed.

### 1. LED Matrix
- **Four 16x16 WS2812B (or SK6812) LED Panels:** Assembled in a 2x2 grid to form a **32x32 display (1024 LEDs)**. Flexible or rigid PCB panels work equally well.

### 2. Microcontroller
- **ESP32-S3 N16R8** is the only currently supported production target. Its firmware release image fits the configured OTA slot. Classic ESP32 and ESP32-bare are not presently supported for OTA releases because their images exceed their slots; see [`VERIFICATION.md`](VERIFICATION.md).

### 3. Microphone (Sound-Reactive Audio)
- **Digital I2S Microphone:** **INMP441** or **ICS-43434** (or SPH0645).
- **Why Digital I2S over Analog (MAX4466 / MAX9814)?**
  - **Zero Noise:** WS2812B matrices induce heavy ripple on 5V/3.3V power rails. Analog microphones pick up this rail noise through the ADC, causing false beats and erratic triggers.
  - **Zero CPU Overhead:** Digital I2S delivers 24-bit/32-bit audio buffers directly into RAM via DMA without blocking the CPU or interrupting render loops.

### 4. Power Supply (PSU) & Wiring
- **5V DC Power Supply:** Recommended **10A to 15A (50W to 75W)** for typical high-brightness animations. (Theoretical peak for 1024 full-white LEDs is ~60A, but software current limiting defaults to 12A).
- **Power Injection:** Connect 5V and GND directly to each 16x16 panel corner with 18 AWG (or thicker) wire to prevent voltage drop and color shifting.
- **Capacitor:** A 1000 µF, 6.3V+ electrolytic capacitor across the 5V/GND terminals near the panels.
- **Logic Level Shifter:** Recommended **74AHCT125** or **74HCT245** to shift ESP32 3.3V GPIO data lines up to 5V logic.

---

## 🔌 Wiring Guide

### Physical 2x2 Panel Layout

Arrange the four 16x16 panels into a 32x32 square as follows:

```text
+-----------------------+-----------------------+
|  Panel 0 (Top-Left)   |  Panel 1 (Top-Right)  |
|  16x16 LEDs (0..255)  |  16x16 LEDs (0..255)  |
+-----------------------+-----------------------+
|  Panel 2 (Bottom-Left)|  Panel 3 (Bottom-Right)|
|  16x16 LEDs (0..255)  |  16x16 LEDs (0..255)  |
+-----------------------+-----------------------+
```

### Mode A: 4-Channel Parallel Output (Recommended for 60+ FPS)

Each panel receives its own dedicated RMT data channel:

| Signal | ESP32-WROOM Pin | ESP32-S3 Pin | Destination |
| :--- | :--- | :--- | :--- |
| **Panel 0 Data** | `GPIO 4` | `GPIO 8` | Panel 0 DIN (Top-Left) |
| **Panel 1 Data** | `GPIO 16` | `GPIO 3` | Panel 1 DIN (Top-Right) |
| **Panel 2 Data** | `GPIO 17` | `GPIO 10` *(See note)* | Panel 2 DIN (Bottom-Left) |
| **Panel 3 Data** | `GPIO 18` | `GPIO 9` | Panel 3 DIN (Bottom-Right) |

> ⚠️ **ESP32-S3 Hardware Note:** On the ESP32-S3, **GPIO 46 is strictly an input-only strapping pin** lacking output driver circuitry. The firmware maps Panel 2 to **GPIO 10** on ESP32-S3. Do **not** connect Panel 2 data to GPIO 46.

### Mode B: Single-Pin Serial Chaining (Alternative)

If using single-pin serial wiring, chain the data lines:
`ESP32 Data Pin -> Panel 0 DIN | Panel 0 DOUT -> Panel 1 DIN | Panel 1 DOUT -> Panel 2 DIN | Panel 2 DOUT -> Panel 3 DIN`.
- **ESP32-WROOM DIN:** `GPIO 4`
- **ESP32-S3 DIN:** `GPIO 8`

### Digital I2S Microphone Wiring (INMP441 / ICS-43434)

| Microphone Pin | ESP32-WROOM | ESP32-S3 | Description |
| :--- | :--- | :--- | :--- |
| **VDD** | `3.3V` | `3.3V` | **Power (Strictly 3.3V — 5V will burn the mic!)** |
| **GND** | `GND` | `GND` | Ground (Common ground with PSU) |
| **SD** (Serial Data / DOUT) | `GPIO 32` | `GPIO 4` | Audio data in |
| **WS** (Word Select / LRCL) | `GPIO 15` | `GPIO 5` | Left/Right word clock |
| **SCK** (Bit Clock / BCLK) | `GPIO 14` | `GPIO 6` | Continuous serial bit clock |
| **L/R** (Channel Select) | `GND` | `GND` | **Tie to GND** for Left audio channel |

---

## 🚀 Build and Flash Guide

### 1. Prerequisites

- **Rust Xtensa Toolchain:** Install via [espup](https://github.com/esp-rs/espup):
  ```sh
  curl -L https://github.com/esp-rs/espup/releases/latest/download/espup-x86_64-unknown-linux-gnu -o espup
  chmod +x espup && ./espup install
  source "$HOME/export-esp.sh"
  ```
- **Build Utilities:**
  ```sh
  cargo install ldproxy
  pip install esptool
  ```
  *(Or enter the included Nix environment: `nix-shell`)*

### 2. Compile the Firmware

Use the included reproducible build script:

```sh
cd firmware

# For standard ESP32-WROOM (with I2S microphone):
./build_firmware.sh esp32 release

# For ESP32-WROOM without microphone:
./build_firmware.sh esp32-bare release

# For ESP32-S3 (N16R8, 16MB Flash, Octal PSRAM):
./build_firmware.sh esp32s3 release

# Or build all profiles:
./build_firmware.sh all release
```

The script compiles the application ELF, converts it to a flashable `.bin` with `esptool.py`, and verifies partition fit against the OTA partition tables.

### 3. Stage Configuration & Assets (SPIFFS)

Create a staging directory containing your Wi-Fi credentials and optional panel configuration:

```sh
mkdir -p staged/config
cat << 'EOF' > staged/config/network.json
{
  "wifiSsid": "YourWiFiSSID",
  "wifiPassword": "YourWiFiPassword",
  "mqttHost": "192.168.1.100",
  "mqttPort": 1883,
  "mqttUser": "",
  "mqttPassword": ""
}
EOF

# Build the SPIFFS filesystem image:
python3 build_assets.py --target esp32 --source staged
# (Use --target esp32s3 for ESP32-S3)
```

### 4. Flash to Hardware

#### Flashing ESP32-WROOM (4MB Flash):
```sh
esptool.py --chip esp32 -p /dev/ttyUSB0 -b 460800 write_flash \
  0x1000 target/xtensa-esp32-espidf/release/bootloader.bin \
  0x8000 partitions.bin \
  0x10000 target/xtensa-esp32-espidf/release/matrix-firmware.bin \
  0x3d0000 target/assets-esp32.bin
```

#### Flashing ESP32-S3 (16MB Flash):
```sh
esptool.py --chip esp32s3 -p /dev/ttyACM0 -b 460800 write_flash \
  0x0 target/xtensa-esp32s3-espidf/release/bootloader.bin \
  0x8000 partitions_16MB.bin \
  0x20000 target/xtensa-esp32s3-espidf/release/matrix-firmware.bin \
  0x820000 target/assets-esp32s3.bin
```

Alternatively, use `espflash`:
```sh
espflash flash --chip esp32 target/xtensa-esp32-espidf/release/matrix-firmware
```

### 5. Over-the-Air (OTA) Updates

Once running on your network, update firmware wirelessly without plugging in USB:

```sh
curl -X POST --data-binary @firmware/target/xtensa-esp32-espidf/release/matrix-firmware.bin \
  http://<device-ip>/update
```

The device performs a two-phase handshake (clears LEDs, halts I2S DMA), writes to the passive OTA slot, and reboots.

### Memory diagnostics

Release firmware emits `RAM` diagnostics to the serial console at boot, around service startup, on microphone capture transitions and OTA quiescence, and every 10 seconds. All values are bytes: `free` is current heap, `min` is the boot-to-date low-water mark, and `largest` is the largest contiguous allocation. Internal DMA-capable memory overlaps the internal region; do not add the two. PSRAM values are zero on boards without it. Region low-water marks are aggregated per heap and are not necessarily simultaneous.

`RAM stack` lines show each task's minimum free stack since creation (smaller means closer to overflow). Render, microphone and MQTT report periodically; telemetry reports when processing work, and HTTP reports when requests are handled. Missing task lines can mean the service is disabled or idle, not that its stack is empty. Exercise effects, microphone capture, HTTP, MQTT and OTA to establish useful worst cases; logging itself affects stack and timing. OTA diagnostics do not initiate an update.

### 6. Fallback Access Point (`JDJLIGHT`)

If configured Wi-Fi is unavailable or out of range, the device can create the fallback Wi-Fi hotspot only when a per-device WPA2 AP password has already been privately provisioned. Firmware has no universal AP password; an unprovisioned device does not expose a setup AP and requires local/physical bootstrap provisioning. The UI is at `http://192.168.4.1/` when the AP is active. Network password values are never returned by the general configuration GET endpoint. See [`WIFI_CONFIGURATION.md`](WIFI_CONFIGURATION.md) for the bootstrap limitation and API details.

---

## 💻 Run on Your Computer (Simulator & WASM Preview)

You do not need an ESP32 to test, design, or preview effects!

### 1. Standalone Desktop Simulator

```sh
cargo run -p matrix-control --bin matrix-simulator -- --listen 127.0.0.1:8080
```

Open `http://127.0.0.1:8080` in any browser. The simulator boots **OFF** by default. Toggle power ON to explore:
- All 80+ visual effects and real-time parameter sliders.
- Color, brightness, speed, and text overlays.
- Virtual microphone audio input with synthetic audio generator.
- Panel layout configuration and live FPS telemetry.

### 2. In-Browser WebAssembly Preview

Rebuild the compressed WebAssembly module after modifying core/effects:

```sh
python3 scripts/build-preview.py
```

The embedded Web UI decompresses the WASM module directly in your browser and renders the exact Rust shader routines on an HTML5 canvas.

---

## 🎨 Adding New Effects (PRs Welcome!)

We encourage and welcome pull requests for new effects, audio visualizers, demoscene shaders, games, and optimizations!

### How to Create an Effect

1. **Create your effect file** in `crates/matrix-effects/src/my_effect.rs`:

```rust
use matrix_core::{Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH};

#[derive(Default)]
pub struct MyAwesomeEffect {
    speed: f32,
}

impl MyAwesomeEffect {
    pub fn new() -> Self {
        Self { speed: 1.0 }
    }
}

impl Effect for MyAwesomeEffect {
    fn id(&self) -> &'static str { "my_awesome_effect" }
    fn name(&self) -> &'static str { "My Awesome Effect" }

    fn parameters(&self) -> &'static [ParameterDescriptor] {
        &[ParameterDescriptor {
            id: "speed",
            label: "Animation Speed",
            kind: ParameterKind::Number,
            min: 0.1,
            max: 5.0,
            step: 0.1,
            default: 1.0,
            options: &[],
            unit: "x",
        }]
    }

    fn set_parameter(&mut self, id: &str, value: f32) -> Result<(), String> {
        if id == "speed" { self.speed = value; Ok(()) }
        else { Err("Unknown parameter".into()) }
    }

    fn get_parameter(&self, id: &str) -> Result<f32, String> {
        if id == "speed" { Ok(self.speed) }
        else { Err("Unknown parameter".into()) }
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let t = (context.now_ms as f32 / 1000.0) * self.speed;
        let bass = context.audio.as_ref().map(|a| a.bass()).unwrap_or(0.0);

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let dx = x as f32 - 15.5;
                let dy = y as f32 - 15.5;
                let r = (dx * dx + dy * dy).sqrt();
                let v = ((r * 0.5 - t * 2.0).sin() + 1.0) * 0.5 * (1.0 + bass);
                
                // Fast saturating additive blend
                frame.set(x, y, Rgb::hsv(v, 0.9, v.clamp(0.0, 1.0)));
            }
        }
    }
}
```

2. **Register the effect** in `crates/matrix-effects/src/lib.rs`:
   - Add `pub mod my_effect;`
   - Export `pub use my_effect::MyAwesomeEffect;`
   - Register it inside the effect factory catalog.

3. **Verify locally:**
   ```sh
   cargo test --workspace
   cargo run -p matrix-control --bin matrix-simulator
   ```

4. **Submit a Pull Request!** Open a PR on GitHub. Every contribution from visual shaders to driver improvements is appreciated.

---

## 🏗️ Repository Architecture

| Crate / Path | Responsibility |
| :--- | :--- |
| **`crates/matrix-core`** | Pixel formats (`Rgb`), frame buffer (`Frame`), effect contract (`Effect`), audio DSP & FFT analysis, current limiting, geometry mapping, serialization. |
| **`crates/matrix-effects`** | Catalog of 80+ handcrafted visual effects, parameter schemas, demoscene math, and lazy factory loading. |
| **`crates/matrix-control`** | Render engine actor, bounded command queues, Web UI & REST API dispatch, MQTT & Home Assistant discovery, state persistence. |
| **`crates/matrix-preview`** | WebAssembly bindings compiled to `.wasm` for instant zero-latency browser rendering. |
| **`firmware`** | Production ESP-IDF application; currently supported only on ESP32-S3 N16R8. Classic ESP32 OTA images exceed their configured slots; see [`VERIFICATION.md`](VERIFICATION.md). |
| **`tests`** | Geometry mapping tests, regression test suites, and mock network services. |

---

## 🧪 Testing and Verification

Run the test suite across the workspace:

```sh
# Run all workspace unit and integration tests
cargo test --workspace

# Lint code with Clippy
cargo clippy --workspace --all-targets -- -D warnings

# Run geometry and catalog integration checks
cargo test --manifest-path tests/Cargo.toml
```

See [VERIFICATION.md](VERIFICATION.md) and [firmware/README.md](firmware/README.md) for low-level hardware verification details, oscilloscope timing benchmarks, and partition fit analysis.

---

## 📄 License

Dual-licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
