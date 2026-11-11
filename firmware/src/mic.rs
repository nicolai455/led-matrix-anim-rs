use matrix_control::AudioInput;
use matrix_core::AudioSnapshot;
#[cfg(target_os = "espidf")]
use matrix_core::audio::MicDsp;
use matrix_core::audio::MicSettings;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

pub const MIC_SAMPLE_RATE: u32 = 22050;
pub const MIC_BLOCK_SAMPLES: usize = 512;
pub const MIC_STALE_TIMEOUT_MS: u64 = 250;

// The request, acknowledgement and channel transition share one lock: a resume
// cannot race a worker publishing an acknowledgement for an older request.
#[derive(Default)]
struct QuiesceState {
    requested: bool,
    confirmed: bool,
    disabled: bool,
    worker_started: bool,
    services_initialized: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum QuiesceStep {
    Paused,
    Running,
    EnableFailed,
}

impl QuiesceState {
    fn resume(&mut self) {
        self.requested = false;
        self.confirmed = false;
    }

    fn step(
        &mut self,
        disable: impl FnOnce() -> bool,
        enable: impl FnOnce() -> bool,
    ) -> QuiesceStep {
        if self.requested {
            if !self.disabled && disable() {
                self.disabled = true;
            }
            self.confirmed = self.disabled;
            QuiesceStep::Paused
        } else if self.disabled {
            if !enable() {
                return QuiesceStep::EnableFailed;
            }
            self.disabled = false;
            QuiesceStep::Running
        } else {
            QuiesceStep::Running
        }
    }
}

struct CaptureShared {
    settings: RwLock<MicSettings>,
    latest_snapshot: RwLock<AudioSnapshot>,
    latest_metrics: RwLock<Option<(f32, f32)>>,
    last_block_ms: RwLock<Option<std::time::Instant>>,
    quiesce: Mutex<QuiesceState>,
    shutdown: AtomicBool,
    reconfigure_channel: AtomicBool,
    /// Set while the engine consumes mic audio. When false, the worker
    /// releases the I2S channel so no DMA/interrupt load remains.
    capture_enabled: AtomicBool,
}

/// I2S Microphone input adapter implementing `matrix_control::AudioInput`.
pub struct I2sMicAudioInput {
    available: bool,
    shared: Arc<CaptureShared>,
    #[allow(dead_code)]
    worker_handle: Option<std::thread::JoinHandle<()>>,
    last_spawn_attempt: Option<std::time::Instant>,
}

#[derive(Clone)]
pub struct MicQuiesce {
    shared: Arc<CaptureShared>,
    available: bool,
}
impl MicQuiesce {
    /// Allow lazy capture startup once network services have initialized.
    pub fn network_services_initialized(&self) {
        self.shared.quiesce.lock().unwrap().services_initialized = true;
    }

