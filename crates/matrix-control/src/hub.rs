use matrix_core::audio::MicSettings;
use matrix_core::{AudioMode, AudioSnapshot, Engine, Frame, PanelConfig};
use matrix_core::engine::DetachedSnapshot;
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

#[cfg(test)]
fn publish_snapshot_with<E>(cache: &RwLock<Arc<Vec<u8>>>, build: impl FnOnce() -> Result<Vec<u8>, E>) {
    if let Ok(bytes) = build() {
        let bytes = Arc::new(bytes);
        if let Ok(mut lock) = cache.write() {
            *lock = bytes;
        }
    }
}

type Replies = Vec<(SyncSender<Result<(), String>>, Result<(), String>)>;
struct Telemetry {
    state: Arc<DetachedSnapshot>, audio: AudioSnapshot, mode: AudioMode,
    history: std::collections::VecDeque<(u64, [u8; 16])>, fps: f32, errors: u32,
    uptime: u64, has_mic: bool, metrics: Option<(f32, f32)>, now_ms: u64, replies: Replies,
}
fn publish_telemetry(cache: &RwLock<Arc<Vec<u8>>>, item: Telemetry) {
    let result = (|| -> Result<(), String> {
        let mut snap = serde_json::to_value(&*item.state).map_err(|e| e.to_string())?;
        let obj = snap.as_object_mut().ok_or("invalid snapshot")?;
        obj.insert("fps".into(), json!(item.fps)); obj.insert("outputErrors".into(), json!(item.errors));
        obj.insert("uptime".into(), json!(item.uptime)); obj.insert("hasMic".into(), json!(item.has_mic));
        if let Some((raw, gain)) = item.metrics { obj.insert("micRawDbfs".into(), json!(raw)); obj.insert("micGainDb".into(), json!(gain)); }
        obj.insert("bands".into(), json!(item.audio.bands)); obj.insert("level".into(), json!(item.audio.level));
        obj.insert("peak".into(), json!(item.audio.peak)); obj.insert("beat".into(), json!(item.audio.beat));
        obj.insert("renderTimeMs".into(), json!(item.now_ms));
        if item.mode == AudioMode::External {
            obj.insert("audioHistory".into(), json!(item.history));
            let waveform: Vec<i8> = item.audio.waveform.iter().map(|v| (v.clamp(-1.0, 1.0) * 127.0).round() as i8).collect();
            obj.insert("waveform".into(), json!(waveform));
        }
        obj.insert("bass".into(), json!(item.audio.bands[0..4].iter().sum::<f32>() / 4.0));
        obj.insert("mid".into(), json!(item.audio.bands[4..10].iter().sum::<f32>() / 6.0));
        obj.insert("treble".into(), json!(item.audio.bands[10..16].iter().sum::<f32>() / 6.0));
        let bytes = serde_json::to_vec(&snap).map_err(|e| e.to_string())?;
        *cache.write().map_err(|_| "snapshot cache poisoned".to_string())? = Arc::new(bytes); Ok(())
    })();
    for (reply, command_result) in item.replies { let _ = reply.try_send(command_result.and_then(|()| result.clone())); }
}

pub struct ControlHubInner {
    command_tx: SyncSender<CommandMessage>,
    cached_snapshot: Arc<RwLock<Arc<Vec<u8>>>>,
    cached_frame: Arc<RwLock<Frame>>,
    cached_catalog: Arc<Vec<u8>>,
    shutdown: Arc<AtomicBool>,
    has_mic_hardware: bool,
    thread_handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    live_metrics: Arc<RwLock<Option<(f32, f32)>>>,
    fps: Arc<RwLock<f32>>,
    output_errors: Arc<AtomicU32>,
    render_progress: Arc<AtomicU32>,
    output_successes: Arc<AtomicU32>,
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
            Arc::new(engine.catalog_json().map_err(|e| e.to_string())?);
        let initial_snapshot = Arc::new(engine.snapshot_json(&serde_json::Map::new()).map_err(|e| e.to_string())?);
        let initial_frame = Frame::default();

        let (command_tx, command_rx) = mpsc::sync_channel::<CommandMessage>(32);
        let shutdown = Arc::new(AtomicBool::new(false));

