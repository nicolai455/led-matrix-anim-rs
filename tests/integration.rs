//! Independent integration acceptance tests against the original effect metadata.
use matrix_control::{ControlHub, LedOutput};
use matrix_core::{AudioSnapshot, Engine, Frame};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

fn engine() -> Engine {
    Engine::new(matrix_effects::builtin_effects()).expect("valid built-in registry")
}

#[test]
fn legacy_catalog_and_all_parameter_ranges_are_preserved() {
    let engine = engine();
    let expected: Value = serde_json::from_str(include_str!("legacy_catalog.json")).unwrap();
    let catalog = engine.catalog();
    let entries = catalog.as_array().expect("catalog array");
    let mut seen = std::collections::BTreeSet::new();
    for entry in entries {
        assert!(
            seen.insert(entry["id"].as_str().unwrap()),
            "duplicate effect ID"
        );
    }
    for legacy in expected.as_array().unwrap() {
        let entry = entries
            .iter()
            .find(|e| e["id"] == legacy["id"])
            .unwrap_or_else(|| panic!("missing legacy effect {}", legacy["id"]));
        assert_eq!(entry["name"], legacy["name"]);
        let parameters = entry["parameters"].as_array().unwrap();
        assert_eq!(
            parameters.len(),
            legacy["parameters"].as_array().unwrap().len(),
            "{} parameter count",
            legacy["id"]
        );
        for param in legacy["parameters"].as_array().unwrap() {
            let actual = parameters.iter().find(|p| p["id"] == param["id"]).unwrap();
            assert_eq!(actual["kind"], param["kind"]);
            for key in ["min", "max", "step", "default"] {
                assert!(
                    (actual[key].as_f64().unwrap() - param[key].as_f64().unwrap()).abs() < 0.0001,
                    "{} {} {key}: {} != {}",
                    legacy["id"],
                    param["id"],
                    actual[key],
                    param[key]
                );
            }
        }
    }
}

#[test]
fn rejected_light_command_does_not_partially_mutate_state() {
    let mut engine = engine();
    let before = engine.snapshot();
    assert!(engine
        .apply(
            "light",
            r#"{"state":"ON","brightness":99,"effect":"does-not-exist"}"#
        )
        .is_err());
    let after = engine.snapshot();
    for key in ["state", "brightness", "effect", "color"] {
        assert_eq!(after[key], before[key], "partial mutation of {key}");
    }
}

#[test]
fn each_effect_can_render_after_a_long_pause_and_time_reset() {
    let mut engine = engine();
    let catalog = engine.catalog();
    engine
        .apply("light", r#"{"state":"ON","brightness":255}"#)
        .unwrap();
    let mut audio = AudioSnapshot::default();
    audio.available = true;
    audio.level = 0.7;
    audio.peak = 0.9;
    audio.bands.fill(0.7);
    audio.waveform.fill(0.3);
    for effect in catalog.as_array().unwrap() {
        engine
            .apply("light", &json!({"effect":effect["name"]}).to_string())
            .unwrap();
        for now in [0, 1, 8, 40, 400, 30_000, 1, u32::MAX as u64 + 5] {
            let frame = engine.render(now, &audio);
            assert_eq!(frame.pixels.len(), 1024);
        }
    }
}

struct SlowSink {
    frames: Arc<AtomicUsize>,
}
impl LedOutput for SlowSink {
    fn show(&mut self, _frame: &Frame, _brightness: u8) -> Result<(), String> {
        std::thread::sleep(Duration::from_millis(140));
        self.frames.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn slow_physical_output_does_not_hold_network_snapshot_locks() {
    let frames = Arc::new(AtomicUsize::new(0));
    let hub = ControlHub::spawn(
        engine(),
        SlowSink {
            frames: frames.clone(),
        },
        None,
    )
    .unwrap();
    let start = Instant::now();
    for _ in 0..100 {
        let snapshot = hub.snapshot();
        assert!(snapshot.is_object());
        let _ = hub.frame();
    }
    assert!(
        start.elapsed() < Duration::from_millis(100),
        "network reads blocked behind show()"
    );
    hub.submit("light", r#"{"state":"ON","brightness":61}"#)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while hub.snapshot()["brightness"] != 61 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(hub.snapshot()["brightness"], 61);
    assert!(frames.load(Ordering::SeqCst) > 0);
}

#[test]
fn ha_discovery_has_unique_ids_and_command_topics_for_every_effect_parameter() {
    let engine = engine();
    let catalog = engine.catalog();
    let discovery = matrix_control::discovery("integration", &catalog);
    let mut ids = std::collections::BTreeSet::new();
    let configs: Vec<Value> = discovery
        .iter()
        .map(|(topic, payload)| {
            assert!(topic.starts_with("homeassistant/"));
            let value: Value = serde_json::from_str(payload).unwrap();
            assert!(ids.insert(value["unique_id"].as_str().unwrap().to_owned()));
            assert_eq!(
                value["availability_topic"],
                "ledmatrix/integration/availability"
            );
            value
        })
        .collect();
    for effect in catalog.as_array().unwrap() {
        for parameter in effect["parameters"].as_array().unwrap() {
            let topic = format!(
                "ledmatrix/integration/effect/{}/{}/set",
                effect["id"].as_str().unwrap(),
                parameter["id"].as_str().unwrap()
            );
            assert!(
                configs
                    .iter()
                    .any(|config| config["command_topic"] == topic),
                "missing {topic}"
            );
        }
    }
}
