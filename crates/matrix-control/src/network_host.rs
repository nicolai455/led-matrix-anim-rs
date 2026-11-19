//! In-memory Wi-Fi configuration for the host simulator.
//!
//! The host has no radio, so `/api/network` used to answer a flat 503 and the
//! whole Wi-Fi card was untestable outside a device. This module keeps the
//! firmware's request/response shape and validation rules in memory instead:
//! profiles can be added, reordered, edited and deleted, the same 5-profile and
//! 32-byte/8..=64-byte rules apply, and stored passphrases are never echoed.
//!
//! `networkAvailable` stays `false` in every response so nobody mistakes the
//! simulator for hardware. Nothing here touches the filesystem.

use serde_json::{json, Value};
use std::sync::{Mutex, OnceLock};

pub const MAX_WIFI_PROFILES: usize = 5;
pub const MAX_NETWORK_BODY_BYTES: usize = 4096;
pub const NETWORK_UNAVAILABLE_NOTICE: &str =
    "Wi-Fi configuration is simulated on the host: there is no radio, changes are not applied to hardware";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct HostProfile {
    ssid: String,
    password: Option<String>,
    priority: u8,
    hidden: bool,
}

#[derive(Clone, Debug, Default)]
struct HostNetwork {
    profiles: Vec<HostProfile>,
    fallback_ap_ssid: String,
    fallback_ap_password: String,
    mqtt_host: String,
    mqtt_port: u16,
    mqtt_user: String,
    mqtt_password: String,
}

fn store() -> &'static Mutex<HostNetwork> {
    static STORE: OnceLock<Mutex<HostNetwork>> = OnceLock::new();
    STORE.get_or_init(|| {
        Mutex::new(HostNetwork {
            mqtt_port: 1883,
            ..HostNetwork::default()
        })
    })
}

/// Test hook: forget every simulated profile.
pub fn reset() {
    if let Ok(mut network) = store().lock() {
        *network = HostNetwork {
            mqtt_port: 1883,
            ..HostNetwork::default()
        };
    }
}

/// Serialises tests that share the process-wide store. Rust runs tests in
/// threads, and every test here mutates the same static, so without this they
/// observe each other's writes (and fail intermittently in the order they happen
/// to run). Poisoning is ignored: a panicking test reports its own failure.
pub fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn redact(profile: &HostProfile) -> Value {
    json!({
        "ssid": profile.ssid,
        "priority": profile.priority,
        "hidden": profile.hidden,
        "passwordConfigured": profile.password.as_ref().is_some_and(|p| !p.is_empty()),
    })
}

/// Same shape as the firmware's redacted response, plus an explicit
/// `networkAvailable: false`.
pub fn config_json() -> Value {
    let Ok(network) = store().lock() else {
        return json!({ "error": "network state lock poisoned" });
    };
    let mut profiles: Vec<&HostProfile> = network.profiles.iter().collect();
    profiles.sort_by_key(|profile| std::cmp::Reverse(profile.priority));
    let ap_ssid = if network.fallback_ap_ssid.is_empty() {
        "JDJLIGHT".to_string()
    } else {
        network.fallback_ap_ssid.clone()
    };
    let ap_enabled = network.fallback_ap_password.len() >= 8;
    json!({
        "wifiProfiles": profiles.into_iter().map(redact).collect::<Vec<_>>(),
        "fallbackApSsid": ap_ssid,
        "fallbackApEnabled": ap_enabled,
        "fallbackApPasswordConfigured": ap_enabled,
        "mqttHost": network.mqtt_host,
        "mqttPort": network.mqtt_port,
        "mqttUser": network.mqtt_user,
        "mqttPasswordConfigured": !network.mqtt_password.is_empty(),
        "networkAvailable": false,
        "notice": NETWORK_UNAVAILABLE_NOTICE,
    })
}

fn invalid(message: &str) -> (u16, String) {
    (400, message.to_string())
}

