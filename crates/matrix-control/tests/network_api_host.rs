//! The Wi-Fi card in the web UI is only testable if the host adapter answers
//! `/api/network` with the firmware's response shape instead of a flat 503.
//! These tests drive the real HTTP handler, so the JSON the browser sees is
//! exactly what is asserted here.

use matrix_control::api::{handle_api_request, ApiContext};
use matrix_control::config::ConfigStore;
use matrix_control::hub::{ControlHub, LedOutput};
use matrix_control::mic::MicController;
use matrix_control::network_host;
use matrix_control::persistence::StateStore;
use matrix_core::{Engine, Frame};
use serde_json::Value;
use std::sync::Arc;

struct FastOutput;
impl LedOutput for FastOutput {
    fn show(&mut self, _frame: &Frame, _brightness: u8) -> Result<(), String> {
        Ok(())
    }
}

fn context() -> ApiContext {
    let engine = Engine::new(matrix_effects::builtin_effects()).expect("valid effects");
    let hub = ControlHub::spawn(engine, FastOutput, None).unwrap();
    ApiContext {
        hub,
        config_store: Arc::new(ConfigStore::new(None)),
        mic_controller: Arc::new(MicController::new(false)),
        state_store: Arc::new(StateStore::new(None)),
    }
}

fn json(response: matrix_control::api::HttpResponse) -> Value {
    serde_json::from_slice(&response.body).unwrap_or(Value::Null)
}

#[test]
fn the_host_serves_wifi_configuration_instead_of_a_503() {
    let _guard = network_host::test_lock();
    network_host::reset();
    let ctx = context();
    let response = handle_api_request(&ctx, "GET", "/api/network", &[]);
    assert_eq!(response.status, 200, "the Wi-Fi card must load on the host");
    let body = json(response);
    assert_eq!(body["networkAvailable"], false, "no radio on the host");
    assert!(body["wifiProfiles"].is_array());
    assert_eq!(body["fallbackApSsid"], "JDJLIGHT");
    assert_eq!(body["mqttPort"], 1883);
}

#[test]
fn profiles_round_trip_and_secrets_stay_hidden() {
    let _guard = network_host::test_lock();
    network_host::reset();
    let ctx = context();
    let body = r#"{"wifiProfiles":[
        {"ssid":"home","password":"longenough","priority":1},
        {"ssid":"garage","password":"","priority":7}
    ],"fallbackApSsid":"MatrixSetup","fallbackApPassword":"appassword"}"#;
    let response = handle_api_request(&ctx, "POST", "/api/network", body.as_bytes());
    assert_eq!(
        response.status,
        200,
        "{}",
        String::from_utf8_lossy(&response.body)
    );
    assert!(
        !String::from_utf8_lossy(&response.body).contains("longenough"),
        "stored passphrases must never be returned"
    );

    let body = json(handle_api_request(&ctx, "GET", "/api/network", &[]));
    let profiles = body["wifiProfiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0]["ssid"], "garage", "higher priority sorts first");
    assert_eq!(profiles[1]["ssid"], "home");
    assert_eq!(profiles[1]["passwordConfigured"], true);
    assert_eq!(profiles[0]["passwordConfigured"], false);
    assert_eq!(body["fallbackApSsid"], "MatrixSetup");
    assert_eq!(body["fallbackApEnabled"], true);
}

#[test]
fn invalid_configuration_is_rejected_with_an_explanation() {
    let _guard = network_host::test_lock();
    network_host::reset();
    let ctx = context();
    for (body, expected_status) in [
        (
            r#"{"wifiProfiles":[{"ssid":"shortpw","password":"abc"}]}"#,
            400,
        ),
        (r#"{"wifiProfiles":[{"ssid":"dup"},{"ssid":"dup"}]}"#, 400),
        (
            r#"{"wifiProfiles":[{"ssid":"ok"}],"fallbackApPassword":"tiny"}"#,
            400,
        ),
        (r#"{"nope":true}"#, 400),
        ("not json", 400),
    ] {
        let response = handle_api_request(&ctx, "POST", "/api/network", body.as_bytes());
        assert_eq!(response.status, expected_status, "body: {body}");
        let error = json(response);
        assert!(
            error["error"].as_str().is_some_and(|text| !text.is_empty()),
            "an invalid request must explain itself: body {body}"
        );
    }
}

#[test]
fn clearing_the_ap_password_disables_the_fallback_ap() {
    let _guard = network_host::test_lock();
    network_host::reset();
    let ctx = context();
    let enable = br#"{"fallbackApPassword":"appassword"}"#;
    assert_eq!(
        handle_api_request(&ctx, "POST", "/api/network", enable).status,
        200
    );
    assert_eq!(
        json(handle_api_request(&ctx, "GET", "/api/network", &[]))["fallbackApEnabled"],
        true
    );
    let clear = br#"{"clearFallbackApPassword":true}"#;
    assert_eq!(
        handle_api_request(&ctx, "POST", "/api/network", clear).status,
        200
    );
    let body = json(handle_api_request(&ctx, "GET", "/api/network", &[]));
    assert_eq!(body["fallbackApEnabled"], false);
    assert_eq!(body["fallbackApPasswordConfigured"], false);
}

#[test]
fn the_host_answers_the_scan_endpoint_without_pretending_to_have_a_radio() {
    let _guard = network_host::test_lock();
    network_host::reset();
    let ctx = context();
    let response = handle_api_request(&ctx, "GET", "/api/network/scan", &[]);
    assert_eq!(response.status, 200);
    let body = json(response);
    assert_eq!(body["networks"].as_array().unwrap().len(), 0);
    assert_eq!(body["stale"], false);
    assert_eq!(body["networkAvailable"], false);
}
