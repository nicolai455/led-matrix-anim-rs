use matrix_control::LedOutput;
#[cfg(target_os = "espidf")]
use matrix_core::geometry::calculate_max_brightness;
use matrix_core::geometry::{PanelConfig, DEFAULT_MAX_MILLIAMPS, NUM_PANELS};
use matrix_core::{Frame, PIXELS};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub const PANEL_PIXELS: usize = PIXELS / NUM_PANELS; // 256
pub const BYTES_PER_PANEL: usize = PANEL_PIXELS * 3; // 768 bytes
pub const TOTAL_BYTES: usize = PIXELS * 3; // 3072 bytes

/// Configuration for LED output power and geometry.
#[derive(Clone, Debug)]
pub struct OutputSettings {
    pub parallel_output: bool,
    pub panel_pins: [u8; NUM_PANELS],
    pub data_pin: u8,
    pub max_milliamps: u32,
    pub supply_volts: u8,
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self {
            parallel_output: false,
            panel_pins: [4, 16, 17, 18],
            data_pin: 4,
            max_milliamps: DEFAULT_MAX_MILLIAMPS,
            supply_volts: 5,
        }
    }
}

impl OutputSettings {
    pub fn from_panel_config(cfg: &PanelConfig) -> Self {
        Self {
            parallel_output: cfg.parallel_output,
            panel_pins: cfg.panel_pins,
            data_pin: cfg.data_pin,
            max_milliamps: cfg.led_max_milliamps,
            supply_volts: 5,
        }
    }
}

#[derive(Clone, Default)]
pub struct OutputQuiesce {
    requested: Arc<AtomicBool>,
    confirmed: Arc<AtomicBool>,
}

