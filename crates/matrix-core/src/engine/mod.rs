use crate::audio::{AudioSnapshot, MicSettings};
use crate::effect::{Effect, ParameterKind, RenderContext};
use crate::pixel::{Frame, Rgb};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioMode {
    #[default]
    Off,
    Synthetic,
    External,
}

pub struct Engine {
    effects: Vec<Box<dyn Effect>>,
    current_effect_idx: usize,
    on: bool,
    brightness: u8,
    color: Rgb,
    text: String,
    speed: u32,
    auto_cycle_ms: u32,
    cycle_interval_ms: u32,
    last_cycle_ms: u64,
    last_render_ms: u64,
    has_rendered: bool,
    audio_mode: AudioMode,
    audio_available: bool,
    mic_settings: MicSettings,
    rssi: i32,
    dirty: bool,
}

impl Engine {
    /// Creates a new Engine with the given effect catalog.
    /// Engine always boots OFF.
    pub fn new(effects: Vec<Box<dyn Effect>>) -> Result<Self, String> {
        if effects.is_empty() {
            return Err("Engine requires at least one effect".to_string());
        }

        let mut seen_ids = std::collections::HashSet::new();
        let mut seen_names = std::collections::HashSet::new();

        for effect in &effects {
            let id = effect.id().trim();
            if id.is_empty() {
                return Err("effect id cannot be empty".to_string());
            }
            let name = effect.name().trim();
            if name.is_empty() {
                return Err("effect name cannot be empty".to_string());
            }
            if !seen_ids.insert(id.to_lowercase()) {
                return Err(format!("duplicate effect ID '{}'", effect.id()));
            }
            if !seen_names.insert(name.to_lowercase()) {
                return Err(format!("duplicate effect name '{}'", effect.name()));
            }
        }

        let mut engine = Self {
            effects,
            current_effect_idx: 0,
            on: false, // Engine boots OFF
            brightness: 128,
            color: Rgb::WHITE,
            text: "HELLO".to_string(),
            speed: 30,
            auto_cycle_ms: 0,
            cycle_interval_ms: 15000,
            last_cycle_ms: 0,
            last_render_ms: 0,
            has_rendered: false,
            audio_mode: AudioMode::Off,
            audio_available: false,
            mic_settings: MicSettings::default(),
            rssi: -60,
            dirty: true,
        };

        if let Some(first) = engine.effects.get_mut(0) {
            first.reset();
        }

        Ok(engine)
    }

    #[inline]
    pub fn is_on(&self) -> bool {
        self.on
    }

    #[inline]
    pub fn brightness(&self) -> u8 {
        self.brightness
    }

    #[inline]
    pub fn color(&self) -> Rgb {
        self.color
    }

    #[inline]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[inline]
    pub fn speed(&self) -> u32 {
        self.speed
    }

    #[inline]
    pub fn auto_cycle_ms(&self) -> u32 {
        self.auto_cycle_ms
    }

    /// Configure the interval used when auto-cycle is enabled. A zero
    /// interval disables cycling without changing its remembered period.
    pub fn set_auto_cycle_interval_ms(&mut self, interval_ms: u32) {
        if interval_ms == 0 {
            self.auto_cycle_ms = 0;
        } else {
            self.cycle_interval_ms = interval_ms;
            if self.auto_cycle_ms != 0 {
                self.auto_cycle_ms = interval_ms;
            }
        }
        self.last_cycle_ms = self.last_render_ms;
        self.dirty = true;
    }

    #[inline]
    pub fn audio_mode(&self) -> AudioMode {
        self.audio_mode
    }

    #[inline]
    pub fn mic_settings(&self) -> &MicSettings {
        &self.mic_settings
    }

