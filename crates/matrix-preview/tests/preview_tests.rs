use matrix_preview::{
    preview_audio_available, preview_audio_bands, preview_audio_beat, preview_audio_level,
    preview_audio_pcm, preview_audio_peak, preview_input, preview_input_capacity, preview_render,
    preview_sync,
};
use serde_json::json;
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn test_capacity_and_buffer_pointer() {
    let _guard = TEST_LOCK.lock().unwrap();
    let cap = preview_input_capacity();
    assert!(
        cap >= 16384,
        "capacity must be at least 16384 bytes, got {}",
        cap
    );

    let ptr = preview_input();
    assert!(!ptr.is_null(), "preview_input pointer must not be null");
}

fn sync_json(val: &serde_json::Value) -> i32 {
    let bytes = serde_json::to_vec(val).unwrap();
    let cap = preview_input_capacity();
    assert!(
        bytes.len() <= cap,
        "payload size {} exceeds capacity {}",
        bytes.len(),
        cap
    );
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), preview_input(), bytes.len());
    }
    preview_sync(bytes.len())
}

#[test]
fn test_sync_valid_state_and_render_power_off() {
    let _guard = TEST_LOCK.lock().unwrap();
    let state = json!({
        "state": "OFF",
        "brightness": 200,
        "color": { "r": 255, "g": 128, "b": 64 },
        "effect": "Solid",
        "effect_id": "Solid",
        "speed": 50,
        "text": "Antigravity",
        "audio_source": "Off"
    });

    let status = sync_json(&state);
    assert_eq!(status, 0, "preview_sync should succeed");

    let ptr = preview_render(1000.0);
    assert!(!ptr.is_null());
    let output = unsafe { std::slice::from_raw_parts(ptr, 4096) };

    // When power is OFF, all RGB channels must be 0 and alpha 255
    for chunk in output.chunks_exact(4) {
        assert_eq!(chunk[0], 0, "R should be 0 when OFF");
        assert_eq!(chunk[1], 0, "G should be 0 when OFF");
        assert_eq!(chunk[2], 0, "B should be 0 when OFF");
        assert_eq!(chunk[3], 255, "Alpha should be 255");
    }
}

#[test]
fn test_sync_power_on_brightness_scaling() {
    let _guard = TEST_LOCK.lock().unwrap();
    let state = json!({
        "state": "ON",
        "brightness": 128, // approx 50%
        "color": { "r": 200, "g": 100, "b": 50 },
        "effect": "Solid",
        "effect_id": "Solid",
        "speed": 50,
        "text": "",
        "audio_source": "Off"
    });

    let status = sync_json(&state);
    assert_eq!(status, 0);

    let ptr = preview_render(1000.0);
    let output = unsafe { std::slice::from_raw_parts(ptr, 4096) };

    // 200 * 128 / 255 = ~100
    // 100 * 128 / 255 = ~50
    // 50 * 128 / 255 = ~25
    let r = output[0];
    let g = output[1];
    let b = output[2];
    let a = output[3];

    assert!((98..=102).contains(&r), "R scaled brightness: got {}", r);
    assert!((48..=52).contains(&g), "G scaled brightness: got {}", g);
    assert!((23..=27).contains(&b), "B scaled brightness: got {}", b);
    assert_eq!(a, 255);
}

#[test]
fn test_unknown_or_frame_effect_returns_nonzero_status() {
    let _guard = TEST_LOCK.lock().unwrap();
    // Unknown effect ID
    let state_unknown = json!({
        "state": "ON",
        "brightness": 255,
        "effect": "NonExistentEffectXYZ",
        "effect_id": "non_existent_effect_xyz"
    });
    let status = sync_json(&state_unknown);
    assert_ne!(
        status, 0,
        "Unknown effect must return non-zero for fallback"
    );

    // Frames / FramePlayer effect fallback
    let state_frames = json!({
        "state": "ON",
        "brightness": 255,
        "effect": "Frames",
        "effect_id": "Frames"
    });
    let status_frames = sync_json(&state_frames);
    assert_ne!(
        status_frames, 0,
        "Frames effect must return non-zero for frontend device fallback"
    );

    let state_frame_player = json!({
        "state": "ON",
        "brightness": 255,
        "effect": "FramePlayer",
        "effect_id": "FramePlayer"
    });
    let status_frame_player = sync_json(&state_frame_player);
    assert_ne!(
        status_frame_player, 0,
        "FramePlayer effect must return non-zero for frontend device fallback"
    );
}

#[test]
fn test_preview_audio_telemetry_and_pcm_export() {
    let _guard = TEST_LOCK.lock().unwrap();
    let state = json!({
        "state": "ON",
        "brightness": 200,
        "effect": "Frequency Bars",
        "effect_id": "freq_bars",
        "audio_source": "Synthetic"
    });
    let status = sync_json(&state);
    assert_eq!(status, 0);

    let _ptr = preview_render(1000.0);
    assert_eq!(preview_audio_available(), 1);
    let level = preview_audio_level();
    let peak = preview_audio_peak();
    let _beat = preview_audio_beat();
    assert!((0.0..=1.0).contains(&level));
    assert!((0.0..=1.0).contains(&peak));

    let bands_ptr = preview_audio_bands();
    assert!(!bands_ptr.is_null());
    let bands = unsafe { std::slice::from_raw_parts(bands_ptr, 16) };
    for &b in bands {
        assert!((0.0..=1.0).contains(&b));
    }

    let pcm_ptr = preview_audio_pcm(1.0, 512, 22050.0);
    assert!(!pcm_ptr.is_null());
    let pcm = unsafe { std::slice::from_raw_parts(pcm_ptr, 512) };
    assert_eq!(pcm.len(), 512);
    for &s in pcm {
        assert!((-1.0..=1.0).contains(&s));
    }
}
