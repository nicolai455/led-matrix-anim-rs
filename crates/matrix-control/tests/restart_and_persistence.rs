use matrix_control::persistence::{
    normalize_runtime_state, FsStorageBackend, StateStore, StorageBackend, MAX_STORAGE_LOAD_BYTES,
};
use matrix_core::{Engine, Frame, LfxWriter, Rgb};
use serde_json::json;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static TEST_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let count = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("{prefix}_{pid}_{count}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("failed to create temp dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn get_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

fn create_test_lfx_file(path: &Path) {
    let mut buf = Vec::new();
    let mut writer = LfxWriter::new(&mut buf, 2).expect("create lfx writer");

    let mut frame0 = Frame::default();
    frame0.set(0, 0, Rgb::new(210, 42, 99));
    frame0.set(1, 1, Rgb::new(100, 200, 50));
    writer.write_frame(&frame0).expect("write frame 0");

    let mut frame1 = Frame::default();
    frame1.set(0, 0, Rgb::new(11, 22, 33));
    writer.write_frame(&frame1).expect("write frame 1");

    fs::write(path, buf).expect("write lfx file");
}

fn decode_http_body(headers: &str, raw_body: &[u8]) -> Result<Vec<u8>, String> {
    let mut is_chunked = false;
    for line in headers.lines() {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("transfer-encoding")
                && v.to_ascii_lowercase().contains("chunked")
            {
                is_chunked = true;
                break;
            }
        }
    }

    if !is_chunked {
        return Ok(raw_body.to_vec());
    }

    let mut cursor = 0;
    let mut decoded = Vec::new();
    while cursor < raw_body.len() {
        let rem = &raw_body[cursor..];
        let crlf = match rem.windows(2).position(|w| w == b"\r\n") {
            Some(p) => p,
            None => break,
        };
        let size_str = match std::str::from_utf8(&rem[..crlf]) {
            Ok(s) => s.trim(),
            Err(_) => break,
        };
        let chunk_size = match usize::from_str_radix(size_str, 16) {
            Ok(sz) => sz,
            Err(_) => break,
        };
        cursor += crlf + 2;

        if chunk_size == 0 {
            break;
        }

        if cursor + chunk_size > raw_body.len() {
            return Err("truncated chunk data".to_string());
        }

        decoded.extend_from_slice(&raw_body[cursor..cursor + chunk_size]);
        cursor += chunk_size + 2; // skip chunk data + CRLF
    }

    Ok(decoded)
}

fn http_request(
    addr: &str,
    method: &str,
    path: &str,
    body: Option<&[u8]>,
) -> Result<(u16, Vec<u8>), String> {
    let mut stream =
        TcpStream::connect(addr).map_err(|e| format!("connect to {addr} failed: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(1000)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_millis(1000)))
        .map_err(|e| e.to_string())?;

    let content_len = body.map(|b| b.len()).unwrap_or(0);
    let req_headers = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Length: {content_len}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(req_headers.as_bytes())
        .map_err(|e| e.to_string())?;
    if let Some(b) = body {
        stream.write_all(b).map_err(|e| e.to_string())?;
    }

    let mut response = Vec::new();
    let mut buf = [0u8; 1024];

    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                response.extend_from_slice(&buf[..n]);
                if response.windows(5).any(|w| w == b"0\r\n\r\n") {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    let header_sep = b"\r\n\r\n";
    let sep_pos = response
        .windows(4)
        .position(|w| w == header_sep)
        .ok_or_else(|| "no header separator found in HTTP response".to_string())?;

    let header_str =
        std::str::from_utf8(&response[..sep_pos]).map_err(|e| format!("headers not UTF-8: {e}"))?;
    let first_line = header_str
        .lines()
        .next()
        .ok_or_else(|| "empty status line".to_string())?;
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(format!("malformed status line: '{first_line}'"));
    }
    let code_str = parts[1];
    let status_code: u16 = code_str
        .parse()
        .map_err(|e| format!("invalid status code '{code_str}': {e}"))?;
    let raw_body = &response[sep_pos + 4..];
    let body_bytes = decode_http_body(header_str, raw_body)?;
    Ok((status_code, body_bytes))
}

fn http_get_json(addr: &str, path: &str) -> Result<(u16, serde_json::Value), String> {
    let (code, bytes) = http_request(addr, "GET", path, None)?;
    let val: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid response JSON: {e}"))?;
    Ok((code, val))
}

fn http_post_json(
    addr: &str,
    path: &str,
    payload: &serde_json::Value,
) -> Result<(u16, serde_json::Value), String> {
    let body_bytes = serde_json::to_vec(payload).unwrap();
    let (code, bytes) = http_request(addr, "POST", path, Some(&body_bytes))?;
    let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    Ok((code, val))
}

struct ProcessGuard {
    child: Option<Child>,
}

impl ProcessGuard {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let pid = child.id();
            // Try sending SIGTERM for clean shutdown
            let _ = Command::new("kill")
                .arg("-15")
                .arg(pid.to_string())
                .status();

            let start = Instant::now();
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) => {
                        if start.elapsed() > Duration::from_millis(1500) {
                            let _ = child.kill();
                            let _ = child.wait();
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    Err(_) => {
                        let _ = child.kill();
                        break;
                    }
                }
            }
        }
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        self.stop();
    }
}