    pub fn pause(&self) -> Result<(), String> {
        if !self.available {
            return Ok(());
        }
        {
            let mut state = self.shared.quiesce.lock().unwrap();
            state.requested = true;
            if !state.worker_started {
                state.confirmed = true;
            }
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while !self.shared.quiesce.lock().unwrap().confirmed {
            if std::time::Instant::now() >= deadline {
                self.resume();
                return Err("microphone did not quiesce".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }
    pub fn resume(&self) {
        self.shared.quiesce.lock().unwrap().resume();
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
            quiesce: Mutex::new(QuiesceState::default()),
            shutdown: AtomicBool::new(false),
            reconfigure_channel: AtomicBool::new(false),
            capture_enabled: AtomicBool::new(false),
        });

        Self {
            available,
            shared,
            worker_handle: None,
            last_spawn_attempt: None,
        }
    }

    fn ensure_worker_started(&mut self) {
        self.ensure_worker_started_with(|shared| {
            std::thread::Builder::new()
                .name("i2s_mic".into())
                .stack_size(16384)
                .spawn(move || Self::capture_worker_loop(shared))
        });
    }

    fn ensure_worker_started_with(
        &mut self,
        spawn: impl FnOnce(Arc<CaptureShared>) -> std::io::Result<std::thread::JoinHandle<()>>,
    ) {
        if !self.available
            || self.worker_handle.is_some()
            || !self.shared.capture_enabled.load(Ordering::SeqCst)
        {
            return;
        }
        // Serialize starting with pause's acknowledgement.
        let mut state = self.shared.quiesce.lock().unwrap();
        if !state.services_initialized
            || state.requested
            || self
                .last_spawn_attempt
                .is_some_and(|at| at.elapsed() < Duration::from_secs(1))
        {
            return;
        }
        self.last_spawn_attempt = Some(std::time::Instant::now());
        match spawn(self.shared.clone()) {
            Ok(handle) => {
                state.worker_started = true;
                self.worker_handle = Some(handle);
            }
            Err(err) => log::error!("Failed to start microphone worker: {err}"),
        }
    }

    pub fn is_available(&self) -> bool {
        self.available
    }

    pub fn pause(&self) {
        if !self.available {
            return;
        }
        {
            let mut state = self.shared.quiesce.lock().unwrap();
            state.requested = true;
            if !state.worker_started {
                state.confirmed = true;
            }
        }
        // Handshake: wait up to 300ms for capture worker to confirm I2S DMA is disabled
        for _ in 0..30 {
            if self.shared.quiesce.lock().unwrap().confirmed {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn resume(&self) {
        if !self.available {
            return;
        }
        self.shared.quiesce.lock().unwrap().resume();
    }

    #[cfg(target_os = "espidf")]
    fn init_hardware_channel(right_channel: bool) -> Option<esp_idf_sys::i2s_chan_handle_t> {
        unsafe {
            let mut chan_cfg: esp_idf_sys::i2s_chan_config_t = std::mem::zeroed();
            chan_cfg.id = esp_idf_sys::i2s_port_t_I2S_NUM_AUTO;
            chan_cfg.role = esp_idf_sys::i2s_role_t_I2S_ROLE_MASTER;
            // 4 x 128 frames (2 KiB internal DMA RAM) suffice for effects.
            chan_cfg.dma_desc_num = 4;
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

            let Some(pins) = crate::pins::get_default_profile().mic_pins else {
                esp_idf_sys::i2s_del_channel(rx_handle);
                return None;
            };
            #[cfg(esp32s3)]
            {
                std_cfg.slot_cfg.left_align = true;
                std_cfg.slot_cfg.big_endian = false;
                std_cfg.slot_cfg.bit_order_lsb = false;
            }
            #[cfg(not(esp32s3))]
            {
                std_cfg.slot_cfg.msb_right = false;
            }
            std_cfg.gpio_cfg.mclk = esp_idf_sys::gpio_num_t_GPIO_NUM_NC;
            std_cfg.gpio_cfg.bclk = pins.sck as esp_idf_sys::gpio_num_t;
            std_cfg.gpio_cfg.ws = pins.ws as esp_idf_sys::gpio_num_t;
            std_cfg.gpio_cfg.dout = esp_idf_sys::gpio_num_t_GPIO_NUM_NC;
            std_cfg.gpio_cfg.din = pins.sd as esp_idf_sys::gpio_num_t;

            let init_err = esp_idf_sys::i2s_channel_init_std_mode(rx_handle, &std_cfg);
            if init_err != esp_idf_sys::ESP_OK {
                log::error!("Failed to initialize I2S std mode: {init_err}");
                esp_idf_sys::i2s_del_channel(rx_handle);
                return None;
            }

            let err = esp_idf_sys::i2s_channel_enable(rx_handle);
            if err != esp_idf_sys::ESP_OK {
                log::error!("Failed to enable I2S channel: {err}");
                esp_idf_sys::i2s_del_channel(rx_handle);
                return None;
            }
            Some(rx_handle)
        }
    }

    fn capture_worker_loop(shared: Arc<CaptureShared>) {
        #[cfg(target_os = "espidf")]
        let mut dsp = Box::new(MicDsp::new());
        #[cfg(target_os = "espidf")]
        let start_time = std::time::Instant::now();

        #[cfg(target_os = "espidf")]
        let mut right_channel_active = shared
            .settings
            .read()
            .map(|s| s.right_channel)
            .unwrap_or(false);

        // The channel is created lazily once capture is enabled.
        #[cfg(target_os = "espidf")]
        let mut rx_chan: Option<esp_idf_sys::i2s_chan_handle_t> = None;
        #[cfg(target_os = "espidf")]
        let mut init_attempted = false;

        #[cfg(target_os = "espidf")]
        let mut raw_buf = vec![0i32; MIC_BLOCK_SAMPLES].into_boxed_slice();
        #[cfg(target_os = "espidf")]
        let mut samples = vec![0.0f32; MIC_BLOCK_SAMPLES].into_boxed_slice();
        #[cfg(target_os = "espidf")]
        let mut last_stack_log = std::time::Instant::now();

        while !shared.shutdown.load(Ordering::Relaxed) {
            #[cfg(target_os = "espidf")]
            if last_stack_log.elapsed() >= Duration::from_secs(10) {
                crate::memory_diagnostics::log_current_stack("microphone");
                last_stack_log = std::time::Instant::now();
            }
            // Serialize the request and transition so old acknowledgements cannot
            // be published after resume, even across a rapid pause/resume/pause.
            let step = shared.quiesce.lock().unwrap().step(
                || {
                    #[cfg(target_os = "espidf")]
                    if let Some(chan) = rx_chan {
                        let err = unsafe { esp_idf_sys::i2s_channel_disable(chan) };
                        if err != esp_idf_sys::ESP_OK {
                            log::error!("Failed to disable I2S channel: {err}");
                            return false;
                        }
                    }
                    true
                },
                || {
                    #[cfg(target_os = "espidf")]
                    if let Some(chan) = rx_chan {
                        let err = unsafe { esp_idf_sys::i2s_channel_enable(chan) };
                        if err != esp_idf_sys::ESP_OK {
                            log::error!("Failed to re-enable I2S channel: {err}");
                            return false;
                        }
                    }
                    true
                },
            );
            if step != QuiesceStep::Running {
                std::thread::sleep(Duration::from_millis(20));
                continue;
            }

            if !shared.capture_enabled.load(Ordering::Acquire) {
                #[cfg(target_os = "espidf")]
                {
                    if let Some(chan) = rx_chan.take() {
                        unsafe {
                            let _ = esp_idf_sys::i2s_channel_disable(chan);
                            let _ = esp_idf_sys::i2s_del_channel(chan);
                        }
                        log::info!("Microphone capture stopped; I2S channel released");
                        dsp = Box::new(MicDsp::new());
                        crate::memory_diagnostics::log_heap("mic-stopped");
                    }
                    init_attempted = false;
                }
                if let Ok(mut lock) = shared.latest_snapshot.write() {
                    *lock = AudioSnapshot::default();
                }
                if let Ok(mut lock) = shared.latest_metrics.write() {
                    *lock = None;
                }
                if let Ok(mut lock) = shared.last_block_ms.write() {
                    *lock = None;
                }
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }

            #[cfg(target_os = "espidf")]
            if rx_chan.is_none() && !init_attempted {
                // One attempt per enable transition avoids log spam on a missing mic.
                init_attempted = true;
                right_channel_active = shared
                    .settings
                    .read()
                    .map(|s| s.right_channel)
                    .unwrap_or(false);
                shared.reconfigure_channel.store(false, Ordering::SeqCst);
                rx_chan = Self::init_hardware_channel(right_channel_active);
                crate::memory_diagnostics::log_heap(if rx_chan.is_some() {
                    "mic-started"
                } else {
                    "mic-start-failed"
                });
                if rx_chan.is_some() {
                    log::info!("Microphone capture started");
                }
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
                            100, // 100ms read timeout
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

                        let snap = dsp.process_at_sample_rate(
                            &samples,
                            &cur_settings,
                            now_ms,
                            MIC_SAMPLE_RATE,
                        );
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
            let mut state = self.shared.quiesce.lock().unwrap();
            state.worker_started = false;
            if state.requested {
                state.confirmed = true;
            }
        }
    }
}

impl AudioInput for I2sMicAudioInput {
    fn snapshot(&mut self, _now_ms: u64) -> AudioSnapshot {
        self.ensure_worker_started();
        if !self.available
            || !self.shared.capture_enabled.load(Ordering::Acquire)
            || self.shared.quiesce.lock().unwrap().requested
        {
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
        if !self.shared.capture_enabled.load(Ordering::Acquire) {
            return None;
        }
        self.shared.latest_metrics.read().ok().and_then(|m| *m)
    }

    fn set_active(&mut self, active: bool) {
        if self.available {
            self.shared.capture_enabled.store(active, Ordering::Release);
            if active {
                self.ensure_worker_started();
            }
        }
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

#[cfg(test)]
mod quiesce_regressions {
    use super::*;

    #[test]
    fn saved_mic_waits_for_services_and_starts_on_next_snapshot() {
        let mut mic = I2sMicAudioInput::new(true, MicSettings::default());
        let gate = mic.quiesce_handle();
        mic.set_active(true);
        mic.snapshot(0);
        assert!(mic.worker_handle.is_none());
        gate.network_services_initialized();
        assert!(mic.worker_handle.is_none());
        mic.snapshot(0);
        assert!(mic.worker_handle.is_some());
    }

    #[test]
    fn services_ready_during_ota_cannot_start_until_resume() {
        let mut mic = I2sMicAudioInput::new(true, MicSettings::default());
        let gate = mic.quiesce_handle();
        mic.set_active(true);
        gate.pause().unwrap();
        gate.network_services_initialized();
        mic.snapshot(0);
        assert!(mic.worker_handle.is_none());
        assert!(mic.shared.quiesce.lock().unwrap().confirmed);
        gate.resume();
        mic.snapshot(0);
        assert!(mic.worker_handle.is_some());
    }

    #[test]
    fn failed_spawn_retries_on_sampling() {
        let mut mic = I2sMicAudioInput::new(true, MicSettings::default());
        mic.quiesce_handle().network_services_initialized();
        mic.shared.capture_enabled.store(true, Ordering::SeqCst);
        mic.ensure_worker_started_with(|_| Err(std::io::Error::other("injected failure")));
        assert!(mic.worker_handle.is_none());
        let mut retried = false;
        mic.ensure_worker_started_with(|_| {
            retried = true;
            Err(std::io::Error::other("unexpected retry"))
        });
        assert!(!retried, "failed spawns must be rate limited");
        mic.last_spawn_attempt = Some(std::time::Instant::now() - Duration::from_secs(2));
        mic.snapshot(0);
        assert!(mic.worker_handle.is_some());
    }

    #[test]
    fn worker_starts_only_on_capture_activation() {
        let mut mic = I2sMicAudioInput::new(true, MicSettings::default());
        mic.quiesce_handle().network_services_initialized();
        assert!(mic.worker_handle.is_none());
        mic.set_active(false);
        assert!(mic.worker_handle.is_none());
        mic.set_active(true);
        assert!(mic.worker_handle.is_some());
        mic.set_active(false);
        mic.set_active(true);
        assert!(mic.worker_handle.is_some());
    }

    #[test]
    fn never_started_worker_can_quiesce_and_activation_remains_paused() {
        let mut mic = I2sMicAudioInput::new(true, MicSettings::default());
        let pause = mic.quiesce_handle();
        pause.network_services_initialized();
        pause.pause().unwrap();
        assert!(mic.worker_handle.is_none());
        mic.set_active(true);
        mic.snapshot(0);
        assert!(mic.worker_handle.is_none());
        assert!(mic.shared.quiesce.lock().unwrap().requested);
        pause.pause().unwrap();
        pause.resume();
        mic.snapshot(0);
        assert!(mic.worker_handle.is_some());
    }

    #[test]
    fn concurrent_activation_and_pause_preserve_quiescence() {
        for _ in 0..100 {
            let mut mic = I2sMicAudioInput::new(true, MicSettings::default());
            let pause = mic.quiesce_handle();
            pause.network_services_initialized();
            let barrier = Arc::new(std::sync::Barrier::new(2));
            let other = barrier.clone();
            let thread = std::thread::spawn(move || {
                other.wait();
                pause.pause().unwrap();
                pause
            });
            barrier.wait();
            mic.set_active(true);
            let pause = thread.join().unwrap();
            let state = mic.shared.quiesce.lock().unwrap();
            assert!(state.requested && state.confirmed);
            drop(state);
            drop(mic);
            pause.pause().unwrap();
            pause.resume();
        }
    }

    #[test]
    fn unavailable_mic_never_spawns() {
        let mut mic = I2sMicAudioInput::new(false, MicSettings::default());
        mic.set_active(true);
        assert!(mic.worker_handle.is_none());
        mic.quiesce_handle().pause().unwrap();
    }

    #[test]
    fn disable_failure_never_acknowledges_and_retries() {
        let mut state = QuiesceState {
            requested: true,
            ..Default::default()
        };
        assert_eq!(state.step(|| false, || true), QuiesceStep::Paused);
        assert!(!state.confirmed);
        assert_eq!(state.step(|| true, || true), QuiesceStep::Paused);
        assert!(state.confirmed);
    }

    #[test]
    fn resume_enable_failure_is_retried_without_reading() {
        let mut state = QuiesceState {
            requested: true,
            ..Default::default()
        };
        state.step(|| true, || true);
        state.resume();
        assert_eq!(state.step(|| true, || false), QuiesceStep::EnableFailed);
        assert!(!state.confirmed);
        assert_eq!(
            state.step(|| panic!("already disabled"), || true),
            QuiesceStep::Running
        );
        assert!(!state.disabled);
    }

    #[test]
    fn resumed_request_cannot_inherit_previous_ack() {
        let mut state = QuiesceState {
            requested: true,
            ..Default::default()
        };
        state.step(|| true, || true);
        state.resume();
        state.requested = true;
        assert!(!state.confirmed);
        assert_eq!(
            state.step(|| panic!("already disabled"), || panic!("still requested")),
            QuiesceStep::Paused
        );
        assert!(state.confirmed);
    }
    #[test]
    fn resume_waits_for_in_flight_disable_before_clearing_ack() {
        use std::sync::mpsc;
        let state = Arc::new(Mutex::new(QuiesceState {
            requested: true,
            ..Default::default()
        }));
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker_state = state.clone();
        let worker = std::thread::spawn(move || {
            worker_state.lock().unwrap().step(
                || {
                    entered_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    true
                },
                || true,
            );
        });
        entered_rx.recv().unwrap();
        let resume_state = state.clone();
        let resume = std::thread::spawn(move || resume_state.lock().unwrap().resume());
        release_tx.send(()).unwrap();
        worker.join().unwrap();
        resume.join().unwrap();
        let mut state = state.lock().unwrap();
        state.requested = true;
        assert!(!state.confirmed);
        assert_eq!(
            state.step(|| panic!("already disabled"), || true),
            QuiesceStep::Paused
        );
        assert!(state.confirmed);
    }
}