    #[inline]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    #[inline]
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    #[inline]
    pub fn audio_status(&self) -> &'static str {
        match self.audio_mode {
            AudioMode::Off => "Off",
            AudioMode::Synthetic => "Synthetic",
            AudioMode::External => {
                if self.audio_available {
                    "External"
                } else {
                    "Off"
                }
            }
        }
    }

    #[inline]
    pub fn audio_available(&self) -> bool {
        self.audio_available
    }

    #[inline]
    pub fn set_audio_available(&mut self, available: bool) {
        if self.audio_available != available {
            self.audio_available = available;
            self.dirty = true;
        }
    }

    #[inline]
    pub fn set_audio_mode(&mut self, mode: AudioMode) {
        if self.audio_mode != mode {
            self.audio_mode = mode;
            self.dirty = true;
        }
    }

    #[inline]
    pub fn mic_settings_mut(&mut self) -> &mut MicSettings {
        self.dirty = true;
        &mut self.mic_settings
    }

    pub fn set_mic_settings(&mut self, settings: MicSettings) -> Result<(), &'static str> {
        settings.validate()?;
        self.mic_settings = settings;
        self.dirty = true;
        Ok(())
    }

    #[inline]
    pub fn rssi(&self) -> i32 {
        self.rssi
    }

    #[inline]
    pub fn set_rssi(&mut self, rssi: i32) {
        if self.rssi != rssi {
            self.rssi = rssi;
            self.dirty = true;
        }
    }

    #[inline]
    pub fn current_effect_index(&self) -> usize {
        self.current_effect_idx
    }

    #[inline]
    pub fn effect_count(&self) -> usize {
        self.effects.len()
    }

    pub fn current_effect(&self) -> &dyn Effect {
        &*self.effects[self.current_effect_idx]
    }

    pub fn current_effect_mut(&mut self) -> &mut (dyn Effect + 'static) {
        &mut *self.effects[self.current_effect_idx]
    }

    pub fn effects(&self) -> &[Box<dyn Effect>] {
        &self.effects
    }

    /// Select an effect by name or id.
    pub fn switch_to_effect(&mut self, name_or_id: &str) -> bool {
        for (i, e) in self.effects.iter().enumerate() {
            if e.id().eq_ignore_ascii_case(name_or_id) || e.name().eq_ignore_ascii_case(name_or_id)
            {
                self.effects[self.current_effect_idx].deactivate();
                self.current_effect_idx = i;
                self.effects[i].reset();
                self.dirty = true;
                return true;
            }
        }
        false
    }

    /// Renders the current frame.
    /// Frame returned is logical full brightness (the hardware driver or sink applies
    /// power capping and brightness scaling).
    pub fn render(&mut self, now_ms: u64, audio: &AudioSnapshot) -> Frame {
        let delta_seconds = if !self.has_rendered {
            self.has_rendered = true;
            0.0
        } else if now_ms > self.last_render_ms {
            let diff = now_ms - self.last_render_ms;
            // Cap delta to 0.5s to prevent large time jump physics blowup
            ((diff as f32) / 1000.0).min(0.5)
        } else {
            // Duplicate tick or backwards clock: delta must be 0
            0.0
        };
        self.last_render_ms = now_ms;

        // Track audio availability for status reporting
        self.audio_available = match self.audio_mode {
            AudioMode::Off => false,
            AudioMode::Synthetic => true,
            AudioMode::External => audio.available,
        };

        // Auto cycle if enabled
        if self.auto_cycle_ms > 0 && !self.effects.is_empty() {
            if self.last_cycle_ms == 0 {
                self.last_cycle_ms = now_ms;
            } else if now_ms.saturating_sub(self.last_cycle_ms) >= (self.auto_cycle_ms as u64) {
                self.effects[self.current_effect_idx].deactivate();
                self.current_effect_idx = (self.current_effect_idx + 1) % self.effects.len();
                self.effects[self.current_effect_idx].reset();
                self.last_cycle_ms = now_ms;
                self.dirty = true;
            }
        }

        let mut frame = Frame::default();
        if let Some(effect) = self.effects.get_mut(self.current_effect_idx) {
            let context = RenderContext {
                now_ms,
                delta_seconds,
                audio,
                color: self.color,
                text: &self.text,
                scroll_speed: self.speed as f32,
            };
            effect.render(&mut frame, &context);
        }

        frame
    }

    /// Atomically applies a command.
    /// If the command is malformed or invalid, state is untouched and an Err is returned.
    pub fn apply(&mut self, kind: &str, payload: &str) -> Result<(), String> {
        let kind_trimmed = kind.trim();

        if kind_trimmed == "rediscover" {
            self.dirty = true;
            return Ok(());
        }

        if let Some(stripped) = kind_trimmed.strip_prefix("effect/") {
            return self.apply_effect_parameter(stripped, payload);
        }

        match kind_trimmed {
            "light" => self.apply_light(payload),
            "text" => self.apply_text(payload),
            "speed" => self.apply_speed(payload),
            "cycle" => self.apply_cycle(payload),
            "audio_source" => self.apply_audio_source(payload),
            "mic" => self.apply_mic(payload),
            "rssi" => self.apply_rssi(payload),
            other => Err(format!("unknown command kind '{}'", other)),
        }
    }

    fn apply_light(&mut self, payload: &str) -> Result<(), String> {
        let val: serde_json::Value =
            serde_json::from_str(payload).map_err(|e| format!("invalid light JSON: {}", e))?;
        let obj = val
            .as_object()
            .ok_or_else(|| "light payload must be a JSON object".to_string())?;

        // 1. Validate state
        let mut target_on = None;
        if let Some(s) = obj.get("state") {
            let s_str = s
                .as_str()
                .ok_or_else(|| "state must be a string 'ON' or 'OFF'".to_string())?;
            if s_str.eq_ignore_ascii_case("on") {
                target_on = Some(true);
            } else if s_str.eq_ignore_ascii_case("off") {
                target_on = Some(false);
            } else {
                return Err(format!("invalid state value '{}'", s_str));
            }
        }

        // 2. Validate brightness
        let mut target_brightness = None;
        if let Some(b) = obj.get("brightness") {
            let b_num = b
                .as_i64()
                .ok_or_else(|| "brightness must be an integer [0..255]".to_string())?;
            if !(0..=255).contains(&b_num) {
                return Err(format!("brightness {} out of range [0..255]", b_num));
            }
            target_brightness = Some(b_num as u8);
        }

        // 3. Validate effect
        let mut target_effect_idx = None;
        if let Some(e) = obj.get("effect") {
            let e_str = e
                .as_str()
                .ok_or_else(|| "effect must be a string".to_string())?;
            let mut found = None;
            for (idx, effect) in self.effects.iter().enumerate() {
                if effect.name().eq_ignore_ascii_case(e_str)
                    || effect.id().eq_ignore_ascii_case(e_str)
                {
                    found = Some(idx);
                    break;
                }
            }
            if let Some(idx) = found {
                target_effect_idx = Some(idx);
            } else {
                return Err(format!("unknown effect '{}'", e_str));
            }
        }

        // 4. Validate color
        let mut target_color = None;
        if let Some(c) = obj.get("color") {
            let c_obj = c
                .as_object()
                .ok_or_else(|| "color must be an object with r, g, b".to_string())?;
            let r = c_obj
                .get("r")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| "color.r must be an integer [0..255]".to_string())?;
            let g = c_obj
                .get("g")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| "color.g must be an integer [0..255]".to_string())?;
            let b = c_obj
                .get("b")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| "color.b must be an integer [0..255]".to_string())?;
            if !(0..=255).contains(&r) || !(0..=255).contains(&g) || !(0..=255).contains(&b) {
                return Err("color channels must be in range [0..255]".to_string());
            }
            target_color = Some(Rgb::new(r as u8, g as u8, b as u8));
        }

        // Validation passed! Atomically apply:
        if let Some(on) = target_on {
            if !on {
                self.effects[self.current_effect_idx].deactivate();
            }
            self.on = on;
        }
        if let Some(brightness) = target_brightness {
            self.brightness = brightness;
        }
        if let Some(color) = target_color {
            self.color = color;
            // Legacy behavior: if color set without explicit effect, switch to "Solid" and stop cycle
            if target_effect_idx.is_none() {
                for (idx, effect) in self.effects.iter().enumerate() {
                    if effect.id().eq_ignore_ascii_case("solid")
                        || effect.id().eq_ignore_ascii_case("solid_color")
                        || effect.name().eq_ignore_ascii_case("solid")
                        || effect.name().eq_ignore_ascii_case("solid color")
                    {
                        self.effects[self.current_effect_idx].deactivate();
                        self.current_effect_idx = idx;
                        self.effects[idx].reset();
                        self.auto_cycle_ms = 0;
                        break;
                    }
                }
            }
        }
        if let Some(effect_idx) = target_effect_idx {
            self.effects[self.current_effect_idx].deactivate();
            self.current_effect_idx = effect_idx;
            self.effects[effect_idx].reset();
            self.auto_cycle_ms = 0;
        }

        self.dirty = true;
        Ok(())
    }

    fn apply_text(&mut self, payload: &str) -> Result<(), String> {
        let text_clean = if let Ok(s) = serde_json::from_str::<String>(payload) {
            s
        } else {
            payload.to_string()
        };

        if text_clean.len() > 128 {
            return Err("text exceeds maximum length of 128 bytes".to_string());
        }

        self.text = text_clean;

        // Legacy behavior: legacy text command auto-selects TextScroll (exact ID per TextAnimation.h:79)
        for (idx, effect) in self.effects.iter().enumerate() {
            if effect.id() == "TextScroll" || effect.name() == "TextScroll" {
                self.effects[self.current_effect_idx].deactivate();
                self.current_effect_idx = idx;
                self.effects[idx].reset();
                self.auto_cycle_ms = 0;
                break;
            }
        }

        self.dirty = true;
        Ok(())
    }

    fn apply_speed(&mut self, payload: &str) -> Result<(), String> {
        let trimmed = payload.trim();
        let speed_val = trimmed
            .parse::<i64>()
            .map_err(|e| format!("invalid speed integer '{}': {}", trimmed, e))?;
        if !(0..=120).contains(&speed_val) {
            return Err(format!("speed {} out of range [0..120]", speed_val));
        }
        self.speed = speed_val as u32;
        self.dirty = true;
        Ok(())
    }

    fn apply_cycle(&mut self, payload: &str) -> Result<(), String> {
        let trimmed = payload.trim();
        if trimmed.eq_ignore_ascii_case("on") || trimmed == "1" {
            self.auto_cycle_ms = self.cycle_interval_ms;
            self.last_cycle_ms = 0;
        } else if trimmed.eq_ignore_ascii_case("off") || trimmed == "0" {
            self.auto_cycle_ms = 0;
        } else {
            return Err(format!(
                "invalid cycle payload '{}', expected ON or OFF",
                payload
            ));
        }
        self.dirty = true;
        Ok(())
    }

    fn apply_audio_source(&mut self, payload: &str) -> Result<(), String> {
        let trimmed = payload.trim();
        if trimmed.eq_ignore_ascii_case("off") {
            self.audio_mode = AudioMode::Off;
        } else if trimmed.eq_ignore_ascii_case("synthetic") {
            self.audio_mode = AudioMode::Synthetic;
        } else if trimmed.eq_ignore_ascii_case("mic") || trimmed.eq_ignore_ascii_case("external") {
            self.audio_mode = AudioMode::External;
        } else {
            return Err(format!(
                "invalid audio_source '{}', expected 'Off', 'Synthetic', or 'Mic'",
                payload
            ));
        }
        self.dirty = true;
        Ok(())
    }

    fn apply_mic(&mut self, payload: &str) -> Result<(), String> {
        let val: serde_json::Value =
            serde_json::from_str(payload).map_err(|e| format!("invalid mic JSON: {}", e))?;
        let obj = val
            .as_object()
            .ok_or_else(|| "mic payload must be a JSON object".to_string())?;

        let mut candidate = self.mic_settings.clone();

        if let Some(v) = obj.get("gainDb") {
            let f = v
                .as_f64()
                .ok_or_else(|| "gainDb must be a float".to_string())? as f32;
            candidate.gain_db = f;
        }
        if let Some(v) = obj.get("squelchDb") {
            let f = v
                .as_f64()
                .ok_or_else(|| "squelchDb must be a float".to_string())? as f32;
            candidate.squelch_db = f;
        }
        if let Some(v) = obj.get("beatThreshold") {
            let f = v
                .as_f64()
                .ok_or_else(|| "beatThreshold must be a float".to_string())?
                as f32;
            candidate.beat_threshold = f;
        }
        if let Some(v) = obj.get("attackMs") {
            let f = v
                .as_f64()
                .ok_or_else(|| "attackMs must be a float".to_string())? as f32;
            candidate.attack_ms = f;
        }
        if let Some(v) = obj.get("decayMs") {
            let f = v
                .as_f64()
                .ok_or_else(|| "decayMs must be a float".to_string())? as f32;
            candidate.decay_ms = f;
        }
        if let Some(v) = obj.get("agc") {
            let b = v
                .as_bool()
                .ok_or_else(|| "agc must be a boolean".to_string())?;
            candidate.agc = b;
        }
        if let Some(v) = obj.get("rightChannel") {
            let b = v
                .as_bool()
                .ok_or_else(|| "rightChannel must be a boolean".to_string())?;
            candidate.right_channel = b;
        }

        candidate.validate().map_err(|e| e.to_string())?;
        self.mic_settings = candidate;
        self.dirty = true;
        Ok(())
    }

    fn apply_rssi(&mut self, payload: &str) -> Result<(), String> {
        let trimmed = payload.trim();
        let val = trimmed
            .parse::<i32>()
            .map_err(|e| format!("invalid rssi integer '{}': {}", trimmed, e))?;
        self.rssi = val;
        Ok(())
    }

    fn apply_effect_parameter(&mut self, path: &str, payload: &str) -> Result<(), String> {
        let (effect_id, param_id) = path.split_once('/').ok_or_else(|| {
            format!(
                "invalid effect parameter path '{}', expected <effect>/<param>",
                path
            )
        })?;

        let effect_idx = self
            .effects
            .iter()
            .position(|e| e.id() == effect_id || e.name() == effect_id)
            .ok_or_else(|| format!("unknown effect '{}'", effect_id))?;

        let effect = &self.effects[effect_idx];
        let descriptor = effect
            .parameters()
            .iter()
            .find(|p| p.id == param_id)
            .ok_or_else(|| {
                format!(
                    "effect '{}' does not have parameter '{}'",
                    effect_id, param_id
                )
            })?;

        let parsed_value = descriptor.parse_value(payload)?;

        // Apply parameter to effect
        self.effects[effect_idx].set_parameter(param_id, parsed_value)?;
        self.dirty = true;
        Ok(())
    }

    /// Full state snapshot matching legacy JSON schema.
    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::from_slice(&self.snapshot_json(&serde_json::Map::new()).expect("serialize state"))
            .expect("internally serialized state")
    }

    /// Small state without the effect parameter tree, for telemetry consumers.
    pub fn snapshot_scalar(&self) -> serde_json::Value {
        let current_effect = &self.effects[self.current_effect_idx];
        let audio_src = match self.audio_mode {
            AudioMode::Off => "Off",
            AudioMode::Synthetic => "Synthetic",
            AudioMode::External => "Mic",
        };
        let audio_stat = match self.audio_mode {
            AudioMode::Off => "Off",
            AudioMode::Synthetic => "Synthetic",
            AudioMode::External => {
                if self.audio_available {
                    "External"
                } else {
                    "Off"
                }
            }
        };

        json!({
            "state": if self.on { "ON" } else { "OFF" },
            "brightness": self.brightness,
            "color_mode": "rgb",
            "color": {
                "r": self.color.r,
                "g": self.color.g,
                "b": self.color.b
            },
            "effect": current_effect.name(),
            "effect_id": current_effect.id(),
            "text": self.text,
            "speed": self.speed,
            "cycle": if self.auto_cycle_ms > 0 { "ON" } else { "OFF" },
            "rssi": self.rssi,
            "audio_source": audio_src,
            "audio_status": audio_stat,
            "mic": {
                "gainDb": self.mic_settings.gain_db,
                "squelchDb": self.mic_settings.squelch_db,
                "beatThreshold": self.mic_settings.beat_threshold,
                "attackMs": self.mic_settings.attack_ms,
                "decayMs": self.mic_settings.decay_ms,
                "agc": self.mic_settings.agc,
                "rightChannel": self.mic_settings.right_channel
            },
            "effects": null
        })
    }

    /// Serialize state and effect parameters directly without constructing a Value tree.
    pub fn snapshot_json(&self, extra: &serde_json::Map<String, serde_json::Value>) -> Result<Vec<u8>, serde_json::Error> {
        use serde::ser::{SerializeMap, Serializer};
        struct Params<'a>(&'a dyn Effect);
        impl Serialize for Params<'_> {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                let mut map = s.serialize_map(None)?;
                for p in self.0.parameters() {
                    if let Some(v) = self.0.parameter(p.id) {
                        match p.kind {
                            ParameterKind::Boolean => map.serialize_entry(p.id, if v != 0.0 { "ON" } else { "OFF" })?,
                            ParameterKind::Select if (v as usize) < p.options.len() => map.serialize_entry(p.id, p.options[v as usize])?,
                            _ => map.serialize_entry(p.id, &v)?,
                        }
                    }
                }
                map.end()
            }
        }
        struct Effects<'a>(&'a Engine);
        impl Serialize for Effects<'_> {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                let mut map = s.serialize_map(None)?;
                for effect in &self.0.effects {
                    if effect.parameters().iter().any(|p| effect.parameter(p.id).is_some()) {
                        map.serialize_entry(effect.id(), &Params(effect.as_ref()))?;
                    }
                }
                map.end()
            }
        }
        struct Snapshot<'a>(&'a Engine, &'a serde_json::Map<String, serde_json::Value>);
        impl Serialize for Snapshot<'_> {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                let scalar = self.0.snapshot_scalar();
                let mut map = s.serialize_map(None)?;
                for (key, value) in scalar.as_object().expect("scalar object") {
                    if key != "effects" && !self.1.contains_key(key) { map.serialize_entry(key, value)?; }
                }
                for (key, value) in self.1 { if key != "effects" { map.serialize_entry(key, value)?; } }
                map.serialize_entry("effects", &Effects(self.0))?;
                map.end()
            }
        }
        let snapshot = Snapshot(self, extra);
        // Preallocate estimated capacity to avoid multiple reallocations and avoid
        // running a redundant second serialization pass.
        let mut bytes = Vec::with_capacity(9216);
        serde_json::to_writer(&mut bytes, &snapshot)?;
        Ok(bytes)
    }

    /// Serialize directly without materializing the much larger JSON value tree.
    pub fn catalog_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        // Size once to avoid transient doubling during Vec growth on constrained heaps.
        struct Counter(usize);
        impl std::io::Write for Counter {
            fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
                self.0 += data.len();
                Ok(data.len())
            }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }
        let catalog = Catalog(&self.effects);
        let mut counter = Counter(0);
        serde_json::to_writer(&mut counter, &catalog)?;
        let mut bytes = Vec::with_capacity(counter.0);
        serde_json::to_writer(&mut bytes, &catalog)?;
        Ok(bytes)
    }

    /// Effect catalog matching legacy metadata format for discovery & Web UI.
    pub fn catalog(&self) -> serde_json::Value {
        // Keep the legacy Value API while sharing the streaming serializer.
        serde_json::from_slice(&self.catalog_json().expect("catalog serialization"))
            .expect("serialized catalog is valid JSON")
    }
}

