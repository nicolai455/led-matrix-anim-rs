use matrix_core::audio::MicSettings;
use matrix_core::{AudioMode, AudioSnapshot, Engine, Frame, PanelConfig};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

pub const MAX_KIND_BYTES: usize = 128;
pub const MAX_PAYLOAD_BYTES: usize = 2048;

pub trait LedOutput: Send + 'static {
    fn show(&mut self, frame: &Frame, brightness: u8) -> Result<(), String>;

    /// Optional live power budget / current limit configuration hook.
    fn set_power_limit(&mut self, _max_milliamps: u32) -> Result<(), String> {
        Ok(())
    }

    /// Optional live panel configuration hook.
    fn update_config(&mut self, _config: &PanelConfig) -> Result<(), String> {
        Ok(())
    }
}

pub trait AudioInput: Send + 'static {
    fn snapshot(&mut self, now_ms: u64) -> AudioSnapshot;

    fn live_metrics(&self) -> Option<(f32, f32)> {
        None
    }

    /// Hook called when microphone DSP parameters change.
    fn configure(&mut self, _settings: &MicSettings) {}
}

pub struct CommandMessage {
    pub kind: String,
    pub payload: String,
    pub reply: Option<SyncSender<Result<(), String>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmitOutcome {
    Applied,
    Pending,
}

pub struct ControlHubInner {
    command_tx: SyncSender<CommandMessage>,
    cached_snapshot: Arc<RwLock<Arc<Value>>>,
    cached_frame: Arc<RwLock<Arc<Frame>>>,
    cached_catalog: Arc<Vec<u8>>,
    shutdown: Arc<AtomicBool>,
    has_mic_hardware: bool,
    thread_handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    live_metrics: Arc<RwLock<Option<(f32, f32)>>>,
    fps: Arc<RwLock<f32>>,
    output_errors: Arc<AtomicU32>,
    last_output_error: Arc<RwLock<Option<String>>>,
    start_time: Instant,
}

impl Drop for ControlHubInner {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Ok(mut lock) = self.thread_handle.lock() {
            if let Some(handle) = lock.take() {
                let _ = handle.join();
            }
        }
    }
}

#[derive(Clone)]
pub struct ControlHub {
    inner: Arc<ControlHubInner>,
}