impl OutputQuiesce {
    pub fn pause(&self, timeout: std::time::Duration) -> Result<(), String> {
        self.requested.store(true, Ordering::Release);
        let deadline = std::time::Instant::now() + timeout;
        while !self.confirmed.load(Ordering::Acquire) {
            if std::time::Instant::now() >= deadline {
                self.resume();
                return Err("LED output did not confirm physical blackout before OTA".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        Ok(())
    }

    pub fn resume(&self) {
        self.confirmed.store(false, Ordering::Release);
        self.requested.store(false, Ordering::Release);
    }
}

pub struct RmtLedOutput {
    settings: OutputSettings,
    panel_config: PanelConfig,
    quiesced: OutputQuiesce,
    // One heap buffer serves either four parallel panels or a single chain.
    buffer: Box<[u8]>,
    #[cfg(target_os = "espidf")]
    hardware: Option<HardwareRmt>,
}

#[cfg(target_os = "espidf")]
struct HardwareRmt {
    channels: Vec<esp_idf_sys::rmt_channel_handle_t>,
    encoders: Vec<esp_idf_sys::rmt_encoder_handle_t>,
}

#[cfg(target_os = "espidf")]
// SAFETY: this object uniquely owns its channel/encoder handles and is moved to
// one render task, never shared. ESP-IDF permits task-context RMT operations;
// transmit_current_buffers drains every started transfer before buffer reuse.
unsafe impl Send for HardwareRmt {}

#[cfg(target_os = "espidf")]
impl Drop for HardwareRmt {
    fn drop(&mut self) {
        unsafe {
            for &chan in &self.channels {
                if !chan.is_null() {
                    esp_idf_sys::rmt_disable(chan);
                    esp_idf_sys::rmt_del_channel(chan);
                }
            }
            for &enc in &self.encoders {
                if !enc.is_null() {
                    esp_idf_sys::rmt_del_encoder(enc);
                }
            }
        }
    }
}

impl RmtLedOutput {
    pub fn new(settings: OutputSettings, panel_config: PanelConfig) -> Self {
        let quiesced = OutputQuiesce::default();

        #[cfg(target_os = "espidf")]
        let hardware = Self::init_hardware(&settings);

        Self {
            settings,
            panel_config,
            quiesced,
            buffer: vec![0u8; TOTAL_BYTES].into_boxed_slice(),
            #[cfg(target_os = "espidf")]
            hardware,
        }
    }

    pub fn get_quiesce_handle(&self) -> OutputQuiesce {
        self.quiesced.clone()
    }

    /// Pin/mode changes require a restart; preserve the active driver wiring.
    pub fn update_config(&mut self, cfg: &PanelConfig) -> Result<(), String> {
        cfg.validate()?;
        if self.settings.parallel_output != cfg.parallel_output
            || self.settings.data_pin != cfg.data_pin
            || self.settings.panel_pins != cfg.panel_pins
        {
            return Err("Output pin or wiring-mode changes require a restart".into());
        }
        self.panel_config = cfg.clone();
        self.settings.max_milliamps = cfg.led_max_milliamps;
        Ok(())
    }

    #[cfg(target_os = "espidf")]
    fn init_hardware(settings: &OutputSettings) -> Option<HardwareRmt> {
        let is_s3 = cfg!(any(feature = "esp32s3", esp32s3));
        unsafe {
            let mut channels = Vec::new();
            let mut encoders = Vec::new();

            let pins_to_init: Vec<u8> = if settings.parallel_output {
                settings.panel_pins.to_vec()
            } else {
                vec![settings.data_pin]
            };

            let mut unique = std::collections::BTreeSet::new();
            if pins_to_init.iter().any(|pin| !unique.insert(*pin)) {
                log::error!("Duplicate RMT output pins");
                return None;
            }
            for &pin in &pins_to_init {
                // Ensure output GPIO bounds on the target architecture
                if !crate::pins::is_valid_output_pin(pin, is_s3) {
                    log::error!(
                        "GPIO {} is invalid for RMT output on target; aborting RMT init",
                        pin
                    );
                    for &c in &channels {
                        esp_idf_sys::rmt_disable(c);
                        esp_idf_sys::rmt_del_channel(c);
                    }
                    for &e in &encoders {
                        esp_idf_sys::rmt_del_encoder(e);
                    }
                    return None;
                }

                // Initialize RMT TX channel
                let mut tx_chan_config: esp_idf_sys::rmt_tx_channel_config_t = std::mem::zeroed();
                tx_chan_config.gpio_num = pin as i32;
                tx_chan_config.clk_src = esp_idf_sys::soc_periph_rmt_clk_src_t_RMT_CLK_SRC_DEFAULT;
                tx_chan_config.resolution_hz = 10_000_000; // 10MHz, 1 tick = 100ns
                tx_chan_config.mem_block_symbols = if is_s3 { 48 } else { 64 };
                tx_chan_config.trans_queue_depth = 4;

                let mut chan: esp_idf_sys::rmt_channel_handle_t = std::ptr::null_mut();
                let err = esp_idf_sys::rmt_new_tx_channel(&tx_chan_config, &mut chan);
                if err != esp_idf_sys::ESP_OK {
                    log::error!("Failed to create RMT channel for pin {}: err {}", pin, err);
                    for &c in &channels {
                        esp_idf_sys::rmt_disable(c);
                        esp_idf_sys::rmt_del_channel(c);
                    }
                    for &e in &encoders {
                        esp_idf_sys::rmt_del_encoder(e);
                    }
                    return None;
                }

                // Create bytes encoder for WS2812B timing at 10 MHz:
                // Bit 0: High 350ns (4 ticks), Low 800ns (8 ticks)
                // Bit 1: High 700ns (7 ticks), Low 600ns (6 ticks)
                let mut bytes_enc_cfg: esp_idf_sys::rmt_bytes_encoder_config_t = std::mem::zeroed();
                bytes_enc_cfg.bit0.__bindgen_anon_1.set_duration0(4);
                bytes_enc_cfg.bit0.__bindgen_anon_1.set_level0(1);
                bytes_enc_cfg.bit0.__bindgen_anon_1.set_duration1(8);
                bytes_enc_cfg.bit0.__bindgen_anon_1.set_level1(0);

                bytes_enc_cfg.bit1.__bindgen_anon_1.set_duration0(7);
                bytes_enc_cfg.bit1.__bindgen_anon_1.set_level0(1);
                bytes_enc_cfg.bit1.__bindgen_anon_1.set_duration1(6);
                bytes_enc_cfg.bit1.__bindgen_anon_1.set_level1(0);

                bytes_enc_cfg.flags.set_msb_first(1);

                let mut enc: esp_idf_sys::rmt_encoder_handle_t = std::ptr::null_mut();
                let err = esp_idf_sys::rmt_new_bytes_encoder(&bytes_enc_cfg, &mut enc);
                if err != esp_idf_sys::ESP_OK {
                    log::error!("Failed to create RMT bytes encoder: err {}", err);
                    esp_idf_sys::rmt_del_channel(chan);
                    for &c in &channels {
                        esp_idf_sys::rmt_disable(c);
                        esp_idf_sys::rmt_del_channel(c);
                    }
                    for &e in &encoders {
                        esp_idf_sys::rmt_del_encoder(e);
                    }
                    return None;
                }

                let error = esp_idf_sys::rmt_enable(chan);
                if error != esp_idf_sys::ESP_OK {
                    esp_idf_sys::rmt_del_encoder(enc);
                    esp_idf_sys::rmt_del_channel(chan);
                    drop(HardwareRmt { channels, encoders });
                    log::error!("Failed to enable RMT channel: {error}");
                    return None;
                }
                channels.push(chan);
                encoders.push(enc);
            }

            Some(HardwareRmt { channels, encoders })
        }
    }

    /// Blanks out the LEDs and stops output (used for dark boot and OTA).
    pub fn blank(&mut self) -> Result<(), String> {
        self.buffer.fill(0);
        self.transmit_current_buffers()
    }

    fn transmit_current_buffers(&mut self) -> Result<(), String> {
        #[cfg(not(target_os = "espidf"))]
        return Err("RMT hardware output not available on host target".into());

        #[cfg(target_os = "espidf")]
        {
            let hw = self.hardware.as_ref().ok_or("RMT hardware unavailable")?;
            let count = if self.settings.parallel_output {
                NUM_PANELS
            } else {
                1
            };
            if hw.channels.len() != count || hw.encoders.len() != count {
                return Err("RMT channel/encoder count does not match output configuration".into());
            }
            let mut failure = None;
            let mut started = 0;
            unsafe {
                let config: esp_idf_sys::rmt_transmit_config_t = std::mem::zeroed();
                for i in 0..count {
                    let bytes: &[u8] = if self.settings.parallel_output {
                        &self.buffer[i * BYTES_PER_PANEL..(i + 1) * BYTES_PER_PANEL]
                    } else {
                        &self.buffer
                    };
                    let error = esp_idf_sys::rmt_transmit(
                        hw.channels[i],
                        hw.encoders[i],
                        bytes.as_ptr().cast(),
                        bytes.len(),
                        &config,
                    );
                    if error != esp_idf_sys::ESP_OK {
                        failure = Some(format!("RMT channel {i} transmit failed: {error}"));
                        break;
                    }
                    started += 1;
                }
                // Always drain every started transaction, even after another
                // channel fails: the driver still borrows our frame buffers.
                for i in 0..started {
                    let error = esp_idf_sys::rmt_tx_wait_all_done(hw.channels[i], -1);
                    if error != esp_idf_sys::ESP_OK {
                        failure.get_or_insert_with(|| {
                            format!("RMT channel {i} completion failed: {error}")
                        });
                    }
                }
                esp_idf_sys::esp_rom_delay_us(300);
            }
            match failure {
                Some(error) => Err(error),
                None => Ok(()),
            }
        }
    }
}

impl LedOutput for RmtLedOutput {
    fn show(&mut self, frame: &Frame, brightness: u8) -> Result<(), String> {
        if self.quiesced.requested.load(Ordering::Acquire) {
            if !self.quiesced.confirmed.load(Ordering::Acquire) {
                self.blank()?;
                self.quiesced.confirmed.store(true, Ordering::Release);
            }
            // Prevent a hot loop while output is intentionally suspended.
            std::thread::sleep(std::time::Duration::from_millis(1));
            return Ok(());
        }

        #[cfg(not(target_os = "espidf"))]
        {
            let _ = (frame, brightness);
            // Host target has no hardware RMT; return an error instead of claiming hardware success
            Err("RMT hardware output not available on host target".to_string())
        }

        #[cfg(target_os = "espidf")]
        {
            // Apply power budget and current limit
            let effective_brightness =
                calculate_max_brightness(frame, brightness, self.settings.max_milliamps);

            let scale = effective_brightness as f32 / 255.0;

            if self.settings.parallel_output {
                // 4-panel parallel output:
                // Map 32x32 frame pixels to panels 0..3 using panel_config
                for y in 0..32 {
                    for x in 0..32 {
                        let logical_pixel = frame.get(x, y);
                        let scaled_r = (logical_pixel.r as f32 * scale + 0.5) as u8;
                        let scaled_g = (logical_pixel.g as f32 * scale + 0.5) as u8;
                        let scaled_b = (logical_pixel.b as f32 * scale + 0.5) as u8;

                        // Physical mapping using panel_config
                        let led_idx = self.panel_config.get_led_index(x, y);
                        if led_idx < PIXELS {
                            let panel_idx = led_idx / PANEL_PIXELS;
                            let panel_led_offset = led_idx % PANEL_PIXELS;
                            let byte_offset = panel_led_offset * 3;

                            // WS2812B GRB order
                            self.buffer[panel_idx * BYTES_PER_PANEL + byte_offset] = scaled_g;
                            self.buffer[panel_idx * BYTES_PER_PANEL + byte_offset + 1] = scaled_r;
                            self.buffer[panel_idx * BYTES_PER_PANEL + byte_offset + 2] = scaled_b;
                        }
                    }
                }
            } else {
                // Single chain output:
                for y in 0..32 {
                    for x in 0..32 {
                        let logical_pixel = frame.get(x, y);
                        let scaled_r = (logical_pixel.r as f32 * scale + 0.5) as u8;
                        let scaled_g = (logical_pixel.g as f32 * scale + 0.5) as u8;
                        let scaled_b = (logical_pixel.b as f32 * scale + 0.5) as u8;

                        let led_idx = self.panel_config.get_led_index(x, y);
                        if led_idx < PIXELS {
                            let byte_offset = led_idx * 3;
                            self.buffer[byte_offset] = scaled_g;
                            self.buffer[byte_offset + 1] = scaled_r;
                            self.buffer[byte_offset + 2] = scaled_b;
                        }
                    }
                }
            }

            self.transmit_current_buffers()
        }
    }

    fn set_power_limit(&mut self, max_milliamps: u32) -> Result<(), String> {
        if !(matrix_core::MIN_MILLIAMPS_LIMIT..=matrix_core::MAX_MILLIAMPS_LIMIT)
            .contains(&max_milliamps)
        {
            return Err("Power limit outside supported range".into());
        }
        self.settings.max_milliamps = max_milliamps;
        Ok(())
    }

    fn update_config(&mut self, config: &PanelConfig) -> Result<(), String> {
        RmtLedOutput::update_config(self, config)
    }
}
