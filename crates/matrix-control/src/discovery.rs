use serde_json::{json, Value};

/// Generates Home Assistant MQTT discovery topics and payloads.
///
/// Preserves legacy topics and entities:
/// - 7 base entities: light, text, speed, cycle, rssi, audio_source, audio_status
/// - Dynamic entities for all effect parameters in `catalog`
pub fn discovery(device_id: &str, catalog: &Value) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let result: Result<(), std::convert::Infallible> =
        discovery_for_each(device_id, catalog, |record| {
            entries.push(record);
            Ok(())
        });
    match result {
        Ok(()) => entries,
        Err(error) => match error {},
    }
}

/// Emits one discovery record at a time, bounding embedded publication memory.
pub fn discovery_for_each<E>(
    device_id: &str,
    catalog: &Value,
    mut emit: impl FnMut((String, String)) -> Result<(), E>,
) -> Result<(), E> {
    discovery_inner(device_id, catalog, true, &mut emit)
}

/// Process the catalog one effect at a time instead of materializing the full tree.
pub fn discovery_json_for_each<E>(device_id: &str, catalog: &[u8], mut emit: impl FnMut((String, String)) -> Result<(), E>) -> Result<(), E> {
    let Ok(effects) = serde_json::from_slice::<Vec<&serde_json::value::RawValue>>(catalog) else { return Ok(()); };
    {
        let names: Vec<Value> = effects.iter().filter_map(|e| {
            #[derive(serde::Deserialize)] struct Name { name: String }
            serde_json::from_str::<Name>(e.get()).ok().map(|n| json!({"name": n.name}))
        }).collect();
        discovery_inner(device_id, &Value::Array(names), true, &mut emit)?;
    }
    for effect in effects {
        if let Ok(value) = serde_json::from_str::<Value>(effect.get()) {
            discovery_inner(device_id, &Value::Array(vec![value]), false, &mut emit)?;
        }
    }
    Ok(())
}

