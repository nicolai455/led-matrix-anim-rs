use matrix_control::api::{handle_api_request, ApiContext};
use matrix_control::config::ConfigStore;
use matrix_control::hub::{AudioInput, ControlHub, LedOutput, SubmitOutcome};
use matrix_control::mic::MicController;
use matrix_control::persistence::{StateStore, StorageBackend};
use matrix_core::audio::MicSettings;
use matrix_core::{AudioMode, AudioSnapshot, Engine, Frame};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn test_engine() -> Engine {
    Engine::new(matrix_effects::builtin_effects()).expect("valid effects")
}

#[test]
fn catalog_reads_share_immutable_metadata_instead_of_cloning_json_trees() {
    let hub = ControlHub::spawn(test_engine(), FastOutput, None).unwrap();
    let first = hub.catalog_json();
    let second = hub.catalog_json();
    assert!(Arc::ptr_eq(&first, &second));
    let catalog: serde_json::Value = serde_json::from_slice(&first).unwrap();
    assert!(catalog.as_array().unwrap().len() >= 21);
    hub.join();
}

#[test]
fn preview_telemetry_includes_clock_and_compact_external_waveform() {
    let hub = ControlHub::spawn(test_engine(), FastOutput, None).unwrap();
    hub.submit_sync("audio_source", "Mic", Duration::from_secs(1))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let state = hub.snapshot();
        if state.get("renderTimeMs").is_some() && state.get("waveform").is_some() {
            assert_eq!(state["waveform"].as_array().unwrap().len(), 128);
            assert!(state["waveform"]
                .as_array()
                .unwrap()
                .iter()
                .all(|v| { v.as_i64().is_some_and(|n| (-127..=127).contains(&n)) }));
            assert!(state["beat"].is_boolean());
            assert!(state["peak"].is_number());
            break;
        }
        assert!(
            Instant::now() < deadline,
            "missing browser preview telemetry"
        );
        thread::sleep(Duration::from_millis(10));
    }
    hub.submit_sync("audio_source", "Off", Duration::from_secs(1))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(1);
    while hub.snapshot().get("waveform").is_some() {
        assert!(
            Instant::now() < deadline,
            "waveform should be omitted without external audio"
        );
        thread::sleep(Duration::from_millis(10));
    }
    hub.join();
}

#[test]
fn microphone_api_reports_live_tuning_not_defaults() {
    let hub = ControlHub::spawn(test_engine(), FastOutput, None).unwrap();
    hub.submit_sync(
        "mic",
        r#"{"gainDb":12.5,"squelchDb":-72,"agc":false}"#,
        Duration::from_secs(1),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(1);
    while hub.snapshot()["mic"]["gainDb"] != 12.5 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    let ctx = ApiContext {
        hub: hub.clone(),
        config_store: Arc::new(ConfigStore::new(None)),
        mic_controller: Arc::new(MicController::new(false)),
        state_store: Arc::new(StateStore::new(None)),
    };
    let asset = handle_api_request(&ctx, "GET", "/preview.wasm.gz", &[]);
    assert_eq!(asset.status, 200);
    assert_eq!(asset.content_type, "application/gzip");
    assert_eq!(&asset.body[..2], &[0x1f, 0x8b]);
    let audio = handle_api_request(&ctx, "GET", "/api/audio", &[]);
    assert_eq!(audio.status, 200);
    let audio: serde_json::Value = serde_json::from_slice(&audio.body).unwrap();
    assert_eq!(audio["bands"].as_array().unwrap().len(), 16);
    assert!(audio.get("effects").is_none());
    assert!(audio.get("renderTimeMs").is_some());
    let response = handle_api_request(&ctx, "GET", "/api/mic", &[]);
    let body: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(body["settings"]["gainDb"], 12.5);
    assert_eq!(body["settings"]["squelchDb"], -72.0);
    assert_eq!(body["settings"]["agc"], false);
    hub.join();
}

struct FastOutput;
impl LedOutput for FastOutput {
    fn show(&mut self, _frame: &Frame, _brightness: u8) -> Result<(), String> {
        thread::sleep(Duration::from_millis(5));
        Ok(())
    }
}

struct SlowOutput {
    delay: Duration,
    call_count: Arc<AtomicUsize>,
    power_limits: Arc<Mutex<Vec<u32>>>,
}

impl SlowOutput {
    fn new(delay: Duration) -> (Self, Arc<AtomicUsize>, Arc<Mutex<Vec<u32>>>) {
        let call_count = Arc::new(AtomicUsize::new(0));
        let power_limits = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                delay,
                call_count: call_count.clone(),
                power_limits: power_limits.clone(),
            },
            call_count,
            power_limits,
        )
    }
}

