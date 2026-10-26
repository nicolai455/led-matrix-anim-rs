pub use matrix_core::audio::MicSettings;
use serde_json::{json, Value};
use std::sync::RwLock;

/// Shared microphone state and telemetry controller.
pub struct MicController {
    available: bool,
    metrics: RwLock<Option<(f32, f32)>>, // (raw_dbfs, applied_gain_db)
}

impl MicController {
    pub fn new(available: bool) -> Self {
        Self {
            available,
            metrics: RwLock::new(None),
        }
    }

    pub fn is_available(&self) -> bool {
        self.available
    }

    pub fn set_metrics(&self, raw_dbfs: f32, applied_gain_db: f32) {
        if let Ok(mut lock) = self.metrics.write() {
            *lock = Some((raw_dbfs, applied_gain_db));
        }
    }

    pub fn to_json(&self, active: bool, settings: &MicSettings, snapshot: Option<&Value>) -> Value {
        if !self.available {
            return json!({
                "type": "mic",
                "available": false,
                "active": false,
                "rawDbfs": null,
                "gainDb": null,
                "level": 0.0,
                "bass": 0.0,
                "mid": 0.0,
                "treble": 0.0,
                "beats": 0,
                "bands": vec![0.0; 16],
                "settings": {
                    "gainDb": settings.gain_db,
                    "squelchDb": settings.squelch_db,
                    "beatThreshold": settings.beat_threshold,
                    "attackMs": settings.attack_ms,
                    "decayMs": settings.decay_ms,
                    "agc": settings.agc,
                    "rightChannel": settings.right_channel,
                }
            });
        }

        let (raw, gain) = self
            .metrics
            .read()
            .ok()
            .and_then(|m| *m)
            .unwrap_or((-180.0, settings.gain_db));

        let bands = snapshot
            .and_then(|s| s.get("bands"))
            .cloned()
            .unwrap_or_else(|| json!(vec![0.0; 16]));

        let level = snapshot
            .and_then(|s| s.get("level"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let bass = snapshot
            .and_then(|s| s.get("bass"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let mid = snapshot
            .and_then(|s| s.get("mid"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let treble = snapshot
            .and_then(|s| s.get("treble"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        json!({
            "type": "mic",
            "available": true,
            "active": active,
            "rawDbfs": raw,
            "gainDb": gain,
            "level": level,
            "bass": bass,
            "mid": mid,
            "treble": treble,
            "beats": 0,
            "bands": bands,
            "settings": {
                "gainDb": settings.gain_db,
                "squelchDb": settings.squelch_db,
                "beatThreshold": settings.beat_threshold,
                "attackMs": settings.attack_ms,
                "decayMs": settings.decay_ms,
                "agc": settings.agc,
                "rightChannel": settings.right_channel,
            }
        })
    }
}
