//! Regression tests for the transport limits that made the Playlist UI lie:
//! the web UI offered a 73-entry queue while `/api/command` rejected anything
//! over 4096 bytes with HTTP 413, so pressing Start did nothing visible.
//!
//! The engine accepts up to 73 entries per queue and 8 named playlists, and the
//! persisted runtime state is capped at 16 KB — so the transport must accept at
//! least one full queue and a small playlist library.

use matrix_control::api::{body_limit_for, handle_api_request, ApiContext, MAX_API_BODY_BYTES, MAX_COMMAND_BODY_BYTES};
use matrix_control::config::ConfigStore;
use matrix_control::hub::{ControlHub, LedOutput};
use matrix_control::mic::MicController;
use matrix_control::persistence::StateStore;
use matrix_core::{Engine, Frame};
use serde_json::{json, Value};
use std::sync::Arc;

struct FastOutput;
impl LedOutput for FastOutput {
    fn show(&mut self, _frame: &Frame, _brightness: u8) -> Result<(), String> {
        Ok(())
    }
}

/// Every built-in effect id, longest first: worst case for the JSON encoding.
fn effect_ids_by_length(len: usize) -> Vec<String> {
    let effects = matrix_effects::builtin_effects();
    let mut ids: Vec<String> = effects.iter().map(|effect| effect.id().to_string()).collect();
    ids.sort_by_key(|id| std::cmp::Reverse(id.len()));
    ids.truncate(len);
    ids
}

fn command_body(kind: &str, payload: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({ "kind": kind, "payload": payload.to_string() })).unwrap()
}

fn context(hub: &ControlHub) -> ApiContext {
    ApiContext {
        hub: hub.clone(),
        config_store: Arc::new(ConfigStore::new(None)),
        mic_controller: Arc::new(MicController::new(false)),
        state_store: Arc::new(StateStore::new(None)),
    }
}

#[test]
fn full_73_entry_queue_fits_the_command_body_limit() {
    let entries: Vec<Value> = effect_ids_by_length(73)
        .into_iter()
        .map(|id| json!({ "effectId": id, "durationMs": 10000 }))
        .collect();
    let body = command_body("playlist_config", json!({ "entries": entries, "loop": true }));
    // Measured: ~3.9 KB with the longest catalog ids. That leaves < 5 % headroom
    // under the old flat 4 KB limit — any added field or longer id broke it.
    assert!(
        body.len() > MAX_API_BODY_BYTES * 9 / 10,
        "test premise: a full queue must nearly fill the generic limit, got {} bytes",
        body.len()
    );
    assert!(
        body.len() <= MAX_COMMAND_BODY_BYTES,
        "a full queue must fit the command limit, got {} bytes",
        body.len()
    );
    assert_eq!(body_limit_for("POST", "/api/command"), MAX_COMMAND_BODY_BYTES);
    assert_eq!(body_limit_for("POST", "/api/command?x=1"), MAX_COMMAND_BODY_BYTES);
    assert_eq!(body_limit_for("POST", "/api/config"), MAX_API_BODY_BYTES);
    assert_eq!(body_limit_for("GET", "/api/state"), MAX_API_BODY_BYTES);
}

#[test]
fn the_api_accepts_a_full_queue_command_and_reports_it_as_running_capable() {
    let engine = Engine::new(matrix_effects::builtin_effects()).expect("valid effects");
    let hub = ControlHub::spawn(engine, FastOutput, None).unwrap();
    let ctx = context(&hub);

    let entries: Vec<Value> = effect_ids_by_length(73)
        .into_iter()
        .map(|id| json!({ "effectId": id, "durationMs": 10000 }))
        .collect();
    let body = command_body("playlist_config", json!({ "entries": entries, "loop": true }));
    let response = handle_api_request(&ctx, "POST", "/api/command", &body);
    assert!(
        response.status < 400,
        "full queue rejected with {}: {}",
        response.status,
        String::from_utf8_lossy(&response.body)
    );

    let snapshot: Value = serde_json::from_slice(hub.snapshot_json().as_ref()).unwrap();
    assert_eq!(snapshot["playlist"]["entries"].as_array().unwrap().len(), 73);

    let start = handle_api_request(
        &ctx,
        "POST",
        "/api/command",
        &command_body("playlist_control", json!({ "action": "start" })),
    );
    assert!(start.status < 400, "start rejected: {:?}", start.status);
    let snapshot: Value = serde_json::from_slice(hub.snapshot_json().as_ref()).unwrap();
    assert_eq!(snapshot["playlist"]["status"], "running");
    hub.join();
}

#[test]
fn a_two_playlist_library_fits_and_persists() {
    let engine = Engine::new(matrix_effects::builtin_effects()).expect("valid effects");
    let hub = ControlHub::spawn(engine, FastOutput, None).unwrap();
    let ctx = context(&hub);

    let entries: Vec<Value> = effect_ids_by_length(73)
        .into_iter()
        .map(|id| json!({ "effectId": id, "durationMs": 10000 }))
        .collect();
    let playlists: Vec<Value> = (0..2)
        .map(|index| {
            json!({
                "id": format!("queue-{index}"),
                "name": format!("Queue number {index}"),
                "loop": true,
                "entries": entries,
            })
        })
        .collect();
    let body = command_body(
        "playlist_collection",
        json!({ "playlists": playlists, "selectedId": "queue-0" }),
    );
    assert!(
        body.len() > MAX_API_BODY_BYTES,
        "test premise: two full playlists must exceed the generic limit, got {} bytes",
        body.len()
    );
    assert!(body.len() <= MAX_COMMAND_BODY_BYTES);

    let response = handle_api_request(&ctx, "POST", "/api/command", &body);
    assert!(
        response.status < 400,
        "playlist library rejected with {}: {}",
        response.status,
        String::from_utf8_lossy(&response.body)
    );
    let snapshot: Value = serde_json::from_slice(hub.snapshot_json().as_ref()).unwrap();
    assert_eq!(
        snapshot["playlistCollection"]["playlists"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    hub.join();
}

#[test]
fn oversized_command_body_is_still_rejected() {
    let engine = Engine::new(matrix_effects::builtin_effects()).expect("valid effects");
    let hub = ControlHub::spawn(engine, FastOutput, None).unwrap();
    let ctx = context(&hub);

    // The reader enforces MAX_COMMAND_BODY_BYTES before this layer; the API must
    // still refuse a payload the hub cannot accept (bounded payload check).
    let huge = vec![b'a'; MAX_COMMAND_BODY_BYTES + 1];
    let response = handle_api_request(&ctx, "POST", "/api/command", &huge);
    assert!(
        response.status >= 400,
        "oversized body must be rejected, got {}",
        response.status
    );
    hub.join();
}
