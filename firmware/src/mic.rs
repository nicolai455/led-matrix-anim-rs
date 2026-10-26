use matrix_control::AudioInput;
#[cfg(target_os = "espidf")]
use matrix_core::audio::MicDsp;
use matrix_core::audio::MicSettings;
use matrix_core::AudioSnapshot;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

pub const MIC_SAMPLE_RATE: u32 = 22050;
pub const MIC_BLOCK_SAMPLES: usize = 512;
pub const MIC_STALE_TIMEOUT_MS: u64 = 250;

struct CaptureShared {
    settings: RwLock<MicSettings>,
    latest_snapshot: RwLock<AudioSnapshot>,
    latest_metrics: RwLock<Option<(f32, f32)>>,
    last_block_ms: RwLock<Option<std::time::Instant>>,
    quiesce_requested: AtomicBool,
    quiesced_confirmed: AtomicBool,
    shutdown: AtomicBool,
    reconfigure_channel: AtomicBool,
}

/// I2S Microphone input adapter implementing `matrix_control::AudioInput`.
pub struct I2sMicAudioInput {
    available: bool,
    shared: Arc<CaptureShared>,
    #[allow(dead_code)]
    worker_handle: Option<std::thread::JoinHandle<()>>,
}

#[derive(Clone)]
pub struct MicQuiesce {
    shared: Arc<CaptureShared>,
    available: bool,
}
impl MicQuiesce {
    pub fn pause(&self) -> Result<(), String> {
        if !self.available {
            return Ok(());
        }
        self.shared.quiesce_requested.store(true, Ordering::Release);
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while !self.shared.quiesced_confirmed.load(Ordering::Acquire) {
            if std::time::Instant::now() >= deadline {
                self.resume();
                return Err("microphone did not quiesce".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }
    pub fn resume(&self) {
        self.shared
            .quiesced_confirmed
            .store(false, Ordering::Release);
        self.shared
            .quiesce_requested
            .store(false, Ordering::Release);
    }
}

impl I2sMicAudioInput {
    pub fn quiesce_handle(&self) -> MicQuiesce {
        MicQuiesce {
            shared: self.shared.clone(),
            available: self.available,
        }
    }
    pub fn new(available: bool, initial_settings: MicSettings) -> Self {
        let shared = Arc::new(CaptureShared {
            settings: RwLock::new(initial_settings),
            latest_snapshot: RwLock::new(AudioSnapshot::default()),
            latest_metrics: RwLock::new(None),
            last_block_ms: RwLock::new(None),
            quiesce_requested: AtomicBool::new(false),
            quiesced_confirmed: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
            reconfigure_channel: AtomicBool::new(false),
        });

        let worker_handle = if available {
            let shared_worker = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("i2s_mic".to_string())
                .stack_size(16384)
                .spawn(move || {
                    Self::capture_worker_loop(shared_worker);
                })
                .ok()
        } else {
            None
        };

        Self {
            available,
            shared,
            worker_handle,
        }
    }

    pub fn is_available(&self) -> bool {
        self.available
    }

    pub fn pause(&self) {
        if !self.available {
            return;
        }
        self.shared.quiesce_requested.store(true, Ordering::SeqCst);
        // Handshake: wait up to 300ms for capture worker to confirm I2S DMA is disabled
        for _ in 0..30 {
            if self.shared.quiesced_confirmed.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn resume(&self) {
        if !self.available {
            return;
        }
        self.shared
            .quiesced_confirmed
            .store(false, Ordering::SeqCst);
        self.shared.quiesce_requested.store(false, Ordering::SeqCst);
    }

    #[cfg(target_os = "espidf")]
    fn init_hardware_channel(right_channel: bool) -> Option<esp_idf_sys::i2s_chan_handle_t> {
        unsafe {
            let mut chan_cfg: esp_idf_sys::i2s_chan_config_t = std::mem::zeroed();
            chan_cfg.id = esp_idf_sys::i2s_port_t_I2S_NUM_0;
            chan_cfg.role = esp_idf_sys::i2s_role_t_I2S_ROLE_MASTER;
            chan_cfg.dma_desc_num = 2;
            chan_cfg.dma_frame_num = 128;
            chan_cfg.auto_clear = false;

            let mut rx_handle: esp_idf_sys::i2s_chan_handle_t = std::ptr::null_mut();
            let err = esp_idf_sys::i2s_new_channel(&chan_cfg, std::ptr::null_mut(), &mut rx_handle);
            if err != esp_idf_sys::ESP_OK || rx_handle.is_null() {
                log::error!("Failed to allocate I2S channel: {err}");
                return None;
            }

            let mut std_cfg: esp_idf_sys::i2s_std_config_t = std::mem::zeroed();
            std_cfg.clk_cfg.sample_rate_hz = MIC_SAMPLE_RATE;
            std_cfg.clk_cfg.clk_src = esp_idf_sys::soc_periph_i2s_clk_src_t_I2S_CLK_SRC_DEFAULT;
            std_cfg.clk_cfg.mclk_multiple = esp_idf_sys::i2s_mclk_multiple_t_I2S_MCLK_MULTIPLE_256;

            std_cfg.slot_cfg.data_bit_width =
                esp_idf_sys::i2s_data_bit_width_t_I2S_DATA_BIT_WIDTH_32BIT;
            std_cfg.slot_cfg.slot_bit_width =
                esp_idf_sys::i2s_slot_bit_width_t_I2S_SLOT_BIT_WIDTH_AUTO;
            std_cfg.slot_cfg.slot_mode = esp_idf_sys::i2s_slot_mode_t_I2S_SLOT_MODE_MONO;
            std_cfg.slot_cfg.slot_mask = if right_channel { 1 << 1 } else { 1 << 0 };
            std_cfg.slot_cfg.ws_width = 32;
            std_cfg.slot_cfg.ws_pol = false;
            std_cfg.slot_cfg.bit_shift = true;

            // Pin assignments from BoardPins.h:
            // SD = 32, WS = 15, SCK = 14
            std_cfg.gpio_cfg.mclk = esp_idf_sys::gpio_num_t_GPIO_NUM_NC;
            std_cfg.gpio_cfg.bclk = esp_idf_sys::gpio_num_t_GPIO_NUM_14;
            std_cfg.gpio_cfg.ws = esp_idf_sys::gpio_num_t_GPIO_NUM_15;
            std_cfg.gpio_cfg.dout = esp_idf_sys::gpio_num_t_GPIO_NUM_NC;
            std_cfg.gpio_cfg.din = esp_idf_sys::gpio_num_t_GPIO_NUM_32;

            let init_err = esp_idf_sys::i2s_channel_init_std_mode(rx_handle, &std_cfg);
            if init_err != esp_idf_sys::ESP_OK {
                log::error!("Failed to initialize I2S std mode: {init_err}");
                esp_idf_sys::i2s_del_channel(rx_handle);
                return None;
            }

            esp_idf_sys::i2s_channel_enable(rx_handle);
            Some(rx_handle)
        }
    }

    fn capture_worker_loop(shared: Arc<CaptureShared>) {
        #[cfg(target_os = "espidf")]
        let mut dsp = MicDsp::new();
        #[cfg(target_os = "espidf")]
        let start_time = std::time::Instant::now();

        #[cfg(target_os = "espidf")]
        let mut right_channel_active = shared
            .settings
            .read()
            .map(|s| s.right_channel)
            .unwrap_or(false);

        #[cfg(target_os = "espidf")]
        let mut rx_chan = Self::init_hardware_channel(right_channel_active);

        #[cfg(target_os = "espidf")]
        let mut raw_buf = [0i32; MIC_BLOCK_SAMPLES];
        #[cfg(target_os = "espidf")]
        let mut samples = [0.0f32; MIC_BLOCK_SAMPLES];

        while !shared.shutdown.load(Ordering::Relaxed) {
            // Check quiesce request (e.g. for OTA)
            if shared.quiesce_requested.load(Ordering::SeqCst) {
                #[cfg(target_os = "espidf")]
                if let Some(chan) = rx_chan {
                    unsafe {
                        let _ = esp_idf_sys::i2s_channel_disable(chan);
                    }
                }
                shared.quiesced_confirmed.store(true, Ordering::SeqCst);

                while shared.quiesce_requested.load(Ordering::SeqCst)
                    && !shared.shutdown.load(Ordering::Relaxed)
                {
                    std::thread::sleep(Duration::from_millis(20));
                }

                #[cfg(target_os = "espidf")]
                if let Some(chan) = rx_chan {
                    unsafe {
                        let _ = esp_idf_sys::i2s_channel_enable(chan);
                    }
                }
                shared.quiesced_confirmed.store(false, Ordering::SeqCst);
                continue;
            }

            // Check dynamic right_channel reconfiguration
            #[cfg(target_os = "espidf")]
            if shared.reconfigure_channel.swap(false, Ordering::SeqCst) {
                let current_right = shared
                    .settings
                    .read()
                    .map(|s| s.right_channel)
                    .unwrap_or(false);
                if current_right != right_channel_active {
                    if let Some(chan) = rx_chan.take() {
                        unsafe {
                            let _ = esp_idf_sys::i2s_channel_disable(chan);
                            let _ = esp_idf_sys::i2s_del_channel(chan);
                        }
                    }
                    rx_chan = Self::init_hardware_channel(current_right);
                    right_channel_active = current_right;
                }
            }

            #[cfg(target_os = "espidf")]
            {
                if let Some(chan) = rx_chan {
                    let mut bytes_read: usize = 0;
                    let err = unsafe {
                        esp_idf_sys::i2s_channel_read(
                            chan,
                            raw_buf.as_mut_ptr() as *mut std::ffi::c_void,
                            raw_buf.len() * 4,
                            &mut bytes_read,
                            50, // 50ms read timeout
                        )
                    };

                    if err == esp_idf_sys::ESP_OK && bytes_read >= raw_buf.len() * 4 {
                        // High 24 bits hold INMP441 sample data
                        for (i, &raw) in raw_buf.iter().enumerate() {
                            samples[i] = (raw >> 8) as f32 / 8388608.0;
                        }

                        let cur_settings = shared
                            .settings
                            .read()
                            .map(|s| s.clone())
                            .unwrap_or_default();
                        let now_ms = start_time.elapsed().as_millis() as u64;

                        let snap = dsp.process(&samples, &cur_settings, now_ms);
                        let metrics = (dsp.raw_dbfs, dsp.applied_gain_db);

                        if let Ok(mut lock) = shared.latest_snapshot.write() {
                            *lock = snap;
                        }
                        if let Ok(mut lock) = shared.latest_metrics.write() {
                            *lock = Some(metrics);
                        }
                        if let Ok(mut lock) = shared.last_block_ms.write() {
                            *lock = Some(std::time::Instant::now());
                        }
                    } else {
                        // Hardware read failure: DO NOT inject fake silence.
                        // Allow staleness logic to mark audio unavailable.
                        std::thread::sleep(Duration::from_millis(10));
                    }
                } else {
                    std::thread::sleep(Duration::from_millis(50));
                }
            }

            #[cfg(not(target_os = "espidf"))]
            {
                // In simulated host mode, yield without fabricating false hardware
                std::thread::sleep(Duration::from_millis(20));
            }
        }

        #[cfg(target_os = "espidf")]
        if let Some(chan) = rx_chan {
            unsafe {
                let _ = esp_idf_sys::i2s_channel_disable(chan);
                let _ = esp_idf_sys::i2s_del_channel(chan);
            }
        }
    }
}

impl Drop for I2sMicAudioInput {
    fn drop(&mut self) {
        self.shared.shutdown.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker_handle.take() {
            let _ = worker.join();
        }
    }
}

impl AudioInput for I2sMicAudioInput {
    fn snapshot(&mut self, _now_ms: u64) -> AudioSnapshot {
        if !self.available || self.shared.quiesce_requested.load(Ordering::Relaxed) {
            return AudioSnapshot::default();
        }
        let age = self
            .shared
            .last_block_ms
            .read()
            .ok()
            .and_then(|at| *at)
            .map(|at| at.elapsed().as_millis() as u64)
            .unwrap_or(u64::MAX);
        let mut snapshot = self
            .shared
            .latest_snapshot
            .read()
            .map(|s| s.clone())
            .unwrap_or_default();
        snapshot.apply_staleness(age);
        snapshot
    }

    fn live_metrics(&self) -> Option<(f32, f32)> {
        self.shared.latest_metrics.read().ok().and_then(|m| *m)
    }

    fn configure(&mut self, settings: &MicSettings) {
        let changed_channel = {
            if let Ok(mut lock) = self.shared.settings.write() {
                let changed = lock.right_channel != settings.right_channel;
                *lock = settings.clone();
                changed
            } else {
                false
            }
        };

        if changed_channel {
            self.shared
                .reconfigure_channel
                .store(true, Ordering::SeqCst);
        }
    }
}
