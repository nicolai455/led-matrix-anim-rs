//! Engine construction, effect discovery and frame capture helpers.

use matrix_core::Engine;
use matrix_effects::builtin_effects;

use crate::model::{AudioChoice, EffectInfo};

/// Warmup frames rendered (and discarded) before recording starts. Effects that
/// build state over time (fire, rain, particles) look empty without them.
pub const WARMUP_FRAMES: u64 = 60;

/// Timestamp in milliseconds for frame `index` at `fps`.
#[inline]
pub fn frame_time_ms(index: u64, fps: u32) -> u64 {
    // Integer math keeps runs bit-for-bit reproducible across machines.
    index.saturating_mul(1000) / u64::from(fps)
}

/// Number of frames needed for `seconds` at `fps` (at least one).
#[inline]
pub fn frame_count(seconds: f64, fps: u32) -> u64 {
    let count = (seconds.max(0.0) * f64::from(fps)).round() as u64;
    count.max(1)
}

/// Builds an engine containing every built-in effect.
pub fn new_engine() -> Result<Engine, String> {
    Engine::new(builtin_effects())
}

/// Reads the effect catalog from a fresh engine.
pub fn discover_effects(engine: &Engine) -> Result<Vec<EffectInfo>, String> {
    let catalog = engine.catalog();
    let entries = catalog
        .as_array()
        .ok_or_else(|| "engine catalog is not a JSON array".to_string())?;

    let mut effects = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let id = entry
            .get("id")
            .and_then(|value| value.as_str())
            .ok_or_else(|| format!("catalog entry {index} has no string 'id'"))?
            .to_string();
        let name = entry
            .get("name")
            .and_then(|value| value.as_str())
            .ok_or_else(|| format!("catalog entry {index} ('{id}') has no string 'name'"))?
            .to_string();
        let parameters = entry
            .get("parameters")
            .and_then(|value| value.as_array())
            .ok_or_else(|| format!("catalog entry {index} ('{id}') has no 'parameters' array"))?;
        let audio_reactive = parameters.iter().any(|parameter| {
            parameter
                .get("id")
                .and_then(|value| value.as_str())
                .is_some_and(|pid| pid == "audio_reactive")
        });

        effects.push(EffectInfo {
            index,
            id,
            name,
            parameter_count: parameters.len(),
            audio_reactive,
        });
    }

    if effects.is_empty() {
        return Err("engine catalog is empty".to_string());
    }
    Ok(effects)
}

/// Selects a single effect and turns the light on at full brightness.
pub fn activate(engine: &mut Engine, id: &str, audio: AudioChoice) -> Result<(), String> {
    engine.apply("audio_source", audio.engine_source())?;
    let payload = serde_json::json!({
        "state": "ON",
        "effect": id,
        "brightness": 255,
    })
    .to_string();
    engine.apply("light", &payload)?;
    if !engine.is_on() {
        return Err(format!("engine did not turn on for effect '{id}'"));
    }
    Ok(())
}

/// Renders `WARMUP_FRAMES` frames so time-based effects reach a steady state.
/// Warmup starts at t=0 and advances with the same cadence as the recording.
pub fn warmup(engine: &mut Engine, fps: u32, audio: AudioChoice) {
    for index in 0..WARMUP_FRAMES {
        let now_ms = frame_time_ms(index, fps);
        let snapshot = audio.snapshot(now_ms);
        let _ = engine.render(now_ms, &snapshot);
    }
}
