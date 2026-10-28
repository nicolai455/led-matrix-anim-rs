use crate::engine::AudioMode;
use crate::engine::Engine;
use serde_json::json;

/// Serializes the engine's persistent settings matching legacy StateStore JSON schema.
/// Power state (`on`) is deliberately omitted because the display always boots with LEDs OFF.
pub fn serialize_runtime_state(engine: &Engine) -> String {
    let current_effect = engine.current_effect();

    let mut params_map = serde_json::Map::new();
    for effect in engine.effects() {
        if effect.parameters().is_empty() {
            continue;
        }
        let mut effect_params = serde_json::Map::new();
        for param in effect.parameters() {
            if let Some(val) = effect.parameter(param.id) {
                effect_params.insert(param.id.to_string(), json!(val));
            }
        }
        if !effect_params.is_empty() {
            params_map.insert(
                effect.id().to_string(),
                serde_json::Value::Object(effect_params),
            );
        }
    }

    let audio_str = match engine.audio_mode() {
        AudioMode::Off => "Off",
        AudioMode::Synthetic => "Synthetic",
        AudioMode::External => "Mic",
    };

    let mic = engine.mic_settings();

    let doc = json!({
        "version": 1,
        "brightness": engine.brightness(),
        "effect": current_effect.name(),
        "autoCycle": engine.auto_cycle_ms() != 0,
        "text": engine.text(),
        "speed": engine.speed(),
        "color": {
            "r": engine.color().r,
            "g": engine.color().g,
            "b": engine.color().b
        },
        "audio": audio_str,
        "mic": {
            "gainDb": mic.gain_db,
            "squelchDb": mic.squelch_db,
            "beatThreshold": mic.beat_threshold,
            "attackMs": mic.attack_ms,
            "decayMs": mic.decay_ms,
            "agc": mic.agc,
            "rightChannel": mic.right_channel
        },
        "params": params_map
    });

    doc.to_string()
}

