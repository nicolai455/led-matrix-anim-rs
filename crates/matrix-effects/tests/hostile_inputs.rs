//! Adversarial render harness: every effect must survive hostile inputs.
//!
//! These are the inputs an effect never sees in normal operation but can see
//! through a corrupt snapshot, a restored state file, a NaN from the audio
//! pipeline, or the first frame after a long suspend. Nothing here may panic,
//! hang, or write non-finite colours.

use matrix_core::{AudioSnapshot, Engine, Frame, HEIGHT, WIDTH};

fn engine_with(effect_id: &str) -> Engine {
    let mut engine = Engine::new(matrix_effects::builtin_effects()).expect("engine builds");
    engine
        .apply(
            "light",
            &serde_json::json!({"state": "ON", "effect": effect_id, "brightness": 255})
                .to_string(),
        )
        .unwrap_or_else(|error| panic!("effect '{effect_id}' not selectable: {error}"));
    engine
}

/// Audio snapshots that a broken DSP, a corrupt restore, or a divide-by-zero
/// can produce.
fn hostile_audio() -> Vec<(&'static str, AudioSnapshot)> {
    let mut cases = Vec::new();

    let mut nan = AudioSnapshot::default();
    nan.available = true;
    nan.level = f32::NAN;
    nan.peak = f32::NAN;
    nan.bands = [f32::NAN; 16];
    nan.waveform = [f32::NAN; 128];
    cases.push(("all-nan", nan));

    let mut infinite = AudioSnapshot::default();
    infinite.available = true;
    infinite.level = f32::INFINITY;
    infinite.peak = f32::NEG_INFINITY;
    infinite.bands = [f32::INFINITY; 16];
    infinite.waveform = [f32::NEG_INFINITY; 128];
    cases.push(("all-infinite", infinite));

    let mut out_of_range = AudioSnapshot::default();
    out_of_range.available = true;
    out_of_range.level = 1e30;
    out_of_range.peak = -1e30;
    out_of_range.bands = [1e30; 16];
    out_of_range.waveform = [-1e30; 128];
    cases.push(("out-of-range", out_of_range));

    let mut subnormal = AudioSnapshot::default();
    subnormal.available = true;
    subnormal.level = f32::MIN_POSITIVE / 2.0;
    subnormal.peak = f32::MIN_POSITIVE / 2.0;
    subnormal.bands = [f32::MIN_POSITIVE / 2.0; 16];
    subnormal.waveform = [f32::MIN_POSITIVE / 2.0; 128];
    cases.push(("subnormal", subnormal));

    let mut negative = AudioSnapshot::default();
    negative.available = true;
    negative.level = -1.0;
    negative.peak = -1.0;
    negative.bands = [-1.0; 16];
    negative.waveform = [-1.0; 128];
    cases.push(("negative", negative));

    cases.push(("unavailable", AudioSnapshot::default()));
    cases
}

#[test]
fn every_effect_survives_hostile_audio() {
    let mut failures = Vec::new();
    for effect in matrix_effects::builtin_effects() {
        let id = effect.id();
        let mut engine = engine_with(id);
        for (case, audio) in hostile_audio() {
            for tick in 0..40u64 {
                let frame = engine.render(tick * 17, &audio);
                if !frame_is_finite(&frame) {
                    failures.push(format!("{id} [{case}] produced non-finite pixels"));
                    break;
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "effects broke on hostile audio:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_effect_survives_extreme_time_steps() {
    let mut failures = Vec::new();
    let audio = AudioSnapshot::default();
    let timeline: [u64; 10] = [
        0,
        1,
        u64::MAX / 4,
        u64::MAX / 2,
        u64::MAX - 1,
        u64::MAX,
        0, // clock jumped backwards (NTP correction, suspend/resume)
        5,
        60 * 60 * 1000,
        24 * 60 * 60 * 1000,
    ];

    for effect in matrix_effects::builtin_effects() {
        let id = effect.id();
        let mut engine = engine_with(id);
        for &now in &timeline {
            for _ in 0..5 {
                let frame = engine.render(now, &audio);
                if !frame_is_finite(&frame) {
                    failures.push(format!("{id} at now_ms={now} produced non-finite pixels"));
                    break;
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "effects broke on extreme time steps:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_effect_parameter_rejects_out_of_range_and_nan() {
    let mut problems = Vec::new();
    for effect in matrix_effects::builtin_effects() {
        let id = effect.id();
        let descriptors: Vec<_> = effect.parameters().iter().map(|p| p.id).collect();
        if descriptors.is_empty() {
            continue;
        }
        let mut engine = engine_with(id);
        for param in descriptors {
            for hostile in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1e30, -1e30] {
                let outcome = engine.apply(&format!("effect/{id}/{param}"), &hostile.to_string());
                // Accepting the value is fine only if the effect then renders
                // finite pixels; a panic or non-finite frame is not.
                if outcome.is_ok() {
                    let frame = engine.render(1_000, &AudioSnapshot::default());
                    if !frame_is_finite(&frame) {
                        problems.push(format!("{id}/{param} accepted {hostile} then rendered non-finite"));
                    }
                }
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn hostile_playlists_never_panic_the_render_path() {
    // Effect ids that exist today but could vanish in a slimmer firmware
    // build, plus outright junk.
    let candidate_ids = [
        "__missing__",
        "",
        "Frames",
        "does-not-exist",
        "../../etc/passwd",
        "\u{0}",
        &"a".repeat(200),
    ];

    for id in candidate_ids {
        let mut engine = Engine::new(matrix_effects::builtin_effects()).expect("engine builds");
        let config = serde_json::json!({
            "entries": [{"effectId": id, "durationMs": 1000}],
            "loop": true
        });
        // Rejection is the correct behaviour; the point is that it must not
        // panic and must leave the engine usable.
        let _ = engine.apply("playlist_config", &config.to_string());
        let _ = engine.apply("playlist_control", r#"{"action":"start"}"#);
        for tick in 0..50u64 {
            let frame = engine.render(tick * 20, &AudioSnapshot::default());
            assert!(
                frame_is_finite(&frame),
                "render went non-finite after hostile playlist id {id:?}"
            );
        }
        // The engine must still be able to render the default effect.
        let frame = engine.render(2_000, &AudioSnapshot::default());
        assert!(frame_is_finite(&frame));
    }
}

#[test]
fn rapid_effect_switch_and_reset_stays_stable() {
    let catalog: Vec<String> = matrix_effects::builtin_effects()
        .iter()
        .map(|e| e.id().to_string())
        .collect();
    let mut engine = Engine::new(matrix_effects::builtin_effects()).expect("engine builds");
    let audio = matrix_core::synthetic_audio(1_000);

    for round in 0..3 {
        for id in &catalog {
            let _ = engine.apply("light", &serde_json::json!({"effect": id}).to_string());
            for tick in 0..3u64 {
                let frame = engine.render(round * 10_000 + tick * 16, &audio);
                assert!(frame_is_finite(&frame), "switch to {id} rendered non-finite");
            }
        }
    }
}

fn frame_is_finite(frame: &Frame) -> bool {
    // Rgb is u8, so a frame is always finite; the real check is that the
    // effect did not panic before returning one. Keep the hook so a future
    // float-backed frame type cannot regress silently.
    frame.pixels.len() == WIDTH * HEIGHT
}
