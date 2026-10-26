//! Shared effect engine compiled to WebAssembly for approximate browser previews.
use matrix_core::{synthetic_audio, AudioMode, AudioSnapshot, Engine};
#[cfg(test)]
use serde_json::json;
use serde_json::Value;
use std::sync::Mutex;

pub const INPUT_CAPACITY: usize = 65536;
const OUTPUT_BYTES: usize = 4096;
struct Preview {
    engine: Engine,
    input: Box<[u8; INPUT_CAPACITY]>,
    output: Box<[u8; OUTPUT_BYTES]>,
    audio: AudioSnapshot,
    current_audio: AudioSnapshot,
    pcm_buffer: Box<[f32; 2048]>,
    audio_time: u64,
    consumed_beat: Option<u64>,
}
impl Preview {
    fn new() -> Self {
        Self {
            engine: Engine::new(matrix_effects::builtin_effects()).expect("builtin effects"),
            input: Box::new([0; INPUT_CAPACITY]),
            output: Box::new([0; OUTPUT_BYTES]),
            audio: AudioSnapshot::default(),
            current_audio: AudioSnapshot::default(),
            pcm_buffer: Box::new([0.0; 2048]),
            audio_time: 0,
            consumed_beat: None,
        }
    }
    fn sync(&mut self, s: Value) -> Result<(), String> {
        let current = self.engine.snapshot();
        let id = s
            .get("effect_id")
            .and_then(Value::as_str)
            .or_else(|| s.get("effect").and_then(Value::as_str))
            .unwrap_or("Rainbow");
        if id == "Frames"
            || !self
                .engine
                .effects()
                .iter()
                .any(|e| e.id() == id || e.name() == id)
        {
            return Err("unsupported effect".into());
        }
        let mut light = serde_json::Map::new();
        for key in ["state", "brightness", "color", "effect"] {
            if let Some(value) = s.get(key) {
                if current.get(key) != Some(value) {
                    light.insert(key.into(), value.clone());
                }
            }
        }
        if !light.is_empty() {
            self.engine
                .apply("light", &Value::Object(light).to_string())?;
        }
        for key in ["text", "speed", "audio_source"] {
            if let Some(value) = s.get(key) {
                if current.get(key) != Some(value) {
                    let payload = value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string());
                    self.engine.apply(key, &payload)?;
                }
            }
        }
        // Text commands select TextScroll and color commands select Solid.
        // Restore the authoritative selection after applying those legacy commands.
        if self.engine.current_effect().id() != id && self.engine.current_effect().name() != id {
            self.engine
                .apply("light", &serde_json::json!({"effect": id}).to_string())?;
        }
        if let Some(effects) = s.get("effects").and_then(Value::as_object) {
            for (effect, params) in effects {
                if let Some(params) = params.as_object() {
                    for (param, value) in params {
                        if current["effects"][effect][param] != *value {
                            let payload = value
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| value.to_string());
                            self.engine
                                .apply(&format!("effect/{effect}/{param}"), &payload)?;
                        }
                    }
                }
            }
        }
        // Device-selected effects are authoritative; never auto-cycle independently.
        self.audio = AudioSnapshot::default();
        self.audio.available = s["audio_status"] == "External";
        self.audio.level = s["level"].as_f64().unwrap_or(0.0) as f32;
        self.audio.peak = s["peak"].as_f64().unwrap_or(0.0) as f32;
        self.audio.beat = s["beat"].as_bool().unwrap_or(false);
        for (key, target, divisor) in [
            ("bands", &mut self.audio.bands[..], 1.0),
            ("waveform", &mut self.audio.waveform[..], 127.0),
        ] {
            if let Some(values) = s[key].as_array() {
                for (dst, src) in target.iter_mut().zip(values) {
                    *dst = src.as_f64().unwrap_or(0.0) as f32 / divisor;
                }
            }
        }
        self.audio_time = s["renderTimeMs"].as_u64().unwrap_or(0);
        Ok(())
    }
}
static STATE: Mutex<Option<Preview>> = Mutex::new(None);
#[no_mangle]
pub extern "C" fn preview_input_capacity() -> usize {
    INPUT_CAPACITY
}
#[no_mangle]
pub extern "C" fn preview_input() -> *mut u8 {
    STATE
        .lock()
        .unwrap()
        .get_or_insert_with(Preview::new)
        .input
        .as_mut_ptr()
}
#[no_mangle]
pub extern "C" fn preview_sync(len: usize) -> i32 {
    if len > INPUT_CAPACITY {
        return 1;
    }
    let mut state = STATE.lock().unwrap();
    let p = state.get_or_insert_with(Preview::new);
    let value = match serde_json::from_slice::<Value>(&p.input[..len]) {
        Ok(v) if v.is_object() => v,
        _ => return 1,
    };
    if p.sync(value).is_ok() {
        0
    } else {
        2
    }
}
#[no_mangle]
pub extern "C" fn preview_render(now_ms: f64) -> *const u8 {
    let mut state = STATE.lock().unwrap();
    let p = state.get_or_insert_with(Preview::new);
    let now = now_ms.max(0.0) as u64;
    let mut audio = match p.engine.audio_mode() {
        AudioMode::Synthetic => synthetic_audio(now),
        AudioMode::External => {
            let mut a = p.audio.clone();
            a.apply_staleness(now.saturating_sub(p.audio_time));
            if p.consumed_beat == Some(p.audio_time) {
                a.beat = false;
            }
            p.consumed_beat = Some(p.audio_time);
            a
        }
        AudioMode::Off => AudioSnapshot::default(),
    };
    if !audio.available {
        audio.beat = false;
    }
    p.current_audio = audio.clone();
    let frame = if p.engine.is_on() {
        p.engine.render(now, &audio)
    } else {
        Default::default()
    };
    let brightness = p.engine.brightness() as u16;
    for (pixel, out) in frame.pixels.iter().zip(p.output.chunks_exact_mut(4)) {
        out.copy_from_slice(&[
            (pixel.r as u16 * brightness / 255) as u8,
            (pixel.g as u16 * brightness / 255) as u8,
            (pixel.b as u16 * brightness / 255) as u8,
            255,
        ]);
    }
    p.output.as_ptr()
}

