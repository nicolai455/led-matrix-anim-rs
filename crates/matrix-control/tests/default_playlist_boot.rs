//! Regression test for the fresh-boot playlist bug: the shipped
//! `sound-default` playlist must be installed *and* activated, so that the web
//! UI shows a startable queue without any user preparation — while the output
//! stays dark until Start is pressed explicitly.
//!
//! Mirrors the boot sequence in `crates/matrix-control/src/bin/simulator.rs`
//! (and `firmware/src/main.rs`).

use matrix_core::{Engine, PlaylistCollection};
use serde_json::json;

/// Reproduces the simulator boot: built-in effects, no saved state, default
/// playlist installed as a named playlist and then selected as the active queue.
fn boot_engine_with_default_playlist() -> Engine {
    let mut engine = Engine::new(matrix_effects::builtin_effects()).expect("valid effects");
    engine
        .install_default_sound_playlist(matrix_effects::default_sound_playlist())
        .expect("default playlist installs");

    let active_queue_empty = engine
        .playlist_config()
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|entries| entries.is_empty());
    assert!(active_queue_empty, "fresh engine must start with no active queue");
    engine
        .apply("playlist_select", &json!({"id": "sound-default"}).to_string())
        .expect("default playlist becomes the active queue");
    assert!(!engine.is_on());
    engine
        .apply("light", r#"{"state":"OFF"}"#)
        .expect("boot stays dark");
    engine
}

#[test]
fn fresh_boot_exposes_startable_default_playlist() {
    let engine = boot_engine_with_default_playlist();
    let snap = engine.snapshot();

    assert_eq!(snap["playlistCollection"]["selectedId"], "sound-default");
    assert_eq!(snap["playlist"]["entries"].as_array().unwrap().len(), 73);
    assert_eq!(snap["playlistCollection"]["playlists"][0]["entries"].as_array().unwrap().len(), 73);
    assert_eq!(snap["playlist"]["status"], "stopped");
    assert_eq!(snap["state"], "OFF", "boot must stay dark until Start");
    assert_eq!(snap["playlist"]["index"], serde_json::Value::Null);
}

#[test]
fn fresh_boot_default_playlist_starts_on_request() {
    let mut engine = boot_engine_with_default_playlist();
    engine
        .apply("playlist_control", r#"{"action":"start"}"#)
        .expect("start accepts the preselected queue");
    let snap = engine.snapshot();
    assert_eq!(snap["playlist"]["status"], "running");
    assert_eq!(snap["playlist"]["index"], 0);
    assert_eq!(snap["state"], "ON", "Start is an explicit user action and wakes the output");
    assert_eq!(snap["playlistConfig"]["entries"].as_array().unwrap().len(), 73);

    engine
        .apply("playlist_control", r#"{"action":"pause"}"#)
        .expect("pause running playlist");
    assert_eq!(engine.snapshot()["playlist"]["status"], "paused");
    engine
        .apply("playlist_control", r#"{"action":"stop"}"#)
        .expect("stop paused playlist");
    assert_eq!(engine.snapshot()["playlist"]["status"], "stopped");
}

#[test]
fn persisted_collection_without_selection_is_repaired_at_boot() {
    // Regression for state files written by the previous boot code: the named
    // playlist exists but neither selectedId nor the active queue were set.
    let mut engine = Engine::new(matrix_effects::builtin_effects()).expect("valid effects");
    engine
        .install_default_sound_playlist(matrix_effects::default_sound_playlist())
        .expect("default playlist installs");
    let collection: PlaylistCollection =
        serde_json::from_value(engine.snapshot()["playlistCollection"].clone()).unwrap();
    assert_eq!(collection.selected_id, None, "install alone leaves no selection");
    assert!(engine.playlist_config()["entries"].as_array().unwrap().is_empty());

    engine
        .apply("playlist_select", &json!({"id": "sound-default"}).to_string())
        .expect("select repairs the missing active queue");
    let snap = engine.snapshot();
    assert_eq!(snap["playlistCollection"]["selectedId"], "sound-default");
    assert_eq!(snap["playlist"]["entries"].as_array().unwrap().len(), 73);
    assert_eq!(snap["playlist"]["status"], "stopped");
    assert_eq!(snap["state"], "OFF");
}

#[test]
fn boot_selection_keeps_a_saved_active_queue_untouched() {
    // A user queue saved in state.json must win over the default playlist.
    let mut engine = Engine::new(matrix_effects::builtin_effects()).expect("valid effects");
    let first = engine.snapshot()["effect_id"].as_str().unwrap().to_string();
    engine
        .apply(
            "playlist_config",
            &json!({"entries":[{"effectId": first, "durationMs": 5000}], "loop": false}).to_string(),
        )
        .expect("custom queue applies");
    engine
        .install_default_sound_playlist(matrix_effects::default_sound_playlist())
        .expect("default playlist installs");

    let active_queue_empty = engine
        .playlist_config()
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|entries| entries.is_empty());
    assert!(!active_queue_empty, "a saved active queue must suppress the boot default");
    assert_eq!(engine.snapshot()["playlist"]["entries"].as_array().unwrap().len(), 1);
}