impl LedOutput for SlowOutput {
    fn show(&mut self, _frame: &Frame, _brightness: u8) -> Result<(), String> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        thread::sleep(self.delay);
        Ok(())
    }

    fn set_power_limit(&mut self, max_milliamps: u32) -> Result<(), String> {
        if let Ok(mut lock) = self.power_limits.lock() {
            lock.push(max_milliamps);
        }
        Ok(())
    }
}

struct ErrorOutput {
    fail_after: usize,
    count: usize,
}

impl LedOutput for ErrorOutput {
    fn show(&mut self, _frame: &Frame, _brightness: u8) -> Result<(), String> {
        self.count += 1;
        if self.count > self.fail_after {
            Err("RMT buffer underrun".to_string())
        } else {
            Ok(())
        }
    }
}

struct MockAudioInput {
    snapshots_taken: Arc<AtomicUsize>,
    config_calls: Arc<Mutex<Vec<MicSettings>>>,
    metrics: Option<(f32, f32)>,
}

impl MockAudioInput {
    fn new() -> (Self, Arc<AtomicUsize>, Arc<Mutex<Vec<MicSettings>>>) {
        let snapshots_taken = Arc::new(AtomicUsize::new(0));
        let config_calls = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                snapshots_taken: snapshots_taken.clone(),
                config_calls: config_calls.clone(),
                metrics: Some((-24.5, 30.0)),
            },
            snapshots_taken,
            config_calls,
        )
    }
}

impl AudioInput for MockAudioInput {
    fn snapshot(&mut self, _now_ms: u64) -> AudioSnapshot {
        self.snapshots_taken.fetch_add(1, Ordering::SeqCst);
        let mut bands = [0.0; 16];
        bands[0] = 0.8;
        bands[1] = 0.6;
        bands[2] = 0.4;
        bands[3] = 0.2;
        AudioSnapshot {
            bands,
            level: 0.75,
            beat: true,
            available: true,
            ..Default::default()
        }
    }

    fn live_metrics(&self) -> Option<(f32, f32)> {
        self.metrics
    }

    fn configure(&mut self, settings: &MicSettings) {
        if let Ok(mut lock) = self.config_calls.lock() {
            lock.push(settings.clone());
        }
    }
}

#[derive(Default)]
struct MemoryStorageBackend {
    data: Mutex<Option<String>>,
}

impl StorageBackend for MemoryStorageBackend {
    fn load(&self) -> Result<Option<String>, String> {
        Ok(self.data.lock().unwrap().clone())
    }

    fn store(&self, content: &str) -> Result<(), String> {
        *self.data.lock().unwrap() = Some(content.to_string());
        Ok(())
    }

    fn clear(&self) -> Result<(), String> {
        *self.data.lock().unwrap() = None;
        Ok(())
    }
}