#[no_mangle]
pub extern "C" fn preview_audio_bands() -> *const f32 {
    let mut state = STATE.lock().unwrap();
    let p = state.get_or_insert_with(Preview::new);
    p.current_audio.bands.as_ptr()
}

#[no_mangle]
pub extern "C" fn preview_audio_level() -> f32 {
    let mut state = STATE.lock().unwrap();
    let p = state.get_or_insert_with(Preview::new);
    p.current_audio.level
}

#[no_mangle]
pub extern "C" fn preview_audio_peak() -> f32 {
    let mut state = STATE.lock().unwrap();
    let p = state.get_or_insert_with(Preview::new);
    p.current_audio.peak
}

#[no_mangle]
pub extern "C" fn preview_audio_beat() -> i32 {
    let mut state = STATE.lock().unwrap();
    let p = state.get_or_insert_with(Preview::new);
    if p.current_audio.beat {
        1
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn preview_audio_available() -> i32 {
    let mut state = STATE.lock().unwrap();
    let p = state.get_or_insert_with(Preview::new);
    if p.current_audio.available {
        1
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn preview_audio_pcm(
    start_sec: f64,
    num_samples: usize,
    sample_rate: f64,
) -> *const f32 {
    let mut state = STATE.lock().unwrap();
    let p = state.get_or_insert_with(Preview::new);
    let count = num_samples.min(2048);
    let sr = if sample_rate > 1000.0 {
        sample_rate
    } else {
        22050.0
    };
    for i in 0..count {
        let t = (start_sec + (i as f64) / sr) as f32;
        p.pcm_buffer[i] = matrix_core::synthetic_audio_sample(t);
    }
    p.pcm_buffer.as_ptr()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_effects_sync_and_render_without_restarting_settings() {
        let mut p = Preview::new();
        let catalog = p.engine.catalog();
        for effect in catalog.as_array().unwrap() {
            let id = effect["id"].as_str().unwrap();
            if id == "Frames" {
                continue;
            }
            let mut state = p.engine.snapshot();
            state["effect_id"] = json!(id);
            state["effect"] = effect["name"].clone();
            state["state"] = json!("ON");
            p.sync(state.clone()).unwrap();
            assert_eq!(p.engine.current_effect().id(), id);
            for tick in 0..3 {
                p.engine.render(tick * 17, &AudioSnapshot::default());
            }
            p.sync(state).unwrap();
        }
        assert!(p.sync(json!({"effect_id":"Frames"})).is_err());
    }
}