/// Applies one `POST /api/network` body. Returns `(status, json)`.
pub fn apply(body: &[u8]) -> (u16, Value) {
    if body.len() > MAX_NETWORK_BODY_BYTES {
        return (
            413,
            json!({ "error": format!("network configuration exceeds {MAX_NETWORK_BODY_BYTES} bytes") }),
        );
    }
    let incoming: Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(_) => return (400, json!({ "error": "invalid network configuration JSON" })),
    };
    let Some(object) = incoming.as_object() else {
        return (400, json!({ "error": "network configuration must be an object" }));
    };
    const FIELDS: [&str; 8] = [
        "wifiProfiles",
        "fallbackApSsid",
        "fallbackApPassword",
        "clearFallbackApPassword",
        "mqttHost",
        "mqttPort",
        "mqttUser",
        "mqttPassword",
    ];
    if object.keys().any(|key| !FIELDS.contains(&key.as_str())) {
        return (400, json!({ "error": "unknown network configuration field" }));
    }

    let mut next = match store().lock() {
        Ok(network) => network.clone(),
        Err(_) => return (500, json!({ "error": "network state lock poisoned" })),
    };

    if let Some(profiles) = object.get("wifiProfiles") {
        let Some(list) = profiles.as_array() else {
            return (400, json!({ "error": "wifiProfiles must be an array" }));
        };
        if list.len() > MAX_WIFI_PROFILES {
            return (
                400,
                json!({ "error": format!("at most {MAX_WIFI_PROFILES} Wi-Fi profiles are supported") }),
            );
        }
        let mut parsed: Vec<HostProfile> = Vec::with_capacity(list.len());
        for (index, entry) in list.iter().enumerate() {
            let Some(profile) = entry.as_object() else {
                let (status, message) = invalid(&format!("profile {index} must be an object"));
                return (status, json!({ "error": message }));
            };
            let ssid = profile.get("ssid").and_then(Value::as_str).unwrap_or("");
            if ssid.is_empty() || ssid.len() > 32 {
                let (status, message) = invalid(&format!(
                    "profile {index} ssid must be 1..=32 UTF-8 bytes"
                ));
                return (status, json!({ "error": message }));
            }
            if parsed.iter().any(|existing| existing.ssid == ssid) {
                let (status, message) = invalid(&format!("duplicate ssid '{ssid}'"));
                return (status, json!({ "error": message }));
            }
            let password = profile.get("password").and_then(Value::as_str);
            if let Some(password) = password {
                // An explicit empty password means "open network".
                if !password.is_empty() && !(8..=64).contains(&password.len()) {
                    let (status, message) = invalid(&format!(
                        "profile {index} password must be 8..=64 bytes or empty for an open network"
                    ));
                    return (status, json!({ "error": message }));
                }
            }
            // `keepPassword` retains the stored secret for an edit round-trip.
            let keep = profile
                .get("keepPassword")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let stored = parsed
                .iter()
                .find(|existing| existing.ssid == ssid)
                .and_then(|existing| existing.password.clone());
            let previous = next.profiles.iter().find(|existing| existing.ssid == ssid);
            let resolved = match (password, keep) {
                (Some(""), _) => None,
                (Some(password), _) => Some(password.to_string()),
                (None, true) => previous.and_then(|existing| existing.password.clone()).or(stored),
                (None, false) => None,
            };
            parsed.push(HostProfile {
                ssid: ssid.to_string(),
                password: resolved,
                priority: profile
                    .get("priority")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
                    .min(255) as u8,
                hidden: profile
                    .get("hidden")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            });
        }
        parsed.sort_by_key(|profile| std::cmp::Reverse(profile.priority));
        next.profiles = parsed;
    }

    if let Some(ssid) = object.get("fallbackApSsid").and_then(Value::as_str) {
        if ssid.len() > 32 {
            return (400, json!({ "error": "fallbackApSsid must be at most 32 bytes" }));
        }
        next.fallback_ap_ssid = ssid.to_string();
    }
    if object
        .get("clearFallbackApPassword")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        next.fallback_ap_password.clear();
    }
    if let Some(password) = object.get("fallbackApPassword").and_then(Value::as_str) {
        if !password.is_empty() && !(8..=64).contains(&password.len()) {
            return (
                400,
                json!({ "error": "fallbackApPassword must be 8..=64 bytes" }),
            );
        }
        if !password.is_empty() {
            next.fallback_ap_password = password.to_string();
        }
    }
    if let Some(host) = object.get("mqttHost").and_then(Value::as_str) {
        next.mqtt_host = host.to_string();
    }
    if let Some(port) = object.get("mqttPort").and_then(Value::as_u64) {
        if port > u16::MAX as u64 {
            return (400, json!({ "error": "mqttPort must fit in 16 bits" }));
        }
        next.mqtt_port = port as u16;
    }
    if let Some(user) = object.get("mqttUser").and_then(Value::as_str) {
        next.mqtt_user = user.to_string();
    }
    if let Some(password) = object.get("mqttPassword").and_then(Value::as_str) {
        if !password.is_empty() {
            next.mqtt_password = password.to_string();
        }
    }

    if let Ok(mut network) = store().lock() {
        *network = next;
    }
    (200, json!({ "ok": true, "config": config_json() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset_store() {
        reset();
    }

    #[test]
    fn profiles_are_sorted_deduplicated_and_redacted() {
        let _guard = test_lock();
        reset_store();
        let (status, value) = apply(
            br#"{"wifiProfiles":[
                {"ssid":"home","password":"supersecret","priority":5,"hidden":false},
                {"ssid":"garage","password":"","priority":9}
            ]}"#,
        );
        assert_eq!(status, 200, "{value}");
        let profiles = value["config"]["wifiProfiles"].as_array().unwrap();
        assert_eq!(profiles[0]["ssid"], "garage");
        assert_eq!(profiles[1]["ssid"], "home");
        assert_eq!(profiles[1]["passwordConfigured"], true);
        assert_eq!(profiles[0]["passwordConfigured"], false);
        // The secret must never leave the process.
        assert!(!value.to_string().contains("supersecret"));
        assert_eq!(value["config"]["networkAvailable"], false);
    }

    #[test]
    fn validation_matches_the_firmware_rules() {
        let _guard = test_lock();
        reset_store();
        for (body, expected) in [
            (r#"{"wifiProfiles":[{"ssid":""}]}"#, 400),
            (r#"{"wifiProfiles":[{"ssid":"a","password":"short"}]}"#, 400),
            (r#"{"wifiProfiles":[{"ssid":"a"},{"ssid":"a"}]}"#, 400),
            (r#"{"unknownField":1}"#, 400),
            ("[]", 400),
            (r#"{"wifiProfiles":[]}"#, 200),
        ] {
            assert_eq!(apply(body.as_bytes()).0, expected, "body: {body}");
        }
        let five: String = format!(
            r#"{{"wifiProfiles":[{}]}}"#,
            (0..5)
                .map(|index| format!(r#"{{"ssid":"net{index}"}}"#))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert_eq!(apply(five.as_bytes()).0, 200);
        let six: String = format!(
            r#"{{"wifiProfiles":[{}]}}"#,
            (0..6)
                .map(|index| format!(r#"{{"ssid":"net{index}"}}"#))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert_eq!(apply(six.as_bytes()).0, 400);
    }
}
