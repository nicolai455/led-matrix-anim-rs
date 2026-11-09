use matrix_core::{
    restore_runtime_state, serialize_runtime_state, AudioSnapshot, Effect, Engine, Frame,
    RenderContext,
};
use serde_json::{json, Value};

struct Probe {
    id: &'static str,
}

impl Effect for Probe {
    fn id(&self) -> &'static str {
        self.id
    }

    fn name(&self) -> &'static str {
        self.id
    }

    fn render(&mut self, _frame: &mut Frame, _context: &RenderContext<'_>) {}
}

fn engine() -> Engine {
    Engine::new(vec![
        Box::new(Probe { id: "a" }),
        Box::new(Probe { id: "b" }),
    ])
    .unwrap()
}

fn config(engine: &mut Engine, entries: &[(&str, u64)], looped: bool) {
    let payload = json!({
        "entries": entries
            .iter()
            .map(|(effect_id, duration_ms)| json!({
                "effectId": effect_id,
                "durationMs": duration_ms,
            }))
            .collect::<Vec<_>>(),
        "loop": looped,
    });
    engine
        .apply("playlist_config", &payload.to_string())
        .unwrap();
}

fn control(engine: &mut Engine, action: &str) {
    engine
        .apply("playlist_control", &json!({"action": action}).to_string())
        .unwrap();
}

fn playlist(engine: &Engine) -> Value {
    engine.snapshot()["playlist"].clone()
}

fn power(engine: &mut Engine, on: bool) {
    engine
        .apply(
            "light",
            &json!({"state": if on { "ON" } else { "OFF" }}).to_string(),
        )
        .unwrap();
}

fn render(engine: &mut Engine, now_ms: u64) {
    engine.render(now_ms, &AudioSnapshot::default());
}

#[test]
fn playlist_config_accepts_stable_effect_ids_and_rejects_invalid_entries_atomically() {
    let mut engine = engine();
    config(&mut engine, &[("a", 1_000), ("b", 3_600_000)], true);

    assert_eq!(
        playlist(&engine),
        json!({
            "entries": [
                {"effectId": "a", "durationMs": 1_000},
                {"effectId": "b", "durationMs": 3_600_000}
            ],
            "loop": true,
            "status": "stopped",
            "index": null,
            "effectId": null,
            "remainingMs": 0
        })
    );

    let before = playlist(&engine);
    let invalid = json!({
        "entries": [{"effectId": "not-an-id", "durationMs": 1_000}],
        "loop": false
    });
    assert!(engine
        .apply("playlist_config", &invalid.to_string())
        .is_err());
    assert_eq!(playlist(&engine), before);

    let too_short = json!({
        "entries": [{"effectId": "a", "durationMs": 999}],
        "loop": false
    });
    assert!(engine
        .apply("playlist_config", &too_short.to_string())
        .is_err());

    let too_many = json!({
        "entries": (0..33).map(|_| json!({"effectId": "a", "durationMs": 1_000})).collect::<Vec<_>>(),
        "loop": false
    });
    assert!(engine
        .apply("playlist_config", &too_many.to_string())
        .is_err());
}

#[test]
fn starting_playlist_from_boot_off_turns_display_on_and_selects_first_entry() {
    let mut engine = engine();
    config(&mut engine, &[("a", 1_000), ("b", 2_000)], false);

    assert!(!engine.is_on(), "boot remains dark until explicit user action");
    control(&mut engine, "start");

    assert!(engine.is_on(), "explicit playlist start should turn output on");
    assert_eq!(playlist(&engine)["status"], "running");
    assert_eq!(playlist(&engine)["index"], 0);
    assert_eq!(engine.current_effect().id(), "a");
}

#[test]
fn playlist_large_time_jumps_preserve_loop_phase_and_wrap() {
    let mut engine = engine();
    config(&mut engine, &[("a", 1_000), ("b", 2_000)], true);
    power(&mut engine, true);
    control(&mut engine, "start");

    render(&mut engine, 1_000_000);
    assert_eq!(playlist(&engine)["index"], 1);
    assert_eq!(playlist(&engine)["effectId"], "b");
    assert_eq!(playlist(&engine)["remainingMs"], 2_000);
    assert_eq!(engine.current_effect().id(), "b");
}

#[test]
fn playlist_advances_by_fake_time_and_stops_after_last_non_looping_entry() {
    let mut engine = engine();
    config(&mut engine, &[("a", 1_000), ("b", 2_000)], false);
    power(&mut engine, true);
    control(&mut engine, "start");

    render(&mut engine, 400);
    assert_eq!(playlist(&engine)["remainingMs"], 600);
    assert_eq!(engine.current_effect().id(), "a");

    render(&mut engine, 1_000);
    assert_eq!(playlist(&engine)["status"], "running");
    assert_eq!(playlist(&engine)["index"], 1);
    assert_eq!(playlist(&engine)["effectId"], "b");
    assert_eq!(playlist(&engine)["remainingMs"], 2_000);
    assert_eq!(engine.current_effect().id(), "b");

    render(&mut engine, 3_000);
    render(&mut engine, 4_000);
    assert_eq!(playlist(&engine)["status"], "stopped");
    assert_eq!(playlist(&engine)["remainingMs"], 0);
    assert_eq!(engine.current_effect().id(), "b");
}