// Borrow the catalog metadata and stream it directly into the serialized output.
struct Catalog<'a>(&'a [Box<dyn Effect>]);
struct CatalogEffect<'a>(&'a dyn Effect);
struct CatalogParameters<'a>(&'a dyn Effect);

impl Serialize for Catalog<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for effect in self.0 { seq.serialize_element(&CatalogEffect(effect.as_ref()))?; }
        seq.end()
    }
}
impl Serialize for CatalogEffect<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut item = serializer.serialize_struct("Effect", 3)?;
        item.serialize_field("id", self.0.id())?;
        item.serialize_field("name", self.0.name())?;
        item.serialize_field("parameters", &CatalogParameters(self.0))?;
        item.end()
    }
}
impl Serialize for CatalogParameters<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        #[derive(Serialize)]
        struct Parameter<'a> {
            #[serde(flatten)] descriptor: &'a crate::effect::ParameterDescriptor,
            value: f32,
        }
        let mut seq = serializer.serialize_seq(Some(self.0.parameters().len()))?;
        for p in self.0.parameters() {
            seq.serialize_element(&Parameter { descriptor: p, value: self.0.parameter(p.id).unwrap_or(p.default) })?;
        }
        seq.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::ParameterDescriptor;

    struct DummyEffect {
        id: &'static str,
        name: &'static str,
        param_val: f32,
    }

    impl Effect for DummyEffect {
        fn id(&self) -> &'static str {
            self.id
        }
        fn name(&self) -> &'static str {
            self.name
        }
        fn parameters(&self) -> &[ParameterDescriptor] {
            static PARAMS: [ParameterDescriptor; 1] = [ParameterDescriptor {
                id: "speed",
                label: "Speed",
                kind: ParameterKind::Number,
                min: 1.0,
                max: 100.0,
                step: 1.0,
                default: 40.0,
                options: &[],
                unit: "%",
            }];
            &PARAMS
        }
        fn parameter(&self, id: &str) -> Option<f32> {
            if id == "speed" {
                Some(self.param_val)
            } else {
                None
            }
        }
        fn set_parameter(&mut self, id: &str, value: f32) -> Result<(), String> {
            if id == "speed" {
                self.param_val = value;
                Ok(())
            } else {
                Err("not found".into())
            }
        }
        fn render(&mut self, frame: &mut Frame, _context: &RenderContext<'_>) {
            frame.set(0, 0, Rgb::WHITE);
        }
    }

    #[test]
    fn test_engine_boot_off() {
        let dummy = Box::new(DummyEffect {
            id: "dummy",
            name: "Dummy Effect",
            param_val: 40.0,
        });
        let engine = Engine::new(vec![dummy]).unwrap();
        assert!(!engine.is_on());
        assert_eq!(engine.brightness(), 128);
    }

    #[test]
    fn test_engine_atomic_light_command() {
        let dummy = Box::new(DummyEffect {
            id: "dummy",
            name: "Dummy Effect",
            param_val: 40.0,
        });
        let mut engine = Engine::new(vec![dummy]).unwrap();
        let snap_before = engine.snapshot();

        // Malformed command
        let err = engine.apply(
            "light",
            r#"{"state":"ON","brightness":99,"effect":"does-not-exist"}"#,
        );
        assert!(err.is_err());
        let snap_after = engine.snapshot();

        assert_eq!(snap_before["state"], snap_after["state"]);
        assert_eq!(snap_before["brightness"], snap_after["brightness"]);
        assert_eq!(snap_before["effect"], snap_after["effect"]);

        // Valid command
        let ok = engine.apply("light", r#"{"state":"ON","brightness":200}"#);
        assert!(ok.is_ok());
        assert!(engine.is_on());
        assert_eq!(engine.brightness(), 200);

        // Atomic rollback test: invalid color channel in light command must reject
        // and keep previous state (brightness must still be 200, color unchanged)
        let color_before = engine.color();
        let bad_color_err = engine.apply(
            "light",
            r#"{"state":"OFF","brightness":50,"color":{"r":300,"g":0,"b":0}}"#,
        );
        assert!(bad_color_err.is_err());
        assert!(engine.is_on()); // State NOT turned off
        assert_eq!(engine.brightness(), 200); // Brightness NOT set to 50
        assert_eq!(engine.color(), color_before);

        // Atomic rollback test: invalid mic settings payload must reject
        // and keep existing mic settings untouched
        let mic_before = engine.mic_settings().clone();
        let bad_mic = engine.apply("mic", r#"{"gainDb":100.0}"#); // gainDb max is 60.0
        assert!(bad_mic.is_err());
        assert_eq!(engine.mic_settings(), &mic_before);
    }

    #[test]
    fn test_engine_parameter_command() {
        let dummy = Box::new(DummyEffect {
            id: "dummy",
            name: "Dummy Effect",
            param_val: 40.0,
        });
        let mut engine = Engine::new(vec![dummy]).unwrap();

        assert!(engine.apply("effect/dummy/speed", "55").is_ok());
        let snap = engine.snapshot();
        assert_eq!(snap["effects"]["dummy"]["speed"], 55.0);

        // Out of range
        assert!(engine.apply("effect/dummy/speed", "150").is_err());
        assert_eq!(engine.snapshot()["effects"]["dummy"]["speed"], 55.0);
    }

    #[test]
    fn test_engine_rejects_duplicate_or_empty_ids() {
        // Empty ID
        let e1 = Box::new(DummyEffect {
            id: "",
            name: "Dummy 1",
            param_val: 0.0,
        });
        assert!(Engine::new(vec![e1]).is_err());

        // Empty Name
        let e2 = Box::new(DummyEffect {
            id: "d2",
            name: "  ",
            param_val: 0.0,
        });
        assert!(Engine::new(vec![e2]).is_err());

        // Duplicate ID
        let d1 = Box::new(DummyEffect {
            id: "same_id",
            name: "Dummy One",
            param_val: 0.0,
        });
        let d2 = Box::new(DummyEffect {
            id: "same_id",
            name: "Dummy Two",
            param_val: 0.0,
        });
        assert!(Engine::new(vec![d1, d2]).is_err());

        // Duplicate Name
        let n1 = Box::new(DummyEffect {
            id: "id1",
            name: "Same Name",
            param_val: 0.0,
        });
        let n2 = Box::new(DummyEffect {
            id: "id2",
            name: "Same Name",
            param_val: 0.0,
        });
        assert!(Engine::new(vec![n1, n2]).is_err());
    }

    struct MotionEffect {
        id: &'static str,
        name: &'static str,
        advanced_ms: std::sync::Arc<std::sync::atomic::AtomicU32>,
    }

    impl Effect for MotionEffect {
        fn id(&self) -> &'static str {
            self.id
        }
        fn name(&self) -> &'static str {
            self.name
        }
        fn render(&mut self, _frame: &mut Frame, context: &RenderContext<'_>) {
            let ms = (context.delta_seconds * 1000.0).round() as u32;
            self.advanced_ms
                .fetch_add(ms, std::sync::atomic::Ordering::SeqCst);
        }
    }

    #[test]
    fn test_render_delta_zero_on_duplicate_and_backwards() {
        let advanced = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let effect = Box::new(MotionEffect {
            id: "motion",
            name: "Motion",
            advanced_ms: advanced.clone(),
        });
        let mut engine = Engine::new(vec![effect]).unwrap();
        let audio = AudioSnapshot::default();

        // Frame 1 at now_ms = 100: first frame delta is 0.0, no advance
        engine.render(100, &audio);
        assert_eq!(advanced.load(std::sync::atomic::Ordering::SeqCst), 0);

        // Frame 2 at now_ms = 100: duplicate tick delta must be 0.0, no advance
        engine.render(100, &audio);
        assert_eq!(advanced.load(std::sync::atomic::Ordering::SeqCst), 0);

        // Repeated calls at same timestamp must not advance motion
        for _ in 0..10 {
            engine.render(100, &audio);
        }
        assert_eq!(advanced.load(std::sync::atomic::Ordering::SeqCst), 0);

        // Frame at backwards timestamp (50ms) resets reference without advancing motion
        engine.render(50, &audio);
        assert_eq!(advanced.load(std::sync::atomic::Ordering::SeqCst), 0);

        // Frame at 150ms (100ms after the 50ms reference reset) advances by 100ms
        engine.render(150, &audio);
        assert_eq!(advanced.load(std::sync::atomic::Ordering::SeqCst), 100);

        // Frame at 10_000ms: large forward delta jump (9850ms) must be capped to 0.5s (500ms)
        engine.render(10_000, &audio);
        assert_eq!(advanced.load(std::sync::atomic::Ordering::SeqCst), 600);
    }
}
