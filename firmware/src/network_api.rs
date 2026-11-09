//! Secret-redacted network configuration API helper.
//! This module deliberately does not expose stored passphrases.
use crate::network_config::{NetworkConfig, WifiProfile, MAX_NETWORK_CONFIG_BYTES, MAX_WIFI_PROFILES};
use crate::nvs_store::FirmwareNvs;
use serde_json::{json, Value};

pub const NETWORK_BODY_LIMIT: usize = 4096;

pub fn redacted_config(config: &NetworkConfig) -> Result<Value, String> {
    let profiles = config.prioritized_wifi_profiles()?.iter().map(|profile| json!({
        "ssid": profile.ssid,
        "priority": profile.priority,
        "hidden": profile.hidden,
        "passwordConfigured": profile.password.as_ref().is_some_and(|p| !p.is_empty()),
    })).collect::<Vec<_>>();
    let ap = config.fallback_ap_config()?;
    let ap_ssid = ap.as_ref().map(|v| v.ssid.as_str()).unwrap_or_else(|| {
        if config.fallback_ap_ssid.is_empty() { "JDJLIGHT" } else { config.fallback_ap_ssid.as_str() }
    });
    Ok(json!({
        "wifiProfiles": profiles,
        "fallbackApSsid": ap_ssid,
        "fallbackApEnabled": ap.is_some(),
        "fallbackApPasswordConfigured": ap.is_some(),
        "mqttHost": config.mqtt_host,
        "mqttPort": config.mqtt_port,
        "mqttUser": config.mqtt_user,
        "mqttPasswordConfigured": !config.mqtt_password.is_empty(),
    }))
}

pub fn get_config(nvs: &FirmwareNvs) -> Result<Value, String> {
    redacted_config(&nvs.load_network_config()?.unwrap_or_default())
}