#[test]
fn playlist_pause_resume_and_power_off_freeze_remaining_time() {
    let mut engine = engine();
    config(&mut engine, &[("a", 1_000), ("b", 2_000)], false);
    power(&mut engine, true);
    control(&mut engine, "start");
    render(&mut engine, 400);
    control(&mut engine, "pause");
    assert_eq!(playlist(&engine)["status"], "paused");
    assert_eq!(playlist(&engine)["remainingMs"], 600);

    render(&mut engine, 5_000);
    assert_eq!(playlist(&engine)["remainingMs"], 600);
    control(&mut engine, "resume");
    render(&mut engine, 5_300);
    assert_eq!(playlist(&engine)["remainingMs"], 300);

    power(&mut engine, false);
    render(&mut engine, 10_000);
    assert_eq!(playlist(&engine)["remainingMs"], 300);
    power(&mut engine, true);
    render(&mut engine, 10_299);
    assert_eq!(playlist(&engine)["remainingMs"], 1);
    render(&mut engine, 10_300);
    assert_eq!(playlist(&engine)["index"], 1);
    assert_eq!(playlist(&engine)["remainingMs"], 2_000);
}

#[test]
fn manual_switch_api_and_light_choice_stop_playback_and_off_on_only_resumes_running() {
    let mut engine = engine();
    config(&mut engine, &[("a", 1_000), ("b", 2_000)], true);
    power(&mut engine, true);
    control(&mut engine, "start");
    render(&mut engine, 250);
    assert!(engine.switch_to_effect("b"));
    assert_eq!(playlist(&engine)["status"], "stopped");
    assert_eq!(playlist(&engine)["remainingMs"], 0);

    control(&mut engine, "start");
    render(&mut engine, 500);
    power(&mut engine, false);
    assert_eq!(playlist(&engine)["remainingMs"], 750);
    render(&mut engine, 5_000);
    assert_eq!(playlist(&engine)["remainingMs"], 750);
    power(&mut engine, true);
    render(&mut engine, 5_200);
    assert_eq!(playlist(&engine)["remainingMs"], 550);

    engine.apply("light", r#"{"effect":"b"}"#).unwrap();
    assert_eq!(playlist(&engine)["status"], "stopped");
    assert_eq!(playlist(&engine)["remainingMs"], 0);
}

#[test]
fn looping_playlist_and_navigation_work_and_manual_choice_stops_it() {
    let mut engine = engine();
    config(&mut engine, &[("a", 1_000), ("b", 2_000)], true);
    power(&mut engine, true);
    control(&mut engine, "start");
    render(&mut engine, 1_000);
    assert_eq!(engine.current_effect().id(), "b");
    control(&mut engine, "previous");
    assert_eq!(engine.current_effect().id(), "a");
    assert_eq!(playlist(&engine)["status"], "running");
    assert_eq!(playlist(&engine)["remainingMs"], 1_000);

    control(&mut engine, "next");
    assert_eq!(engine.current_effect().id(), "b");
    assert_eq!(playlist(&engine)["index"], 1);
    engine.apply("light", r#"{"effect":"a"}"#).unwrap();
    assert_eq!(playlist(&engine)["status"], "stopped");
    assert_eq!(engine.current_effect().id(), "a");
}

#[test]
fn playlist_and_auto_cycle_are_mutually_exclusive() {
    let mut engine = engine();
    config(&mut engine, &[("a", 1_000), ("b", 1_000)], true);
    engine.set_auto_cycle_interval_ms(1_000);
    engine.apply("cycle", "ON").unwrap();
    power(&mut engine, true);

    control(&mut engine, "start");
    assert_eq!(engine.auto_cycle_ms(), 0);
    assert_eq!(playlist(&engine)["status"], "running");

    engine.apply("cycle", "ON").unwrap();
    assert_eq!(playlist(&engine)["status"], "stopped");
    assert!(engine.auto_cycle_ms() > 0);
}

#[test]
fn restore_keeps_configured_playlist_stopped_without_overriding_auto_cycle() {
    let mut restored = engine();
    let state = json!({
        "version": 1,
        "autoCycle": true,
        "playlist": {
            "entries": [{"effectId": "a", "durationMs": 1_000}],
            "loop": true
        }
    });

    restore_runtime_state(&mut restored, &state.to_string()).unwrap();

    assert!(restored.auto_cycle_ms() > 0);
    assert_eq!(playlist(&restored)["status"], "stopped");
    assert_eq!(playlist(&restored)["entries"][0]["effectId"], "a");
}

#[test]
fn playlist_configuration_persists_but_restore_never_resumes_playback_or_power() {
    let mut original = engine();
    config(&mut original, &[("b", 4_000), ("a", 5_000)], true);
    power(&mut original, true);
    control(&mut original, "start");

    let saved = serialize_runtime_state(&original);
    let saved_value: Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(
        saved_value["playlist"],
        json!({
            "entries": [
                {"effectId": "b", "durationMs": 4_000},
                {"effectId": "a", "durationMs": 5_000}
            ],
            "loop": true
        })
    );
    assert!(saved_value.get("state").is_none());

    let mut restored = engine();
    restore_runtime_state(&mut restored, &saved).unwrap();
    assert!(!restored.is_on());
    assert_eq!(
        playlist(&restored),
        json!({
            "entries": [
                {"effectId": "b", "durationMs": 4_000},
                {"effectId": "a", "durationMs": 5_000}
            ],
            "loop": true,
            "status": "stopped",
            "index": null,
            "effectId": null,
            "remainingMs": 0
        })
    );
}