impl ControlHub {
    /// Spawns a dedicated render actor thread driving Engine and LedOutput.
    ///
    /// Backpressure is determined strictly by physical blocking output, not fixed FPS.
    /// Yields each iteration for scheduling fairness.
    /// State locks are never held across render/show or network calls.
    pub fn spawn(
        mut engine: Engine,
        mut output: impl LedOutput,
        mut audio: Option<Box<dyn AudioInput>>,
    ) -> Result<Self, String> {
        let has_mic_hardware = audio.is_some();
        let initial_catalog =
            Arc::new(serde_json::to_vec(&engine.catalog()).map_err(|e| e.to_string())?);
        let initial_snapshot = Arc::new(engine.snapshot());
        let initial_frame = Arc::new(Frame::default());

        let (command_tx, command_rx) = mpsc::sync_channel::<CommandMessage>(32);
        let shutdown = Arc::new(AtomicBool::new(false));

        let cached_snapshot = Arc::new(RwLock::new(initial_snapshot));
        let cached_frame = Arc::new(RwLock::new(initial_frame));
        let thread_handle = Arc::new(Mutex::new(None));
        let live_metrics = Arc::new(RwLock::new(None));
        let fps = Arc::new(RwLock::new(0.0));
        let output_errors = Arc::new(AtomicU32::new(0));
        let last_output_error = Arc::new(RwLock::new(None));
        let start_time = Instant::now();

        let inner = Arc::new(ControlHubInner {
            command_tx,
            cached_snapshot: cached_snapshot.clone(),
            cached_frame: cached_frame.clone(),
            cached_catalog: initial_catalog,
            shutdown: shutdown.clone(),
            has_mic_hardware,
            thread_handle: thread_handle.clone(),
            live_metrics: live_metrics.clone(),
            fps: fps.clone(),
            output_errors: output_errors.clone(),
            last_output_error: last_output_error.clone(),
            start_time,
        });

        let cached_snapshot_for_thread = cached_snapshot;
        let cached_frame_for_thread = cached_frame;
        let shutdown_for_thread = shutdown;
        let live_metrics_for_thread = live_metrics;
        let fps_for_thread = fps;
        let output_errors_for_thread = output_errors;
        let last_output_error_for_thread = last_output_error;

        #[cfg(target_os = "espidf")]
        let stack_size = 24576;
        #[cfg(not(target_os = "espidf"))]
        let stack_size = 131072;

        let spawn_res = std::thread::Builder::new()
            .name("matrix-render".to_string())
            .stack_size(stack_size)
            .spawn(move || {
                let mut last_snapshot_sync = Instant::now();
                let mut last_mic_settings = engine.mic_settings().clone();

                // Initial delivery of microphone DSP settings
                if let Some(ref mut input) = audio {
                    input.configure(&last_mic_settings);
                }

                let mut fps_window_start = Instant::now();
                let mut fps_frames: u32 = 0;

                while !shutdown_for_thread.load(Ordering::Relaxed) {
                    // 1. Drain up to finite batch of commands (e.g. 16)
                    let mut state_changed = false;
                    for _ in 0..16 {
                        match command_rx.try_recv() {
                            Ok(cmd) => {
                                let res = if cmd.kind == "power_limit" {
                                    match cmd.payload.trim().parse::<u32>() {
                                        Ok(limit) => output.set_power_limit(limit),
                                        Err(e) => Err(format!("invalid power limit: {e}")),
                                    }
                                } else if cmd.kind == "config" {
                                    match serde_json::from_str::<PanelConfig>(&cmd.payload) {
                                        Ok(cfg) => {
                                            let _ = output.set_power_limit(cfg.led_max_milliamps);
                                            output.update_config(&cfg)
                                        }
                                        Err(e) => Err(format!("invalid panel config: {e}")),
                                    }
                                } else {
                                    engine.apply(&cmd.kind, &cmd.payload)
                                };
                                if let Some(reply_tx) = cmd.reply {
                                    let _ = reply_tx.try_send(res.clone());
                                }
                                if res.is_ok() {
                                    state_changed = true;
                                }
                            }
                            Err(mpsc::TryRecvError::Empty) => break,
                            Err(mpsc::TryRecvError::Disconnected) => {
                                return;
                            }
                        }
                    }

                    // Microphone settings delivery hook (only when settings changed)
                    if let Some(ref mut input) = audio {
                        let current_mic = engine.mic_settings();
                        if current_mic != &last_mic_settings {
                            input.configure(current_mic);
                            last_mic_settings = current_mic.clone();
                        }
                        if let Some(metrics) = input.live_metrics() {
                            if let Ok(mut lock) = live_metrics_for_thread.write() {
                                *lock = Some(metrics);
                            }
                        }
                    }

                    // 2. Audio sampling: select by engine audio mode first
                    let now_ms = start_time.elapsed().as_millis() as u64;
                    let audio_snap = match engine.audio_mode() {
                        AudioMode::Off => AudioSnapshot::default(),
                        AudioMode::Synthetic => matrix_core::synthetic_audio(now_ms),
                        AudioMode::External => {
                            if let Some(ref mut input) = audio {
                                input.snapshot(now_ms)
                            } else {
                                AudioSnapshot::default()
                            }
                        }
                    };

                    // 3. Render frame
                    let frame = if engine.is_on() {
                        engine.render(now_ms, &audio_snap)
                    } else {
                        // OFF must not reallocate an effect released by power-down.
                        Frame::default()
                    };
                    let brightness = if engine.is_on() {
                        engine.brightness()
                    } else {
                        0
                    };

                    // Cache preview frame (fast pointer swap)
                    if let Ok(mut lock) = cached_frame_for_thread.write() {
                        *lock = Arc::new(frame.clone());
                    }

                    // FPS calculation (rolling window ~250ms)
                    fps_frames += 1;
                    let fps_elapsed = fps_window_start.elapsed();
                    if fps_elapsed >= Duration::from_millis(250) {
                        let calculated_fps = (fps_frames as f64 / fps_elapsed.as_secs_f64()) as f32;
                        if let Ok(mut lock) = fps_for_thread.write() {
                            *lock = calculated_fps;
                        }
                        fps_window_start = Instant::now();
                        fps_frames = 0;
                    }

                    // Refresh snapshot periodically (every 100ms) or immediately on state change
                    if state_changed
                        || engine.is_dirty()
                        || last_snapshot_sync.elapsed() >= Duration::from_millis(100)
                    {
                        let mut snap = engine.snapshot();
                        let cur_fps = fps_for_thread.read().map(|l| *l).unwrap_or(0.0);
                        let errs = output_errors_for_thread.load(Ordering::Relaxed);
                        if let Some(obj) = snap.as_object_mut() {
                            obj.insert("fps".to_string(), json!(cur_fps));
                            obj.insert("outputErrors".to_string(), json!(errs));
                            obj.insert("uptime".to_string(), json!(start_time.elapsed().as_secs()));
                            obj.insert("hasMic".to_string(), json!(has_mic_hardware));
                            if let Ok(lock) = live_metrics_for_thread.read() {
                                if let Some((raw, gain)) = *lock {
                                    obj.insert("micRawDbfs".to_string(), json!(raw));
                                    obj.insert("micGainDb".to_string(), json!(gain));
                                }
                            }
                            obj.insert("bands".to_string(), json!(audio_snap.bands.to_vec()));
                            obj.insert("level".to_string(), json!(audio_snap.level));
                            obj.insert("peak".to_string(), json!(audio_snap.peak));
                            obj.insert("beat".to_string(), json!(audio_snap.beat));
                            obj.insert("renderTimeMs".to_string(), json!(now_ms));
                            // Only external audio needs transport; synthetic audio is
                            // generated in the browser. Quantize to keep telemetry small.
                            if engine.audio_mode() == AudioMode::External {
                                let waveform: Vec<i8> = audio_snap
                                    .waveform
                                    .iter()
                                    .map(|v| (v.clamp(-1.0, 1.0) * 127.0).round() as i8)
                                    .collect();
                                obj.insert("waveform".to_string(), json!(waveform));
                            }
                            let bass = (audio_snap.bands[0]
                                + audio_snap.bands[1]
                                + audio_snap.bands[2]
                                + audio_snap.bands[3])
                                / 4.0;
                            let mid = (audio_snap.bands[4]
                                + audio_snap.bands[5]
                                + audio_snap.bands[6]
                                + audio_snap.bands[7]
                                + audio_snap.bands[8]
                                + audio_snap.bands[9])
                                / 6.0;
                            let treble = (audio_snap.bands[10]
                                + audio_snap.bands[11]
                                + audio_snap.bands[12]
                                + audio_snap.bands[13]
                                + audio_snap.bands[14]
                                + audio_snap.bands[15])
                                / 6.0;
                            obj.insert("bass".to_string(), json!(bass));
                            obj.insert("mid".to_string(), json!(mid));
                            obj.insert("treble".to_string(), json!(treble));
                        }
                        if let Ok(mut lock) = cached_snapshot_for_thread.write() {
                            *lock = Arc::new(snap);
                        }
                        engine.clear_dirty();
                        last_snapshot_sync = Instant::now();
                    }

                    // 4. Physical blocking output is frame backpressure!
                    // Notice: no lock is held across show().
                    match output.show(&frame, brightness) {
                        Ok(()) => {}
                        Err(e) => {
                            output_errors_for_thread.fetch_add(1, Ordering::Relaxed);
                            if let Ok(mut lock) = last_output_error_for_thread.write() {
                                *lock = Some(e);
                            }
                        }
                    }

                    // Yield for scheduling fairness
                    std::thread::yield_now();
                }
            });

        let handle = match spawn_res {
            Ok(h) => h,
            Err(e) => return Err(format!("failed to spawn render actor thread: {e}")),
        };

        if let Ok(mut lock) = thread_handle.lock() {
            *lock = Some(handle);
        }

        Ok(ControlHub { inner })
    }