fn wait_for_ready(addr: &str, child: &mut Child, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    let mut last_err = String::new();
    while Instant::now() < deadline {
        if let Ok(Some(status)) = child.try_wait() {
            let mut err_out = String::new();
            if let Some(mut e) = child.stderr.take() {
                let _ = e.read_to_string(&mut err_out);
            }
            return Err(format!(
                "process exited prematurely with {status}: {err_out}"
            ));
        }
        match http_get_json(addr, "/api/state") {
            Ok((200, _)) => return Ok(()),
            Ok((other, _)) => last_err = format!("status {other}"),
            Err(e) => last_err = e,
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(format!(
        "server at {addr} failed to become ready within {timeout:?}, last error: {last_err}"
    ))
}

#[test]
fn test_fs_storage_atomic_and_bounded() {
    let dir = TempDir::new("test_fs_store");
    let path = dir.path().join("nested/storage.json");
    let backend = FsStorageBackend::new(path.clone());

    // Initially none
    assert_eq!(backend.load().unwrap(), None);

    // Atomic store
    let sample = r#"{"hello":"world"}"#;
    assert!(backend.store(sample).is_ok());
    assert_eq!(backend.load().unwrap().as_deref(), Some(sample));

    // Atomic overwrite
    let sample2 = r#"{"hello":"updated"}"#;
    assert!(backend.store(sample2).is_ok());
    assert_eq!(backend.load().unwrap().as_deref(), Some(sample2));

    // Bounded load check: file exceeding MAX_STORAGE_LOAD_BYTES is rejected
    let large_path = dir.path().join("oversized.json");
    let large_file = fs::File::create(&large_path).unwrap();
    large_file.set_len(MAX_STORAGE_LOAD_BYTES + 512).unwrap();
    drop(large_file);

    let large_backend = FsStorageBackend::new(large_path);
    let err = large_backend.load().unwrap_err();
    assert!(err.contains("exceeds maximum size bound"));
}

#[test]
fn test_state_store_schema_normalization_and_restore_boot_off() {
    let snapshot_from_hub = json!({
        "state": "ON",
        "brightness": 200,
        "color_mode": "rgb",
        "color": { "r": 255, "g": 128, "b": 64 },
        "effect": "Rainbow",
        "effect_id": "rainbow",
        "text": "PERSIST_TEST",
        "speed": 45,
        "cycle": "ON",
        "rssi": -42,
        "audio_source": "Synthetic",
        "audio_status": "Synthetic",
        "mic": {
            "gainDb": 12.0,
            "squelchDb": -45.0,
            "beatThreshold": 0.35,
            "attackMs": 15,
            "decayMs": 120,
            "agc": true,
            "rightChannel": false
        },
        "effects": {
            "solid": { "brightness": 180 },
            "fire": { "cooling": 55, "sparking": 120 }
        }
    });

    let normalized = normalize_runtime_state(&snapshot_from_hub);

    // Transient fields omitted
    assert!(normalized.get("state").is_none());
    assert!(normalized.get("rssi").is_none());
    assert!(normalized.get("audio_status").is_none());
    assert!(normalized.get("color_mode").is_none());
    assert!(normalized.get("effect_id").is_none());

    // Schema mapped correctly to core restore schema
    assert_eq!(normalized["audio"], "Synthetic");
    assert_eq!(normalized["autoCycle"], true);
    assert_eq!(normalized["brightness"], 200);
    assert_eq!(normalized["text"], "PERSIST_TEST");
    assert_eq!(normalized["speed"], 45);
    assert_eq!(normalized["params"]["solid"]["brightness"], 180.0);
    assert_eq!(normalized["params"]["fire"]["cooling"], 55.0);

    // Test restoring into core engine: MUST BE BOOT OFF
    let effects = matrix_effects::builtin_effects();
    let mut engine = Engine::new(effects).unwrap();

    let dir = TempDir::new("test_state_store");
    let state_file = dir.path().join("state.json");
    let store = StateStore::new(Some(state_file));
    store.save(&snapshot_from_hub).unwrap();

    store.restore(&mut engine).unwrap();

    assert!(!engine.is_on(), "Engine MUST boot OFF after restore!");
    assert_eq!(engine.brightness(), 200);
    assert_eq!(engine.text(), "PERSIST_TEST");
    assert_eq!(engine.speed(), 45);
    assert_eq!(engine.audio_mode(), matrix_core::AudioMode::Synthetic);
    assert!(engine.auto_cycle_ms() > 0);
}

#[test]
fn test_simulator_cli_malformed_inputs_fail_clearly() {
    let bin_path = env!("CARGO_BIN_EXE_matrix-simulator");
    let dir = TempDir::new("test_cli_malformed");

    // 1. Malformed config JSON
    let bad_config = dir.path().join("bad_config.json");
    fs::write(&bad_config, "not valid json").unwrap();
    let status = Command::new(bin_path)
        .arg("--config")
        .arg(&bad_config)
        .output()
        .expect("run binary");
    assert!(!status.status.success(), "malformed config should fail CLI");

    // 2. Malformed state JSON
    let bad_state = dir.path().join("bad_state.json");
    fs::write(&bad_state, "{ unquoted: broken }").unwrap();
    let status = Command::new(bin_path)
        .arg("--state")
        .arg(&bad_state)
        .output()
        .expect("run binary");
    assert!(!status.status.success(), "malformed state should fail CLI");

    // 3. Nonexistent animation path
    let status = Command::new(bin_path)
        .arg("--animation")
        .arg(dir.path().join("missing.lfx"))
        .output()
        .expect("run binary");
    assert!(
        !status.status.success(),
        "missing animation should fail CLI"
    );

    // 4. Invalid animation file content
    let bad_anim = dir.path().join("corrupted.lfx");
    fs::write(&bad_anim, b"NOT_LFX_MAGIC_HEADER_BYTES").unwrap();
    let status = Command::new(bin_path)
        .arg("--animation")
        .arg(&bad_anim)
        .output()
        .expect("run binary");
    assert!(
        !status.status.success(),
        "corrupted animation should fail CLI"
    );
}

#[test]
fn test_simulator_save_restart_persistence_and_frame_playback() {
    let bin_path = env!("CARGO_BIN_EXE_matrix-simulator");
    let dir = TempDir::new("test_simulator_restart");
    let state_file = dir.path().join("runtime_state.json");
    let config_file = dir.path().join("panel_config.json");
    let anim_file = dir.path().join("animation.lfx");

    create_test_lfx_file(&anim_file);

    let config_json = json!({
        "matrixWidth": 2,
        "matrixHeight": 2,
        "panelOrder": [0, 1, 2, 3],
        "panelRotation": [0, 0, 0, 0],
        "serpentine": [false, false, false, false],
        "startCorner": 0,
        "panelLayout": 0,
        "panelSerpentine": false,
        "defaultAnimation": "Rainbow",
        "ledBrightness": 150,
        "autoCycleMs": 0,
        "fsAnimationPath": anim_file.to_str().unwrap()
    });
    fs::write(
        &config_file,
        serde_json::to_string_pretty(&config_json).unwrap(),
    )
    .unwrap();

    let port1 = get_free_port();
    let addr1 = format!("127.0.0.1:{port1}");

    // Phase 1: Launch simulator first time
    let mut child1 = Command::new(bin_path)
        .arg("--listen")
        .arg(&addr1)
        .arg("--config")
        .arg(&config_file)
        .arg("--state")
        .arg(&state_file)
        .arg("--animation")
        .arg(&anim_file)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn simulator 1");
    wait_for_ready(&addr1, &mut child1, Duration::from_secs(5)).expect("simulator 1 ready");
    let mut guard1 = ProcessGuard::new(child1);

    // Check initial state: boots OFF
    let (code, state_val) = http_get_json(&addr1, "/api/state").unwrap();
    assert_eq!(code, 200);
    assert_eq!(state_val["state"], "OFF");

    let (c, _) = http_post_json(
        &addr1,
        "/api/command",
        &json!({ "kind": "text", "payload": "RESTART_VERIFIED" }),
    )
    .unwrap();
    assert_eq!(c, 200);

    let (c, _) = http_post_json(
        &addr1,
        "/api/command",
        &json!({ "kind": "speed", "payload": "42" }),
    )
    .unwrap();
    assert_eq!(c, 200);

    // Set effect to Frames after text (since text selects TextScroll)
    let (c, _) = http_post_json(
        &addr1,
        "/api/command",
        &json!({ "kind": "light", "payload": "{\"state\":\"ON\",\"brightness\":77,\"effect\":\"Frames\"}" }),
    )
    .unwrap();
    assert_eq!(c, 200);

    // Enable cycle after light (since switching effect stops cycle)
    let (c, _) = http_post_json(
        &addr1,
        "/api/command",
        &json!({ "kind": "cycle", "payload": "ON" }),
    )
    .unwrap();
    assert_eq!(c, 200);

    let (c, _) = http_post_json(
        &addr1,
        "/api/command",
        &json!({ "kind": "audio_source", "payload": "Synthetic" }),
    )
    .unwrap();
    assert_eq!(c, 200);

    // Give render thread time to render a frame
    std::thread::sleep(Duration::from_millis(300));
    let (_, curr_state) = http_get_json(&addr1, "/api/state").unwrap();
    println!("PHASE 1 STATE BEFORE FRAME: {curr_state:?}");

    // Verify GET /api/frame returns valid binary frame matching LFX asset (frame 0 or frame 1)
    let (c, frame_bytes) = http_request(&addr1, "GET", "/api/frame", None).unwrap();
    assert_eq!(c, 200);
    assert_eq!(frame_bytes.len(), 3072);
    let is_frame0 = frame_bytes[0] == 210 && frame_bytes[1] == 42 && frame_bytes[2] == 99;
    let is_frame1 = frame_bytes[0] == 11 && frame_bytes[1] == 22 && frame_bytes[2] == 33;
    assert!(
        is_frame0 || is_frame1,
        "frame must match LFX asset frame0 or frame1, got RGB({}, {}, {})",
        frame_bytes[0],
        frame_bytes[1],
        frame_bytes[2]
    );

    // Save state via API
    let (c, save_resp) = http_post_json(&addr1, "/api/state/save", &json!({})).unwrap();
    assert_eq!(c, 200);
    assert_eq!(save_resp["ok"], true);

    // Verify file written to disk
    assert!(state_file.exists());
    let saved_disk_content = fs::read_to_string(&state_file).unwrap();
    let saved_disk_json: serde_json::Value = serde_json::from_str(&saved_disk_content).unwrap();
    // Power state is NOT stored
    assert!(saved_disk_json.get("state").is_none());
    assert_eq!(saved_disk_json["brightness"], 77);
    assert_eq!(saved_disk_json["effect"], "Frames");
    assert_eq!(saved_disk_json["text"], "RESTART_VERIFIED");
    assert_eq!(saved_disk_json["speed"], 42);
    assert_eq!(saved_disk_json["autoCycle"], true);
    assert_eq!(saved_disk_json["audio"], "Synthetic");

    // Clean shutdown of process 1
    guard1.stop();

    // Phase 2: Restart simulator with the same state file on a new port
    let port2 = get_free_port();
    let addr2 = format!("127.0.0.1:{port2}");

    let mut child2 = Command::new(bin_path)
        .arg("--listen")
        .arg(&addr2)
        .arg("--config")
        .arg(&config_file)
        .arg("--state")
        .arg(&state_file)
        .arg("--animation")
        .arg(&anim_file)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn simulator 2");
    wait_for_ready(&addr2, &mut child2, Duration::from_secs(5)).expect("simulator 2 ready");
    let mut guard2 = ProcessGuard::new(child2);

    // Query restored state
    let (code, restored_state) = http_get_json(&addr2, "/api/state").unwrap();
    assert_eq!(code, 200);
    println!("RESTORED STATE: {restored_state:?}");

    // CRITICAL: Display MUST boot OFF!
    assert_eq!(restored_state["state"], "OFF");

    // All settings preserved:
    assert_eq!(restored_state["brightness"], 77);
    assert_eq!(restored_state["effect"], "Frames");
    assert_eq!(restored_state["text"], "RESTART_VERIFIED");
    assert_eq!(restored_state["speed"], 42);
    assert_eq!(restored_state["cycle"], "ON");
    assert_eq!(restored_state["audio_source"], "Synthetic");

    // Turn ON and verify frame animation playback resumes
    let (c, _) = http_post_json(
        &addr2,
        "/api/command",
        &json!({ "kind": "light", "payload": "{\"state\":\"ON\"}" }),
    )
    .unwrap();
    assert_eq!(c, 200);

    std::thread::sleep(Duration::from_millis(150));
    let (c, frame_bytes2) = http_request(&addr2, "GET", "/api/frame", None).unwrap();
    assert_eq!(c, 200);
    assert_eq!(frame_bytes2.len(), 3072);
    let is_frame0 = frame_bytes2[0] == 210 && frame_bytes2[1] == 42 && frame_bytes2[2] == 99;
    let is_frame1 = frame_bytes2[0] == 11 && frame_bytes2[1] == 22 && frame_bytes2[2] == 33;
    assert!(
        is_frame0 || is_frame1,
        "frame after restart must match LFX asset frame0 or frame1, got RGB({}, {}, {})",
        frame_bytes2[0],
        frame_bytes2[1],
        frame_bytes2[2]
    );

    guard2.stop();
}