fn discovery_inner<E>(device_id: &str, catalog: &Value, base_entities: bool, emit: &mut impl FnMut((String, String)) -> Result<(), E>) -> Result<(), E> {
    let base = format!("ledmatrix/{device_id}");
    let availability_topic = format!("{base}/availability");
    let state_topic = format!("{base}/state");

    let device_obj = json!({
        "identifiers": [device_id],
        "name": "LED Matrix",
        "model": "ESP32-S3 32x32"
    });

    let effect_list: Vec<String> = if let Some(arr) = catalog.as_array().filter(|_| base_entities) {
        arr.iter()
            .filter_map(|e| e["name"].as_str().map(|s| s.to_string()))
            .collect()
    } else {
        Vec::new()
    };

    if base_entities {
    // 1. Light
    let light_config = json!({
        "name": "Matrix",
        "unique_id": format!("{device_id}_light"),
        "device": device_obj.clone(),
        "availability_topic": availability_topic,
        "state_topic": state_topic,
        "command_topic": format!("{base}/light/set"),
        "schema": "json",
        "brightness": true,
        "supported_color_modes": ["rgb"],
        "effect": true,
        "effect_list": effect_list,
    });
    emit((
        format!("homeassistant/light/{device_id}/light/config"),
        light_config.to_string(),
    ))?;

    // 2. Text
    let text_config = json!({
        "name": "Message",
        "unique_id": format!("{device_id}_text"),
        "device": device_obj.clone(),
        "availability_topic": availability_topic,
        "state_topic": state_topic,
        "command_topic": format!("{base}/text/set"),
        "value_template": "{{ value_json.text }}",
        "max": 128,
    });
    emit((
        format!("homeassistant/text/{device_id}/text/config"),
        text_config.to_string(),
    ))?;

    // 3. Speed
    let speed_config = json!({
        "name": "Scroll speed",
        "unique_id": format!("{device_id}_speed"),
        "device": device_obj.clone(),
        "availability_topic": availability_topic,
        "state_topic": state_topic,
        "command_topic": format!("{base}/speed/set"),
        "value_template": "{{ value_json.speed }}",
        "min": 0,
        "max": 120,
        "step": 1,
        "unit_of_measurement": "px/s",
    });
    emit((
        format!("homeassistant/number/{device_id}/speed/config"),
        speed_config.to_string(),
    ))?;

    // 4. Cycle
    let cycle_config = json!({
        "name": "Auto cycle",
        "unique_id": format!("{device_id}_cycle"),
        "device": device_obj.clone(),
        "availability_topic": availability_topic,
        "state_topic": state_topic,
        "command_topic": format!("{base}/cycle/set"),
        "value_template": "{{ value_json.cycle }}",
        "payload_on": "ON",
        "payload_off": "OFF",
        "state_on": "ON",
        "state_off": "OFF",
    });
    emit((
        format!("homeassistant/switch/{device_id}/cycle/config"),
        cycle_config.to_string(),
    ))?;

    // 5. RSSI
    let rssi_config = json!({
        "name": "Wi-Fi signal",
        "unique_id": format!("{device_id}_rssi"),
        "device": device_obj.clone(),
        "availability_topic": availability_topic,
        "state_topic": state_topic,
        "value_template": "{{ value_json.rssi }}",
        "device_class": "signal_strength",
        "unit_of_measurement": "dBm",
        "entity_category": "diagnostic",
    });
    emit((
        format!("homeassistant/sensor/{device_id}/rssi/config"),
        rssi_config.to_string(),
    ))?;

    // 6. Audio Source
    let audio_source_config = json!({
        "name": "Audio source",
        "unique_id": format!("{device_id}_audio_source"),
        "device": device_obj.clone(),
        "availability_topic": availability_topic,
        "state_topic": state_topic,
        "command_topic": format!("{base}/audio_source/set"),
        "value_template": "{{ value_json.audio_source }}",
        "options": ["Off", "Synthetic", "Mic"],
    });
    emit((
        format!("homeassistant/select/{device_id}/audio_source/config"),
        audio_source_config.to_string(),
    ))?;

    // 7. Audio Status
    let audio_status_config = json!({
        "name": "Audio input status",
        "unique_id": format!("{device_id}_audio_status"),
        "device": device_obj.clone(),
        "availability_topic": availability_topic,
        "state_topic": state_topic,
        "value_template": "{{ value_json.audio_status }}",
        "entity_category": "diagnostic",
    });
    emit((
        format!("homeassistant/sensor/{device_id}/audio_status/config"),
        audio_status_config.to_string(),
    ))?;

    }

    // Effect parameter entities
    if let Some(effects) = catalog.as_array() {
        for effect in effects {
            let effect_id = effect["id"].as_str().unwrap_or("");
            let effect_name = effect["name"].as_str().unwrap_or(effect_id);
            if let Some(parameters) = effect["parameters"]
                .as_array()
                .or_else(|| effect["params"].as_array())
            {
                for param in parameters {
                    let param_id = param["id"].as_str().unwrap_or("");
                    let param_label = param["label"].as_str().unwrap_or(param_id);
                    let kind = param["kind"].as_str().unwrap_or("number");
                    let unique_id = format!("{device_id}_{effect_id}_{param_id}");
                    let name = format!("{effect_name} {param_label}");
                    let command_topic = format!("{base}/effect/{effect_id}/{param_id}/set");
                    let value_template =
                        format!("{{{{ value_json.effects['{effect_id}']['{param_id}'] }}}}");

                    let (domain, doc) = match kind {
                        "boolean" => (
                            "switch",
                            json!({
                                "name": name,
                                "unique_id": unique_id,
                                "device": device_obj.clone(),
                                "availability_topic": availability_topic,
                                "state_topic": state_topic,
                                "command_topic": command_topic,
                                "value_template": value_template,
                                "payload_on": "ON",
                                "payload_off": "OFF",
                                "state_on": "ON",
                                "state_off": "OFF",
                            }),
                        ),
                        "select" => {
                            let options = param["options"].as_array().cloned().unwrap_or_default();
                            (
                                "select",
                                json!({
                                    "name": name,
                                    "unique_id": unique_id,
                                    "device": device_obj.clone(),
                                    "availability_topic": availability_topic,
                                    "state_topic": state_topic,
                                    "command_topic": command_topic,
                                    "value_template": value_template,
                                    "options": options,
                                }),
                            )
                        }
                        _ => {
                            let min = param["min"].as_f64().unwrap_or(0.0);
                            let max = param["max"].as_f64().unwrap_or(100.0);
                            let step = param["step"].as_f64().unwrap_or(1.0);
                            let mut doc = json!({
                                "name": name,
                                "unique_id": unique_id,
                                "device": device_obj.clone(),
                                "availability_topic": availability_topic,
                                "state_topic": state_topic,
                                "command_topic": command_topic,
                                "value_template": value_template,
                                "min": min,
                                "max": max,
                                "step": step,
                            });
                            if let Some(unit) = param["unit"].as_str() {
                                if !unit.is_empty() {
                                    doc["unit_of_measurement"] = json!(unit);
                                }
                            }
                            ("number", doc)
                        }
                    };

                    let topic =
                        format!("homeassistant/{domain}/{device_id}/{effect_id}_{param_id}/config");
                    emit((topic, doc.to_string()))?;
                }
            }
        }
    }

    Ok(())
}