    /// Submits a command asynchronously with nonblocking queue acceptance.
    ///
    /// Bounds input byte lengths before string allocation.
    pub fn submit_async(&self, kind: &str, payload: &str) -> Result<(), String> {
        if kind.len() > MAX_KIND_BYTES {
            return Err(format!("command kind exceeds {MAX_KIND_BYTES} bytes"));
        }
        if payload.len() > MAX_PAYLOAD_BYTES {
            return Err(format!("command payload exceeds {MAX_PAYLOAD_BYTES} bytes"));
        }

        self.inner
            .command_tx
            .try_send(CommandMessage {
                kind: kind.to_string(),
                payload: payload.to_string(),
                reply: None,
            })
            .map_err(|e| match e {
                mpsc::TrySendError::Full(_) => "busy: command queue full".to_string(),
                mpsc::TrySendError::Disconnected(_) => "render thread stopped".to_string(),
            })
    }

    /// Submits a command asynchronously (alias for `submit_async`).
    pub fn submit(&self, kind: &str, payload: &str) -> Result<(), String> {
        self.submit_async(kind, payload)
    }

    /// Submits a command synchronously with bounded timeout.
    ///
    /// Returns:
    /// - `Ok(SubmitOutcome::Applied)` when processed within timeout and applied.
    /// - `Ok(SubmitOutcome::Pending)` when enqueued and waiting in queue past timeout.
    /// - `Err(reason)` when rejected by validation or queue is full.
    pub fn submit_sync(
        &self,
        kind: &str,
        payload: &str,
        timeout: Duration,
    ) -> Result<SubmitOutcome, String> {
        if kind.len() > MAX_KIND_BYTES {
            return Err(format!("command kind exceeds {MAX_KIND_BYTES} bytes"));
        }
        if payload.len() > MAX_PAYLOAD_BYTES {
            return Err(format!("command payload exceeds {MAX_PAYLOAD_BYTES} bytes"));
        }

        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.inner
            .command_tx
            .try_send(CommandMessage {
                kind: kind.to_string(),
                payload: payload.to_string(),
                reply: Some(reply_tx),
            })
            .map_err(|e| match e {
                mpsc::TrySendError::Full(_) => "busy: command queue full".to_string(),
                mpsc::TrySendError::Disconnected(_) => "render thread stopped".to_string(),
            })?;

        match reply_rx.recv_timeout(timeout) {
            Ok(result) => result.map(|()| SubmitOutcome::Applied),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(SubmitOutcome::Pending),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err("render thread disconnected".to_string())
            }
        }
    }

    /// Returns a cached snapshot of current Engine state.
    ///
    /// Never blocks on render or physical output.
    pub fn snapshot(&self) -> Value {
        self.inner
            .cached_snapshot
            .read()
            .map(|arc| (**arc).clone())
            .unwrap_or_else(|_| Value::Null)
    }

    /// Returns the static effect catalog.
    pub fn catalog(&self) -> Value {
        serde_json::from_slice(self.inner.cached_catalog.as_slice())
            .expect("internally serialized catalog must remain valid")
    }

    /// Shared compact JSON avoids retaining/cloning a large tree on embedded targets.
    pub fn catalog_json(&self) -> Arc<Vec<u8>> {
        self.inner.cached_catalog.clone()
    }

    /// Returns the latest rendered frame preview.
    ///
    /// Never blocks on physical output.
    pub fn frame(&self) -> Frame {
        self.inner
            .cached_frame
            .read()
            .map(|arc| (**arc).clone())
            .unwrap_or_default()
    }

    /// Current measured rendering frame rate.
    pub fn fps(&self) -> f32 {
        self.inner.fps.read().map(|l| *l).unwrap_or(0.0)
    }

    /// Total number of physical output errors since spawn.
    pub fn output_error_count(&self) -> u64 {
        self.inner.output_errors.load(Ordering::Relaxed) as u64
    }

    /// Most recent output error string, if any.
    pub fn last_output_error(&self) -> Option<String> {
        self.inner
            .last_output_error
            .read()
            .ok()
            .and_then(|l| l.clone())
    }

    /// Most recent audio live metrics (raw_dbfs, applied_gain_db).
    pub fn live_metrics(&self) -> Option<(f32, f32)> {
        self.inner.live_metrics.read().ok().and_then(|l| *l)
    }

    /// Time elapsed since the render actor thread was spawned.
    pub fn uptime(&self) -> Duration {
        self.inner.start_time.elapsed()
    }

    /// Returns rich system status snapshot.
    pub fn status(&self) -> Value {
        let snap = self.snapshot();
        let fallback = json!("OFF");
        let state = snap.get("state").unwrap_or(&fallback);
        json!({
            "fps": self.fps(),
            "state": state,
            "uptime": self.uptime().as_secs(),
            "outputErrors": self.output_error_count(),
            "lastError": self.last_output_error(),
            "micAvailable": self.has_mic_hardware(),
        })
    }

    /// Whether microphone hardware input was attached.
    pub fn has_mic_hardware(&self) -> bool {
        self.inner.has_mic_hardware
    }

    /// Requests clean shutdown of the render thread.
    pub fn shutdown(&self) {
        self.inner.shutdown.store(true, Ordering::SeqCst);
    }

    /// Cleanly requests shutdown and waits for render thread to terminate.
    pub fn join(&self) {
        self.shutdown();
        if let Ok(mut lock) = self.inner.thread_handle.lock() {
            if let Some(handle) = lock.take() {
                let _ = handle.join();
            }
        }
    }
}