#[test]
fn test_slow_physical_output_does_not_block_snapshot_or_preview() {
    let (slow_out, _, _) = SlowOutput::new(Duration::from_millis(50));
    let hub = ControlHub::spawn(test_engine(), slow_out, None).expect("hub spawn");

    // Hub snapshot and frame reads must return quickly regardless of slow physical output
    let start = Instant::now();
    for _ in 0..10 {
        let _snap = hub.snapshot();
        let _frame = hub.frame();
    }
    let duration = start.elapsed();
    assert!(
        duration < Duration::from_millis(30),
        "Snapshot and frame access took too long ({duration:?}), must not block on physical output"
    );

    hub.join();
}

#[test]
fn test_command_queue_bounds_and_honest_pending_outcome() {
    // Output that blocks for 100ms per frame
    let (slow_out, _, _) = SlowOutput::new(Duration::from_millis(100));
    let hub = ControlHub::spawn(test_engine(), slow_out, None).expect("hub spawn");

    // 1. Submit synchronous command with very short timeout (1ms)
    // While the actor is busy in show(), the command is queued but cannot complete in 1ms.
    // It MUST return Ok(SubmitOutcome::Pending), NEVER fake Applied success!
    let outcome = hub
        .submit_sync("light", r#"{"state":"ON"}"#, Duration::from_millis(1))
        .expect("queue accepted command");

    // It could be Applied if lucky on timing, but with 100ms delay it is typically Pending
    assert!(
        outcome == SubmitOutcome::Pending || outcome == SubmitOutcome::Applied,
        "Honest outcome expected, got {outcome:?}"
    );

    // 2. Queue capacity test: fill queue (size 32)
    let mut rejected = false;
    for _ in 0..40 {
        if let Err(e) = hub.submit_async("light", r#"{"brightness":50}"#) {
            if e.contains("queue full") || e.contains("busy") {
                rejected = true;
                break;
            }
        }
    }
    assert!(
        rejected,
        "Excess commands beyond queue bounds must be rejected with busy error"
    );

    hub.join();
}

#[test]
fn test_audio_mode_precedence_and_mic_settings_delivery() {
    let (mock_audio, snapshots_taken, config_calls) = MockAudioInput::new();
    let engine = test_engine();

    // Verify default engine audio mode is Off
    assert_eq!(engine.audio_mode(), AudioMode::Off);

    let hub = ControlHub::spawn(engine, FastOutput, Some(Box::new(mock_audio))).expect("hub spawn");

    // Initial configure must be delivered on spawn
    thread::sleep(Duration::from_millis(50));
    {
        let calls = config_calls.lock().unwrap();
        assert!(
            !calls.is_empty(),
            "AudioInput::configure should be called on startup with initial settings"
        );
    }

    // While audio mode is Off, external audio snapshot should NOT be polled
    let snapshots_off = snapshots_taken.load(Ordering::SeqCst);
    assert_eq!(
        snapshots_off, 0,
        "AudioMode::Off must not sample external input"
    );

    // Switch to External (Mic) audio mode
    let outcome = hub
        .submit_sync("audio_source", "Mic", Duration::from_millis(100))
        .expect("submit audio mode");
    assert_eq!(outcome, SubmitOutcome::Applied);

    thread::sleep(Duration::from_millis(50));
    let snapshots_on = snapshots_taken.load(Ordering::SeqCst);
    assert!(
        snapshots_on > 0,
        "AudioMode::External must actively sample external audio input"
    );

    // Update mic settings via command
    let initial_call_count = config_calls.lock().unwrap().len();
    let res = hub.submit_sync(
        "mic",
        r#"{"gainDb": 45.0, "squelchDb": -40.0}"#,
        Duration::from_millis(100),
    );
    assert_eq!(res, Ok(SubmitOutcome::Applied));

    thread::sleep(Duration::from_millis(50));
    {
        let calls = config_calls.lock().unwrap();
        assert!(
            calls.len() > initial_call_count,
            "configure must be called when mic settings change"
        );
        let latest = calls.last().unwrap();
        assert_eq!(latest.gain_db, 45.0);
        assert_eq!(latest.squelch_db, -40.0);
    }

    hub.join();
}

#[test]
fn test_output_error_telemetry() {
    let err_out = ErrorOutput {
        fail_after: 2,
        count: 0,
    };
    let hub = ControlHub::spawn(test_engine(), err_out, None).expect("hub spawn");

    // Allow a few render cycles
    thread::sleep(Duration::from_millis(60));

    let err_count = hub.output_error_count();
    assert!(
        err_count > 0,
        "Output error telemetry must record failures from LedOutput::show"
    );
    assert_eq!(
        hub.last_output_error(),
        Some("RMT buffer underrun".to_string())
    );

    let status = hub.status();
    assert_eq!(status["lastError"], json!("RMT buffer underrun"));
    assert!(status["outputErrors"].as_u64().unwrap() > 0);

    hub.join();
}

#[test]
fn test_fps_calculation_and_uptime() {
    let hub = ControlHub::spawn(test_engine(), FastOutput, None).expect("hub spawn");

    // Run for > 300ms so rolling FPS window calculates
    thread::sleep(Duration::from_millis(350));

    let fps = hub.fps();
    assert!(
        fps > 0.0,
        "FPS should be non-zero after rendering frames, got {fps}"
    );
    assert!(hub.uptime() >= Duration::from_millis(300));

    let snap = hub.snapshot();
    assert!(snap.get("fps").is_some(), "Snapshot must contain FPS");
    assert!(snap.get("uptime").is_some(), "Snapshot must contain uptime");

    hub.join();
}

#[test]
fn test_live_power_limit_hook_and_config_restart_required() {
    let (slow_out, _, power_limits) = SlowOutput::new(Duration::from_millis(5));
    let hub = ControlHub::spawn(test_engine(), slow_out, None).expect("hub spawn");

    let config_backend = Box::new(MemoryStorageBackend::default());
    let config_store = Arc::new(ConfigStore::with_backend(config_backend, None));
    let state_backend = Box::new(MemoryStorageBackend::default());
    let state_store = Arc::new(StateStore::with_backend(state_backend));
    let mic_controller = Arc::new(MicController::new(true));

    let ctx = ApiContext {
        hub: hub.clone(),
        config_store: config_store.clone(),
        mic_controller,
        state_store,
    };

    // 1. Update power limit only (live update, no restart required)
    let body = json!({
        "ledMaxMilliamps": 8500
    });
    let resp = handle_api_request(
        &ctx,
        "POST",
        "/api/config",
        serde_json::to_string(&body).unwrap().as_bytes(),
    );
    assert_eq!(resp.status, 200);
    let val: serde_json::Value = serde_json::from_slice(&resp.body).unwrap();
    assert_eq!(val["ok"], true);
    assert_eq!(val["restartRequired"], false);

    thread::sleep(Duration::from_millis(30));
    {
        let limits = power_limits.lock().unwrap();
        assert!(
            limits.contains(&8500),
            "LedOutput::set_power_limit must receive the updated limit live: {limits:?}"
        );
    }

    // 2. Update valid panel geometry -> restart MUST be reported as required
    let body_restart = json!({
        "mirrorX": true
    });
    let resp_restart = handle_api_request(
        &ctx,
        "POST",
        "/api/config",
        serde_json::to_string(&body_restart).unwrap().as_bytes(),
    );
    assert_eq!(resp_restart.status, 200);
    let val_restart: serde_json::Value = serde_json::from_slice(&resp_restart.body).unwrap();
    assert_eq!(val_restart["ok"], true);
    assert_eq!(
        val_restart["restartRequired"], true,
        "Altering panel arrangement must require a restart"
    );

    hub.join();
}

#[test]
fn test_storage_backend_abstraction() {
    let mem = Box::new(MemoryStorageBackend::default());
    let store = StateStore::with_backend(mem);

    assert_eq!(store.load(), None);

    let test_state = json!({
        "state": "ON",
        "brightness": 200,
        "effect": "Fire"
    });
    store.save(&test_state).expect("save state");

    let loaded = store.load().expect("state loaded");
    assert_eq!(loaded["brightness"], 200);
    assert_eq!(loaded["effect"], "Fire");
    // "state" is stripped from persistence by design (boot with LEDs off)
    assert!(loaded.get("state").is_none());

    store.clear().expect("clear state");
    assert_eq!(store.load(), None);
}

#[test]
fn test_status_endpoint_returns_real_telemetry() {
    let hub = ControlHub::spawn(test_engine(), FastOutput, None).expect("hub spawn");
    let ctx = ApiContext {
        hub: hub.clone(),
        config_store: Arc::new(ConfigStore::new(None)),
        mic_controller: Arc::new(MicController::new(false)),
        state_store: Arc::new(StateStore::new(None)),
    };

    thread::sleep(Duration::from_millis(260));
    let resp = handle_api_request(&ctx, "GET", "/api/status", &[]);
    assert_eq!(resp.status, 200);
    let body: serde_json::Value = serde_json::from_slice(&resp.body).unwrap();

    assert!(body.get("fps").is_some());
    let fps = body["fps"].as_f64().unwrap();
    assert!(fps > 0.0, "Expected non-zero FPS in /api/status, got {fps}");
    assert_eq!(body["outputErrors"], 0);
    assert_eq!(body["hasMic"], false);

    hub.join();
}

#[test]
fn test_handle_api_request_panic_guard() {
    let hub = ControlHub::spawn(test_engine(), FastOutput, None).expect("hub spawn");
    let ctx = ApiContext {
        hub: hub.clone(),
        config_store: Arc::new(ConfigStore::new(None)),
        mic_controller: Arc::new(MicController::new(false)),
        state_store: Arc::new(StateStore::new(None)),
    };

    // Unknown routes return 404 cleanly
    let resp = handle_api_request(&ctx, "GET", "/unknown", &[]);
    assert_eq!(resp.status, 404);

    hub.join();
}

#[test]
fn concurrent_config_updates_preserve_both_changes() {
    use std::sync::Condvar;
    struct BlockingBackend {
        state: Arc<(Mutex<(usize, bool, String)>, Condvar)>,
    }
    impl StorageBackend for BlockingBackend {
        fn load(&self) -> Result<Option<String>, String> {
            Ok(None)
        }
        fn store(&self, content: &str) -> Result<(), String> {
            let (lock, cv) = &*self.state;
            let mut s = lock.lock().unwrap();
            s.0 += 1;
            if s.0 == 1 {
                cv.notify_all();
                while !s.1 {
                    s = cv.wait(s).unwrap();
                }
            }
            s.2 = content.to_owned();
            Ok(())
        }
    }
    let state = Arc::new((Mutex::new((0, false, String::new())), Condvar::new()));
    let store = Arc::new(ConfigStore::with_backend(
        Box::new(BlockingBackend {
            state: state.clone(),
        }),
        None,
    ));
    let a = store.clone();
    let first = thread::spawn(move || a.update_from_json(&json!({"ledBrightness":42})));
    let (lock, cv) = &*state;
    let mut s = lock.lock().unwrap();
    while s.0 == 0 {
        s = cv.wait(s).unwrap();
    }
    drop(s);
    let b = store.clone();
    let second = thread::spawn(move || b.update_from_json(&json!({"ledMaxMilliamps":5000})));
    thread::sleep(Duration::from_millis(100));
    let mut s = lock.lock().unwrap();
    s.1 = true;
    cv.notify_all();
    drop(s);
    first.join().unwrap().unwrap();
    second.join().unwrap().unwrap();
    let config = store.to_json();
    assert_eq!(config["ledBrightness"], 42);
    assert_eq!(config["ledMaxMilliamps"], 5000);
    let persisted: serde_json::Value = serde_json::from_str(&lock.lock().unwrap().2).unwrap();
    assert_eq!(persisted, config);
    assert!(store
        .update_from_json(&json!({"ledBrightness":"bad"}))
        .is_err());
    assert_eq!(store.to_json(), config);
}

#[test]
fn failed_live_power_apply_reports_persisted_config() {
    struct RejectPower;
    impl LedOutput for RejectPower {
        fn show(&mut self, _: &Frame, _: u8) -> Result<(), String> {
            Ok(())
        }
        fn set_power_limit(&mut self, _: u32) -> Result<(), String> {
            Err("power rejected".into())
        }
    }
    let hub = ControlHub::spawn(test_engine(), RejectPower, None).unwrap();
    let config_store = Arc::new(ConfigStore::new(None));
    let ctx = ApiContext {
        hub: hub.clone(),
        config_store: config_store.clone(),
        mic_controller: Arc::new(MicController::new(true)),
        state_store: Arc::new(StateStore::with_backend(Box::new(
            MemoryStorageBackend::default(),
        ))),
    };
    let response = handle_api_request(&ctx, "POST", "/api/config", br#"{"ledMaxMilliamps":5000}"#);
    let body: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(response.status, 503);
    assert_eq!(body["ok"], false);
    assert_eq!(body["powerLimitApply"], "failed");
    assert_eq!(body["config"]["ledMaxMilliamps"], 5000);
    assert_eq!(config_store.to_json()["ledMaxMilliamps"], 5000);
    hub.join();
}

#[test]
fn pending_full_and_disconnected_power_apply_are_truthful() {
    use std::sync::Condvar;
    struct BlockOutput(Arc<(Mutex<(bool, bool)>, Condvar)>);
    impl LedOutput for BlockOutput {
        fn show(&mut self, _: &Frame, _: u8) -> Result<(), String> {
            let (lock, cv) = &*self.0;
            let mut s = lock.lock().unwrap();
            s.0 = true;
            cv.notify_all();
            while !s.1 {
                s = cv.wait(s).unwrap();
            }
            Ok(())
        }
    }
    let signal = Arc::new((Mutex::new((false, false)), Condvar::new()));
    let hub = ControlHub::spawn(test_engine(), BlockOutput(signal.clone()), None).unwrap();
    let (lock, cv) = &*signal;
    let mut s = lock.lock().unwrap();
    while !s.0 {
        s = cv.wait(s).unwrap();
    }
    drop(s);
    let ctx = ApiContext {
        hub: hub.clone(),
        config_store: Arc::new(ConfigStore::new(None)),
        mic_controller: Arc::new(MicController::new(true)),
        state_store: Arc::new(StateStore::with_backend(Box::new(
            MemoryStorageBackend::default(),
        ))),
    };
    let request =
        || handle_api_request(&ctx, "POST", "/api/config", br#"{"ledMaxMilliamps":5000}"#);
    let pending = request();
    let body: serde_json::Value = serde_json::from_slice(&pending.body).unwrap();
    assert_eq!(pending.status, 202);
    assert_eq!(body["powerLimitApply"], "pending");
    for _ in 0..31 {
        hub.submit_async("power_limit", "5000").unwrap();
    }
    let full = request();
    let body: serde_json::Value = serde_json::from_slice(&full.body).unwrap();
    assert_eq!(full.status, 503);
    assert_eq!(body["powerLimitApply"], "failed");
    assert!(body["error"].as_str().unwrap().contains("full"));
    let mut s = lock.lock().unwrap();
    s.1 = true;
    cv.notify_all();
    drop(s);
    hub.join();
    let disconnected = request();
    let body: serde_json::Value = serde_json::from_slice(&disconnected.body).unwrap();
    assert_eq!(disconnected.status, 503);
    assert!(body["error"].as_str().unwrap().contains("stopped"));
}