/// Restores persistent settings from a StateStore JSON string into the Engine.
/// Invalid or unknown individual entries are skipped, matching legacy StateStore semantics.
pub fn restore_runtime_state(engine: &mut Engine, json_str: &str) -> Result<(), String> {
    if json_str.trim().is_empty() {
        return Ok(());
    }

    // Parse large parameter sets as borrowed raw JSON; allocate only one effect at a time.
    let mut fields: std::collections::BTreeMap<&str, &serde_json::value::RawValue> =
        serde_json::from_str(json_str).map_err(|e| format!("invalid JSON: {e}"))?;
    let params = fields.remove("params");
    let effects = fields.remove("effects");
    if let Some(params) = params.or(effects) {
        let effects: std::collections::BTreeMap<&str, &serde_json::value::RawValue> =
            serde_json::from_str(params.get()).unwrap_or_default();
        for (effect_id, raw) in effects {
            let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(raw.get()) else { continue; };
            for (param_id, val) in map {
                let payload = if let Some(num) = val.as_f64() { Some(num.to_string()) }
                    else if let Some(b) = val.as_bool() { Some(if b { "ON" } else { "OFF" }.to_owned()) }
                    else { val.as_str().map(str::to_owned) };
                if let Some(payload) = payload { let _ = engine.apply(&format!("effect/{effect_id}/{param_id}"), &payload); }
            }
        }
    }
    let val = serde_json::to_value(fields).map_err(|e| e.to_string())?;
    let doc = val.as_object().ok_or("expected JSON object")?;

    // 2. Restore mic settings
    if let Some(mic_val) = doc.get("mic") {
        let _ = engine.apply("mic", &mic_val.to_string());
    }

    // 3. Restore text (may select TextScroll in engine, so effect is restored after)
    if let Some(text) = doc.get("text").and_then(|v| v.as_str()) {
        if text.len() <= 128 {
            let _ = engine.apply("text", text);
        }
    }

    // 4. Restore speed
    if let Some(speed) = doc.get("speed").and_then(|v| v.as_u64()) {
        let _ = engine.apply("speed", &speed.to_string());
    }

    // 5. Restore audio mode
    if let Some(audio) = doc.get("audio").and_then(|v| v.as_str()) {
        let _ = engine.apply("audio_source", audio);
    }

    // 6. Restore light settings (brightness, color, saved effect) LAST after text
    let mut light_obj = serde_json::Map::new();
    if let Some(b) = doc.get("brightness").and_then(|v| v.as_u64()) {
        light_obj.insert("brightness".to_string(), json!(b));
    }
    if let Some(eff_name) = doc.get("effect").and_then(|v| v.as_str()) {
        // Only set if effect is known
        if engine.effects().iter().any(|e| {
            e.name().eq_ignore_ascii_case(eff_name) || e.id().eq_ignore_ascii_case(eff_name)
        }) {
            light_obj.insert("effect".to_string(), json!(eff_name));
            if let Some(color) = doc.get("color") {
                light_obj.insert("color".to_string(), color.clone());
            }
        }
    }
    if !light_obj.is_empty() {
        let _ = engine.apply("light", &serde_json::Value::Object(light_obj).to_string());
    }

    // 7. Restore auto-cycle after light command (since light command disables auto-cycle)
    if let Some(cycle) = doc.get("autoCycle").and_then(|v| v.as_bool()) {
        let _ = engine.apply("cycle", if cycle { "ON" } else { "OFF" });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::{Effect, ParameterDescriptor, ParameterKind, RenderContext};
    use crate::pixel::Frame;

    struct TestEffect {
        id: &'static str,
        name: &'static str,
        val: f32,
    }

    impl Effect for TestEffect {
        fn id(&self) -> &'static str {
            self.id
        }
        fn name(&self) -> &'static str {
            self.name
        }
        fn parameters(&self) -> &[ParameterDescriptor] {
            static P: [ParameterDescriptor; 1] = [ParameterDescriptor {
                id: "p1",
                label: "P1",
                kind: ParameterKind::Number,
                min: 0.0,
                max: 100.0,
                step: 1.0,
                default: 10.0,
                options: &[],
                unit: "",
            }];
            &P
        }
        fn parameter(&self, id: &str) -> Option<f32> {
            if id == "p1" {
                Some(self.val)
            } else {
                None
            }
        }
        fn set_parameter(&mut self, id: &str, value: f32) -> Result<(), String> {
            if id == "p1" {
                self.val = value;
                Ok(())
            } else {
                Err("unknown".into())
            }
        }
        fn render(&mut self, _frame: &mut Frame, _context: &RenderContext<'_>) {}
    }

    #[test]
    fn test_state_serialization_roundtrip() {
        let mut engine = Engine::new(vec![Box::new(TestEffect {
            id: "fx1",
            name: "Effect 1",
            val: 25.0,
        })])
        .unwrap();

        engine.apply("speed", "45").unwrap();
        engine.apply("cycle", "ON").unwrap();
        engine.apply("text", "WORLD").unwrap();

        let json = serialize_runtime_state(&engine);
        assert!(json.contains("\"speed\":45"));
        assert!(json.contains("\"autoCycle\":true"));
        assert!(json.contains("\"text\":\"WORLD\""));
        assert!(json.contains("\"fx1\":{\"p1\":25.0}"));

        // New engine with default state
        let mut new_engine = Engine::new(vec![Box::new(TestEffect {
            id: "fx1",
            name: "Effect 1",
            val: 10.0,
        })])
        .unwrap();

        assert!(!new_engine.is_on()); // Always boots OFF
        assert_eq!(new_engine.speed(), 30);
        restore_runtime_state(&mut new_engine, &json).unwrap();
        assert!(!new_engine.is_on()); // Must REMAIN OFF after restore!
        assert_eq!(new_engine.speed(), 45);
        assert_eq!(new_engine.text(), "WORLD");
        assert!(new_engine.auto_cycle_ms() > 0);
        assert_eq!(new_engine.current_effect().parameter("p1"), Some(25.0));
    }

    #[test]
    fn test_persistence_boot_off_and_tamper_immunity() {
        let mut engine = Engine::new(vec![Box::new(TestEffect {
            id: "fx1",
            name: "Effect 1",
            val: 10.0,
        })])
        .unwrap();

        // Turn engine ON and set custom brightness
        engine
            .apply("light", r#"{"state":"ON","brightness":220}"#)
            .unwrap();
        assert!(engine.is_on());

        // Serialize state: must not persist power ON state
        let serialized = serialize_runtime_state(&engine);
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert!(parsed.get("state").is_none());
        assert!(parsed.get("on").is_none());

        // Restore to fresh engine: must boot OFF and remain OFF
        let mut fresh = Engine::new(vec![Box::new(TestEffect {
            id: "fx1",
            name: "Effect 1",
            val: 10.0,
        })])
        .unwrap();
        assert!(!fresh.is_on());
        restore_runtime_state(&mut fresh, &serialized).unwrap();
        assert!(!fresh.is_on());
        assert_eq!(fresh.brightness(), 220);

        // Even if JSON contains an explicit state: "ON", restore_runtime_state ignores it
        let tampered = r#"{"version":1,"state":"ON","brightness":150}"#;
        restore_runtime_state(&mut fresh, tampered).unwrap();
        assert!(!fresh.is_on());
        assert_eq!(fresh.brightness(), 150);

        // Malformed JSON returns Err and leaves state intact
        let malformed = "not valid json";
        assert!(restore_runtime_state(&mut fresh, malformed).is_err());
        assert_eq!(fresh.brightness(), 150);
    }

    #[test]
    fn test_restore_preserves_non_text_effect_when_text_is_present() {
        let e1 = Box::new(TestEffect {
            id: "TextScroll",
            name: "TextScroll",
            val: 0.0,
        });
        let e2 = Box::new(TestEffect {
            id: "fire",
            name: "Fire",
            val: 0.0,
        });

        let mut engine = Engine::new(vec![e1, e2]).unwrap();
        // Select Fire and set a text message
        engine.apply("light", r#"{"effect":"Fire"}"#).unwrap();
        assert_eq!(engine.current_effect().name(), "Fire");
        engine.apply("text", "SPECIAL_TEXT").unwrap();
        // Legacy text command selected TextScroll, now select Fire explicitly:
        engine.apply("light", r#"{"effect":"Fire"}"#).unwrap();
        assert_eq!(engine.current_effect().name(), "Fire");
        assert_eq!(engine.text(), "SPECIAL_TEXT");

        // Serialize state: contains effect: "Fire" and text: "SPECIAL_TEXT"
        let serialized = serialize_runtime_state(&engine);
        assert!(serialized.contains(r#""effect":"Fire""#));
        assert!(serialized.contains(r#""text":"SPECIAL_TEXT""#));

        // Create new engine and restore
        let e1_new = Box::new(TestEffect {
            id: "TextScroll",
            name: "TextScroll",
            val: 0.0,
        });
        let e2_new = Box::new(TestEffect {
            id: "fire",
            name: "Fire",
            val: 0.0,
        });
        let mut restored_engine = Engine::new(vec![e1_new, e2_new]).unwrap();
        restore_runtime_state(&mut restored_engine, &serialized).unwrap();

        // Must still be Fire, not TextScroll!
        assert_eq!(restored_engine.current_effect().name(), "Fire");
        assert_eq!(restored_engine.text(), "SPECIAL_TEXT");
    }
}
