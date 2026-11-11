use matrix_core::{
    restore_runtime_state, serialize_runtime_state, AudioSnapshot, Effect, Engine, Frame,
    PlaylistCollection, RenderContext, StoredPlaylist, StoredPlaylistEntry, MAX_STORED_PLAYLISTS,
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
fn named_playlists_can_be_saved_selected_and_deleted_without_autostart() {
    let mut original = engine();
    original.apply("playlist_save", &json!({
        "id":"first", "name":"First list", "entries":[{"effectId":"a","durationMs":5000}], "loop":false
    }).to_string()).unwrap();
    original.apply("playlist_save", &json!({
        "id":"second", "name":"Second list", "entries":[{"effectId":"b","durationMs":8000}], "loop":true
    }).to_string()).unwrap();

    assert_eq!(original.playlist_collection().playlists.len(), 2);
    assert_eq!(original.playlist_collection().selected_id.as_deref(), Some("second"));
    assert_eq!(playlist(&original)["entries"][0]["effectId"], "b");
    assert_eq!(playlist(&original)["entries"][0]["durationMs"], 8000);
    assert_eq!(playlist(&original)["status"], "stopped");
    assert!(!original.is_on());

    original.apply("playlist_select", &json!({"id":"first"}).to_string()).unwrap();
    assert_eq!(original.playlist_collection().selected_id.as_deref(), Some("first"));
    assert_eq!(playlist(&original)["entries"][0]["effectId"], "a");
    assert_eq!(playlist(&original)["status"], "stopped");

    original.apply("playlist_delete", &json!({"id":"first"}).to_string()).unwrap();
    assert_eq!(original.playlist_collection().playlists.len(), 1);
    assert_eq!(original.playlist_collection().selected_id, None);
}

#[test]
fn saving_invalid_named_playlist_is_atomic() {
    let mut original = engine();
    original.apply("playlist_save", &json!({
        "id":"first", "name":"First list", "entries":[{"effectId":"a","durationMs":5000}], "loop":false
    }).to_string()).unwrap();
    let before = original.playlist_collection().clone();
    assert!(original.apply("playlist_save", &json!({
        "id":"bad id", "name":"Bad", "entries":[{"effectId":"unknown","durationMs":5000}], "loop":false
    }).to_string()).is_err());
    assert_eq!(original.playlist_collection(), &before);
}

#[test]
fn playlist_configuration_and_collection_round_trip_separately() {
    let mut original = engine();
    original.apply("playlist_save", &json!({
        "id":"saved", "name":"Saved list", "entries":[{"effectId":"b","durationMs":5000}], "loop":true
    }).to_string()).unwrap();
    let saved = serialize_runtime_state(&original);
    let serialized: serde_json::Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(serialized["playlistCollection"]["playlists"].as_array().unwrap().len(), 1);
    let mut restored = engine();
    restore_runtime_state(&mut restored, &saved).unwrap();
    assert_eq!(restored.playlist_collection().playlists.len(), 1);
    assert_eq!(restored.playlist_collection().selected_id.as_deref(), Some("saved"));
    assert_eq!(playlist(&restored)["entries"][0]["effectId"], "b");
    assert_eq!(playlist(&restored)["status"], "stopped");
    assert!(!restored.is_on());
}

#[test]
fn playlist_collection_rejects_oversized_or_invalid_state() {
    let collection = PlaylistCollection { playlists: (0..=matrix_core::engine::MAX_STORED_PLAYLISTS).map(|i| StoredPlaylist {
        id: format!("p{i}"), name: format!("Playlist {i}"),
        entries: vec![StoredPlaylistEntry { effect_id:"a".into(), duration_ms:5000 }], r#loop:false
    }).collect(), ..Default::default() };
    assert!(collection.validate().is_err());
}

#[test]
fn default_sound_playlist_contains_every_inventory_effect_for_ten_seconds() {
    let playlist = matrix_effects::default_sound_playlist();
    let expected = [
        "arcade_invaders", "audio_mesh", "audio_pulse", "audio_tunnel", "audio_waveform",
        "black_hole", "bongo_cat", "bouncing_speakers", "chaser_cannons", "cyber_rain",
        "cyber_tunnel", "dancing_banana", "dancing_skeleton", "derby_flower", "disco_ball",
        "disco_floor", "disco_pinwheel", "dna_spiral", "doge_wow", "domain_warp",
        "equalizer_city", "fire", "fire_tornado", "fireworks", "freq_bars", "flappy_bird",
        "galaxy_spiral", "game_of_life", "hamster_dance", "hazard_pulsar", "hypno_spirals",
        "julia_orbit", "kaleidoscope", "laser_harp", "laser_scanner", "laser_show",
        "lava_lamp", "liquid_oil", "lissajous", "mario_stomp", "matrix_rain", "metaballs",
        "mirror_cavern", "moving_heads", "neon_heart", "nyan_cat", "octopus_scanner",
        "outrun_drive", "pacman_chase", "party_dancers", "party_duck", "party_minion",
        "plasma", "polar_lights", "police_strobe", "popcat", "pulse_ball", "radial_spectrum",
        "retro_breakout", "ripple", "rotating_fish", "rotozoom", "spectrogram", "spectrum_orb",
        "spinning_vinyl", "starfield", "strobe_blinder", "stroboscope", "supernova",
        "text_effects", "ufo_abduction", "water_caustics", "yin_yang",
    ];
    assert_eq!(playlist.id, "sound-default");
    assert_eq!(playlist.name, "Sound reactive");
    assert!(playlist.r#loop);
    assert_eq!(playlist.entries.len(), expected.len());
    assert_eq!(playlist.entries.iter().map(|entry| entry.effect_id.as_str()).collect::<Vec<_>>(), expected);
    assert!(playlist.entries.iter().all(|entry| entry.duration_ms == 10_000));
}

#[test]
fn default_playlist_can_be_installed_as_named_playlist_and_keeps_boot_off() {
    let mut engine = Engine::new(matrix_effects::builtin_effects()).unwrap();
    assert!(!engine.is_on());
    engine
        .install_default_sound_playlist(matrix_effects::default_sound_playlist())
        .unwrap();
    assert_eq!(engine.playlist_collection().playlists.len(), 1);
    let snapshot = engine.snapshot();
    assert!(!engine.is_on(), "installing defaults must not turn LED output on");
    let preset = snapshot["playlistCollection"]["playlists"].as_array().unwrap().iter()
        .find(|entry| entry["id"] == "sound-default").unwrap();
    assert_eq!(preset["entries"].as_array().unwrap().len(), 73);
    assert!(preset["entries"].as_array().unwrap().iter().all(|entry| entry["durationMs"] == 10_000));
    assert_eq!(snapshot["playlist"]["status"], "stopped");
}

#[test]
fn default_playlist_does_not_overflow_a_full_user_playlist_collection() {
    let mut engine = Engine::new(matrix_effects::builtin_effects()).unwrap();
    for index in 0..MAX_STORED_PLAYLISTS {
        engine.save_named_playlist(StoredPlaylist {
            id: format!("list-{index}"),
            name: format!("List {index}"),
            entries: vec![StoredPlaylistEntry { effect_id: "audio_pulse".into(), duration_ms: 1_000 }],
            r#loop: false,
        }).unwrap();
    }
    let existing = engine.playlist_collection().playlists.clone();
    assert_eq!(existing.len(), MAX_STORED_PLAYLISTS);
    assert!(engine.install_default_sound_playlist(matrix_effects::default_sound_playlist()).is_err());
    assert_eq!(engine.playlist_collection().playlists, existing);
}

#[test]
fn default_playlist_installs_when_collection_has_room_without_dropping_playlists() {
    let mut engine = Engine::new(matrix_effects::builtin_effects()).unwrap();
    for index in 0..MAX_STORED_PLAYLISTS - 1 {
        engine.save_named_playlist(StoredPlaylist {
            id: format!("list-{index}"),
            name: format!("List {index}"),
            entries: vec![StoredPlaylistEntry { effect_id: "audio_pulse".into(), duration_ms: 1_000 }],
            r#loop: false,
        }).unwrap();
    }
    engine.install_default_sound_playlist(matrix_effects::default_sound_playlist()).unwrap();
    assert_eq!(engine.playlist_collection().playlists.len(), MAX_STORED_PLAYLISTS);
    assert!(engine.playlist_collection().playlists.iter().any(|playlist| playlist.id == "sound-default"));
}

#[test]
fn default_playlist_is_not_installed_when_collection_is_full() {
    let mut engine = Engine::new(matrix_effects::builtin_effects()).unwrap();
    for index in 0..MAX_STORED_PLAYLISTS {
        engine.save_named_playlist(StoredPlaylist {
            id: format!("list-{index}"),
            name: format!("List {index}"),
            entries: vec![StoredPlaylistEntry { effect_id: "audio_pulse".into(), duration_ms: 1_000 }],
            r#loop: false,
        }).unwrap();
    }
    assert!(engine.install_default_sound_playlist(matrix_effects::default_sound_playlist()).is_err());
    assert_eq!(engine.playlist_collection().playlists.len(), MAX_STORED_PLAYLISTS);
    assert!(!engine.playlist_collection().playlists.iter().any(|playlist| playlist.id == "sound-default"));
}

#[test]
fn default_playlist_install_does_not_overwrite_a_user_edited_default_playlist() {
    let mut engine = Engine::new(matrix_effects::builtin_effects()).unwrap();
    let edited = StoredPlaylist {
        id: "sound-default".into(),
        name: "My sound playlist".into(),
        entries: vec![StoredPlaylistEntry { effect_id: "audio_pulse".into(), duration_ms: 12_000 }],
        r#loop: false,
    };
    engine.save_named_playlist(edited.clone()).unwrap();
    engine.install_default_sound_playlist(matrix_effects::default_sound_playlist()).unwrap();
    let saved = engine.playlist_collection().playlists.iter().find(|playlist| playlist.id == "sound-default").unwrap();
    assert_eq!(saved, &edited, "boot-time default installation must preserve user edits");
}

#[test]
fn default_playlist_install_preserves_existing_user_playback_and_custom_playlists() {
    let mut engine = Engine::new(matrix_effects::builtin_effects()).unwrap();
    let custom = StoredPlaylist {
        id: "custom".into(),
        name: "Custom".into(),
        entries: vec![StoredPlaylistEntry { effect_id: "audio_pulse".into(), duration_ms: 12_000 }],
        r#loop: false,
    };
    engine.save_named_playlist(custom.clone()).unwrap();
    engine.apply("light", r#"{"state":"ON"}"#).unwrap();
    engine.apply("playlist_control", r#"{"action":"start"}"#).unwrap();
    let active_before = engine.snapshot()["playlist"].clone();
    let was_on = engine.is_on();

    engine.install_default_sound_playlist(matrix_effects::default_sound_playlist()).unwrap();
    let snapshot = engine.snapshot();
    assert_eq!(snapshot["playlist"]["entries"], active_before["entries"]);
    assert_eq!(snapshot["playlist"]["status"], active_before["status"]);
    assert_eq!(snapshot["playlistCollection"]["selectedId"], "custom");
    assert_eq!(snapshot["playlistCollection"]["playlists"].as_array().unwrap().len(), 2);
    assert_eq!(snapshot["playlistCollection"]["playlists"][0]["id"], "custom");
    assert_eq!(engine.is_on(), was_on);
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
        "entries": (0..74).map(|_| json!({"effectId": "a", "durationMs": 1_000})).collect::<Vec<_>>(),
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
    let serialized = serde_json::to_value(original.playlist_config()).unwrap();

    let saved = serialize_runtime_state(&original);
    let saved_value: Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(
        serialized,
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