        let cached_snapshot = Arc::new(RwLock::new(initial_snapshot));
        let (telemetry_tx, telemetry_rx) = mpsc::sync_channel::<Telemetry>(2);
        let worker_cache = cached_snapshot.clone();
        std::thread::Builder::new().name("telemetry-worker".into()).stack_size(16384)
            .spawn(move || { while let Ok(item) = telemetry_rx.recv() { publish_telemetry(&worker_cache, item); } })
            .map_err(|e| format!("failed to spawn telemetry worker: {e}"))?;
        let cached_frame = Arc::new(RwLock::new(initial_frame));
        let thread_handle = Arc::new(Mutex::new(None));
        let live_metrics = Arc::new(RwLock::new(None));
        let fps = Arc::new(RwLock::new(0.0));
        let output_errors = Arc::new(AtomicU32::new(0));
        let render_progress = Arc::new(AtomicU32::new(0));
        let output_successes = Arc::new(AtomicU32::new(0));
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
            render_progress: render_progress.clone(),
            output_successes: output_successes.clone(),
            last_output_error: last_output_error.clone(),
            start_time,
        });

        let cached_frame_for_thread = cached_frame;
        let shutdown_for_thread = shutdown;
        let live_metrics_for_thread = live_metrics;
        let fps_for_thread = fps;
        let output_errors_for_thread = output_errors;
        let output_successes_for_thread = output_successes;
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
                let mut audio_history = std::collections::VecDeque::with_capacity(16);
                let mut last_audio_sample_ms = 0u64;

                let mut replies = Vec::with_capacity(16);
                let mut state = Arc::new(engine.capture_snapshot());
                let mut published_state = state.clone();
                let mut pending: Option<Telemetry> = None;
                while !shutdown_for_thread.load(Ordering::Relaxed) {
                    // 1. Drain up to finite batch of commands (e.g. 16)
                    let mut state_changed = false;
                    if let Some(item) = pending.take() {
                        match telemetry_tx.try_send(item) {
                            Ok(()) => {},
                            Err(mpsc::TrySendError::Full(item)) => pending = Some(item),
                            Err(mpsc::TrySendError::Disconnected(item)) => {
                                for (reply, _) in item.replies { let _ = reply.try_send(Err("telemetry worker stopped".into())); } break;
                            }
                        }
                    }
                    for _ in 0..if pending.is_some() { 0 } else { 16 } {
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
                                    replies.push((reply_tx, res.clone()));
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

                    if engine.audio_mode() == AudioMode::External {
                        if now_ms.saturating_sub(last_audio_sample_ms) >= 50 {
                            if audio_history.len() == 16 {
                                audio_history.pop_front();
                            }
                            let bands = audio_snap.bands.map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8);
                            audio_history.push_back((now_ms, bands));
                            last_audio_sample_ms = now_ms;
                        }
                    } else {
                        audio_history.clear();
                    }

                    let previous_audio_status = engine.audio_status();
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

                    // Publish the fixed-size frame without waiting on a preview reader.
                    if let Ok(mut lock) = cached_frame_for_thread.try_write() {
                        *lock = frame.clone();
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

                    let snapshot_interval = if engine.audio_mode() == AudioMode::External { Duration::from_millis(200) } else { Duration::from_millis(1000) };
                    if engine.is_dirty() || state_changed || engine.audio_status() != previous_audio_status { state = Arc::new(engine.capture_snapshot()); engine.clear_dirty(); }
                    if pending.is_none() && (!Arc::ptr_eq(&state, &published_state) || !replies.is_empty() || last_snapshot_sync.elapsed() >= snapshot_interval) {
                        let item = Telemetry { state: state.clone(), audio: audio_snap.clone(), mode: engine.audio_mode(),
                            history: audio_history.clone(), fps: fps_for_thread.read().map(|l| *l).unwrap_or(0.0),
                            errors: output_errors_for_thread.load(Ordering::Relaxed), uptime: start_time.elapsed().as_secs(),
                            has_mic: has_mic_hardware, metrics: live_metrics_for_thread.read().ok().and_then(|m| *m),
                            now_ms, replies: std::mem::take(&mut replies) };
                        match telemetry_tx.try_send(item) {
                            Ok(()) => { published_state = state.clone(); last_snapshot_sync = Instant::now(); },
                            Err(mpsc::TrySendError::Full(item)) => { if !item.replies.is_empty() { pending = Some(item); } },
                            Err(mpsc::TrySendError::Disconnected(item)) => {
                                for (reply, _) in item.replies { let _ = reply.try_send(Err("telemetry worker stopped".into())); } break;
                            }
                        }
                    }

                    render_progress.fetch_add(1, Ordering::Relaxed);
                    // 4. Physical blocking output is frame backpressure!
                    // Notice: no lock is held across show().
                    match output.show(&frame, brightness) {
                        Ok(()) => { output_successes_for_thread.fetch_add(1, Ordering::Relaxed); }
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

    /// Successful physical output calls and errors, for startup validation.
    pub fn render_progress(&self) -> u32 { self.inner.render_progress.load(Ordering::Relaxed) }

    pub fn output_progress(&self) -> (u32, u32) {
        (self.inner.output_successes.load(Ordering::Relaxed), self.inner.output_errors.load(Ordering::Relaxed))
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
        serde_json::from_slice(self.snapshot_json().as_slice()).unwrap_or(Value::Null)
    }

    /// Shared serialized state, without rebuilding the effect parameter tree.
    pub fn snapshot_json(&self) -> Arc<Vec<u8>> {
        self.inner.cached_snapshot.read().map(|v| v.clone()).unwrap_or_else(|_| Arc::new(b"null".to_vec()))
    }

    /// Deserialize only scalar fields; skip the potentially large effects object.
    pub fn snapshot_scalar(&self) -> Value {
        let bytes = self.snapshot_json();
        let fields: Result<std::collections::BTreeMap<&str, &serde_json::value::RawValue>, _> = serde_json::from_slice(&bytes);
        let mut result = serde_json::Map::new();
        if let Ok(fields) = fields {
            for (key, raw) in fields {
                if key != "effects" {
                    if let Ok(value) = serde_json::from_str(raw.get()) { result.insert(key.to_owned(), value); }
                }
            }
        }
        Value::Object(result)
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
        self.inner.cached_frame.read().map(|frame| frame.clone()).unwrap_or_default()
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

#[cfg(test)]
mod snapshot_publish_tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn detached_snapshot_is_immutable_and_matches_engine_schema() {
        let mut engine = Engine::new(matrix_effects::builtin_effects()).unwrap();
        let detached = engine.capture_snapshot();
        let before = engine.snapshot();
        engine.apply("light", r#"{"brightness":17}"#).unwrap();
        assert_eq!(serde_json::to_value(&detached).unwrap(), before);
        assert_ne!(serde_json::to_value(&detached).unwrap(), engine.snapshot());
    }

    #[test]
    fn synchronous_command_publishes_snapshot_before_reply() {
        let engine = Engine::new(matrix_effects::builtin_effects()).unwrap();
        struct Sink;
        impl LedOutput for Sink {
            fn show(&mut self, _: &Frame, _: u8) -> Result<(), String> { Ok(()) }
        }
        let hub = ControlHub::spawn(engine, Sink, None).unwrap();
        for brightness in [17, 213, 48, 192] {
            hub.submit_sync("light", &format!(r#"{{"brightness":{brightness}}}"#), Duration::from_secs(1)).unwrap();
            assert_eq!(hub.snapshot()["brightness"], brightness);
        }
        hub.join();
    }

    #[test]
    fn saturated_worker_does_not_block_output_or_lose_replies() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct CountingSink(Arc<AtomicUsize>);
        impl LedOutput for CountingSink {
            fn show(&mut self, _: &Frame, _: u8) -> Result<(), String> {
                self.0.fetch_add(1, Ordering::Relaxed);
                std::thread::sleep(Duration::from_millis(1));
                Ok(())
            }
        }
        let frames = Arc::new(AtomicUsize::new(0));
        let hub = ControlHub::spawn(Engine::new(matrix_effects::builtin_effects()).unwrap(), CountingSink(frames.clone()), None).unwrap();
        let guard = hub.inner.cached_snapshot.write().unwrap();
        let mut receivers = Vec::new();
        for n in 0..24 {
            let (tx, rx) = mpsc::sync_channel(1);
            hub.inner.command_tx.send(CommandMessage { kind: "light".into(), payload: format!(r#"{{"brightness":{n}}}"#), reply: Some(tx) }).unwrap();
            receivers.push(rx);
            std::thread::sleep(Duration::from_millis(3));
        }
        let count = frames.load(Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(30));
        assert!(frames.load(Ordering::Relaxed) > count, "output stalled behind cache worker");
        assert!(receivers.iter().all(|rx| rx.try_recv().is_err()), "reply before publication");
        drop(guard);
        for rx in receivers { assert_eq!(rx.recv_timeout(Duration::from_secs(3)).unwrap(), Ok(())); }
        assert_eq!(hub.snapshot()["brightness"], 23);
        hub.join();
    }

    #[test]
    fn readers_are_not_blocked_while_snapshot_is_built() {
        let cache = Arc::new(RwLock::new(Arc::new(b"old".to_vec())));
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let writer_cache = cache.clone();
        let writer = std::thread::spawn(move || {
            publish_snapshot_with(&writer_cache, || {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok::<_, ()>(b"new".to_vec())
            });
        });
        entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        let old = cache.try_read().expect("serialization held snapshot lock").clone();
        assert_eq!(&**old, b"old");
        release_tx.send(()).unwrap();
        writer.join().unwrap();
        assert_eq!(&**cache.read().unwrap(), b"new");
        assert_eq!(&**old, b"old");
    }
}