/// Validate the full candidate before a single atomic persistence write.
pub fn update_config(nvs: &FirmwareNvs, body: &[u8]) -> Result<Value, String> {
    if body.len() > NETWORK_BODY_LIMIT || body.len() > MAX_NETWORK_CONFIG_BYTES {
        return Err("network configuration exceeds 4096 bytes".into());
    }
    let incoming: Value = serde_json::from_slice(body).map_err(|_| "invalid network configuration JSON")?;
    let obj = incoming.as_object().ok_or("network configuration must be an object")?;
    let fields = ["wifiProfiles", "fallbackApSsid", "fallbackApPassword", "clearFallbackApPassword", "mqttHost", "mqttPort", "mqttUser", "mqttPassword"];
    if obj.keys().any(|key| !fields.contains(&key.as_str())) { return Err("unknown network configuration field".into()); }

    let old = nvs.load_network_config()?.unwrap_or_default();
    let old_profiles = old.prioritized_wifi_profiles()?;
    let mut next = old.clone();
    let mut stations_after = old_profiles.len();
    if let Some(values) = obj.get("wifiProfiles") {
        let values = values.as_array().ok_or("wifiProfiles must be an array")?;
        if values.len() > MAX_WIFI_PROFILES { return Err(format!("at most {MAX_WIFI_PROFILES} Wi-Fi profiles are supported")); }
        let mut profiles = Vec::with_capacity(values.len());
        for value in values {
            let mut fields = value.as_object().cloned().ok_or("each Wi-Fi profile must be an object")?;
            let keep_password = fields.remove("keepPassword").and_then(|v| v.as_bool()).unwrap_or(false);
            let clear_password = fields.remove("clearPassword").and_then(|v| v.as_bool()).unwrap_or(false);
            fields.remove("passwordConfigured");
            let mut profile: WifiProfile = serde_json::from_value(Value::Object(fields)).map_err(|_| "invalid Wi-Fi profile")?;
            if clear_password { profile.password = None; }
            else if keep_password && profile.password.is_none() {
                profile.password = old_profiles.iter().find(|old| old.ssid == profile.ssid).and_then(|old| old.password.clone());
            }
            profiles.push(profile);
        }
        stations_after = profiles.len();
        next.wifi_profiles = Some(profiles);
        next.wifi_ssid.clear();
        next.wifi_password.clear();
    }
    if let Some(value) = obj.get("fallbackApSsid") { next.fallback_ap_ssid = value.as_str().ok_or("fallbackApSsid must be a string")?.to_owned(); }
    if obj.get("clearFallbackApPassword").and_then(Value::as_bool) == Some(true) { next.fallback_ap_password.clear(); }
    else if let Some(value) = obj.get("fallbackApPassword") {
        let value = value.as_str().ok_or("fallbackApPassword must be a string")?;
        if !value.is_empty() { next.fallback_ap_password = value.to_owned(); }
    }
    if let Some(value) = obj.get("mqttHost") { next.mqtt_host = value.as_str().ok_or("mqttHost must be a string")?.to_owned(); }
    if let Some(value) = obj.get("mqttPort") { next.mqtt_port = value.as_u64().and_then(|v| u16::try_from(v).ok()).ok_or("mqttPort must be a valid port")?; }
    if let Some(value) = obj.get("mqttUser") { next.mqtt_user = value.as_str().ok_or("mqttUser must be a string")?.to_owned(); }
    if let Some(value) = obj.get("mqttPassword") { next.mqtt_password = value.as_str().ok_or("mqttPassword must be a string")?.to_owned(); }

    if stations_after == 0 && next.fallback_ap_password.is_empty() {
        return Err("cannot remove the last station profile while the fallback AP is disabled; configure an AP WPA2 passphrase first".into());
    }
    let validated = NetworkConfig::parse(&next.to_json()?)?;
    nvs.save_network_config(&validated)?;
    redacted_config(&validated)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn store() -> std::sync::Arc<FirmwareNvs> { FirmwareNvs::new().unwrap() }
    #[test]
    fn response_redacts_wifi_ap_and_mqtt_secrets() {
        let nvs = store();
        let cfg = NetworkConfig::parse(r#"{"wifiProfiles":[{"ssid":"test-ssid","password":"fake-wifi-pass"}],"fallbackApPassword":"fake-ap-pass","mqttPassword":"fake-mqtt-pass"}"#).unwrap();
        nvs.save_network_config(&cfg).unwrap();
        let response = get_config(&nvs).unwrap().to_string();
        for secret in ["fake-wifi-pass", "fake-ap-pass", "fake-mqtt-pass"] { assert!(!response.contains(secret)); }
        assert!(response.contains("test-ssid"));
    }
    #[test]
    fn response_reports_disabled_saved_ap_ssid_without_passphrase() {
        let nvs = store();
        let cfg = NetworkConfig::parse(r#"{"fallbackApSsid":"test-ap-name"}"#).unwrap();
        nvs.save_network_config(&cfg).unwrap();
        let response = get_config(&nvs).unwrap();
        assert_eq!(response["fallbackApSsid"], "test-ap-name");
        assert_eq!(response["fallbackApEnabled"], false);
        assert_eq!(response["fallbackApPasswordConfigured"], false);
    }

    #[test]
    fn config_edit_retains_password_only_when_explicitly_requested() {
        let nvs = store();
        let cfg = NetworkConfig::parse(r#"{"wifiProfiles":[{"ssid":"test-ssid","password":"fake-wifi-pass"}],"fallbackApPassword":"fake-ap-pass"}"#).unwrap();
        nvs.save_network_config(&cfg).unwrap();
        update_config(&nvs, br#"{"wifiProfiles":[{"ssid":"test-ssid","priority":7,"hidden":true,"keepPassword":true}]}"#).unwrap();
        let saved = nvs.load_network_config().unwrap().unwrap();
        assert_eq!(saved.prioritized_wifi_profiles().unwrap()[0].password.as_deref(), Some("fake-wifi-pass"));
        assert_eq!(saved.fallback_ap_password, "fake-ap-pass");
        let before = saved.to_json().unwrap();
        assert!(update_config(&nvs, br#"{"wifiProfiles":[{"ssid":"","password":"bad"}]}"#).is_err());
        assert_eq!(before, nvs.load_network_config().unwrap().unwrap().to_json().unwrap());
        let output = update_config(&nvs, br#"{"wifiProfiles":[],"fallbackApSsid":"my-ap"}"#).unwrap();
        assert!(nvs.load_network_config().unwrap().unwrap().prioritized_wifi_profiles().unwrap().is_empty());
        assert_eq!(output["fallbackApEnabled"], true);
    }
    #[test]
    fn explicit_clear_removes_credentials() {
        let nvs = store();
        let cfg = NetworkConfig::parse(r#"{"wifiProfiles":[{"ssid":"test-ssid","password":"fake-wifi-pass"}],"fallbackApPassword":"fake-ap-pass"}"#).unwrap();
        nvs.save_network_config(&cfg).unwrap();
        update_config(&nvs, br#"{"wifiProfiles":[{"ssid":"test-ssid","clearPassword":true}],"clearFallbackApPassword":true,"fallbackApSsid":""}"#).unwrap();
        let saved = nvs.load_network_config().unwrap().unwrap();
        assert!(saved.prioritized_wifi_profiles().unwrap()[0].password.is_none());
        assert!(saved.fallback_ap_password.is_empty());
    }
}
