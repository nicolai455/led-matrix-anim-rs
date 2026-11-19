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
            &serde_json::json!({"state": "ON", "effect": effect_id, "brightness": 255}).to_string(),
        )
        .unwrap_or_else(|error| panic!("effect '{effect_id}' not selectable: {error}"));
    engine
}

/// Audio snapshots that a broken DSP, a corrupt restore, or a divide-by-zero
/// can produce.
fn hostile_audio() -> Vec<(&'static str, AudioSnapshot)> {
    let cases = vec![
        (
            "all-nan",
            AudioSnapshot {
                available: true,
                level: f32::NAN,
                peak: f32::NAN,
                bands: [f32::NAN; 16],
                waveform: [f32::NAN; 128],
                ..AudioSnapshot::default()
            },
        ),
        (
            "all-infinite",
            AudioSnapshot {
                available: true,
                level: f32::INFINITY,
                peak: f32::NEG_INFINITY,
                bands: [f32::INFINITY; 16],
                waveform: [f32::NEG_INFINITY; 128],
                ..AudioSnapshot::default()
            },
        ),
        (
            "out-of-range",
            AudioSnapshot {
                available: true,
                level: 1e30,
                peak: -1e30,
                bands: [1e30; 16],
                waveform: [-1e30; 128],
                ..AudioSnapshot::default()
            },
        ),
        (
            "subnormal",
            AudioSnapshot {
                available: true,
                level: f32::MIN_POSITIVE / 2.0,
                peak: f32::MIN_POSITIVE / 2.0,
                bands: [f32::MIN_POSITIVE / 2.0; 16],
                waveform: [f32::MIN_POSITIVE / 2.0; 128],
                ..AudioSnapshot::default()
            },
        ),
        (
            "negative",
            AudioSnapshot {
                available: true,
                level: -1.0,
                peak: -1.0,
                bands: [-1.0; 16],
                waveform: [-1.0; 128],
                ..AudioSnapshot::default()
            },
        ),
        ("unavailable", AudioSnapshot::default()),
    ];
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
                        problems.push(format!(
                            "{id}/{param} accepted {hostile} then rendered non-finite"
                        ));
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
                assert!(
                    frame_is_finite(&frame),
                    "switch to {id} rendered non-finite"
                );
            }
        }
    }
}

#[test]
fn every_effect_survives_an_unclamped_delta_and_stays_within_a_frame_budget() {
    // `Engine::render` clamps delta to 0.5 s, so it can never expose a catch-up
    // loop that replays simulated time. Effects are also driven directly (preview
    // renderer, matrix-gif, tests), so the unclamped contract needs its own guard.
    // `game_of_life` used to step once per elapsed generation with no cap: a
    // resumed host (delta ~1e9) stalled the render thread for minutes in a debug
    // build. A real frame budget is the assertion that catches that class.
    use matrix_core::{RenderContext, Rgb};
    use std::time::{Duration, Instant};

    const BUDGET: Duration = Duration::from_millis(400);
    let mut slow = Vec::new();
    let mut nonfinite = Vec::new();

    for effect in matrix_effects::builtin_effects().iter_mut() {
        let id = effect.id().to_string();
        let audio = hostile_audio()
            .into_iter()
            .next()
            .expect("a hostile case")
            .1;
        let started = Instant::now();
        for frame_idx in 0..40u64 {
            // Alternate an exploding clock with a stuck one.
            let now_ms = if frame_idx % 2 == 0 {
                frame_idx * (u64::MAX / 2 / 40)
            } else {
                u64::MAX / 2
            };
            let context = RenderContext {
                now_ms,
                delta_seconds: 1e9,
                audio: &audio,
                color: Rgb::new(200, 100, 50),
                text: "HELLO WORLD",
                scroll_speed: 30.0,
            };
            let mut frame = Frame::new();
            effect.render(&mut frame, &context);
            if !frame_is_finite(&frame) {
                nonfinite.push(id.clone());
                break;
            }
        }
        let elapsed = started.elapsed();
        if elapsed > BUDGET {
            slow.push(format!("{id} took {elapsed:?}"));
        }
    }

    assert!(
        nonfinite.is_empty(),
        "non-finite frames from: {nonfinite:?}"
    );
    assert!(
        slow.is_empty(),
        "effects exceeded a {BUDGET:?} budget for 40 frames at delta=1e9:\n{}",
        slow.join("\n")
    );
}

fn frame_is_finite(frame: &Frame) -> bool {
    // Rgb is u8, so a frame is always finite; the real check is that the
    // effect did not panic before returning one. Keep the hook so a future
    // float-backed frame type cannot regress silently.
    frame.pixels.len() == WIDTH * HEIGHT
}
