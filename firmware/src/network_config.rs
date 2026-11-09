//! Validated network configuration and legacy `network.json` migration.
//!
//! Serialized fields use camelCase: `wifiProfiles` is a prioritized array of
//! `{ssid, password?, priority?, hidden?}` profiles; legacy `wifiSsid` and
//! `wifiPassword` are accepted on input and normalized to a one-entry array.
//! MQTT fields (`mqttHost`, `mqttPort`, `mqttUser`, `mqttPassword`) are retained.
//! AP credentials are optional, but `fallbackApPassword` must be explicitly
//! provisioned (8..=64 bytes) before firmware will enable the fallback AP.
//! There is intentionally no shared/default AP password: without explicitly
//! provisioned AP credentials, a fresh device has no Wi-Fi bootstrap AP. This
//! limitation is safer than shipping a universal production credential; it
//! requires physical/provisioning access to supply a per-device password first.
use crate::net::wifi::{FallbackApConfig, FALLBACK_AP_SSID};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;

pub const MAX_WIFI_PROFILES: usize = 5;
pub const MAX_NETWORK_CONFIG_BYTES: usize = 4096;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WifiProfile {
    pub ssid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default)]
    pub priority: u8,
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConfig {
    /// Legacy single-profile fields; accepted for migration and omitted on save.
    #[serde(default, skip_serializing)]
    pub wifi_ssid: String,
    #[serde(default, skip_serializing)]
    pub wifi_password: String,
    /// `None` means use legacy fields if present; an empty list disables STA.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wifi_profiles: Option<Vec<WifiProfile>>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fallback_ap_ssid: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fallback_ap_password: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mqtt_host: String,
    #[serde(default = "default_port")]
    pub mqtt_port: u16,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mqtt_user: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mqtt_password: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            wifi_ssid: String::new(),
            wifi_password: String::new(),
            wifi_profiles: Some(Vec::new()),
            fallback_ap_ssid: String::new(),
            fallback_ap_password: String::new(),
            mqtt_host: String::new(),
            mqtt_port: default_port(),
            mqtt_user: String::new(),
            mqtt_password: String::new(),
        }
    }
}

fn default_port() -> u16 {
    1883
}

impl NetworkConfig {
    /// Parse, validate, and normalize JSON. Profile order is descending by
    /// priority; equal-priority entries retain their input order.
    pub fn parse(content: &str) -> Result<Self, String> {
        if content.len() > MAX_NETWORK_CONFIG_BYTES {
            return Err("network configuration exceeds 4096 bytes".into());
        }
        let mut config: Self = serde_json::from_str(content)
            .map_err(|_| "invalid network configuration JSON".to_string())?;
        if config.mqtt_port == 0
            || config.mqtt_host.len() > 253
            || config.mqtt_host.contains(['/', '\0', '\n', '\r', '@'])
            || config.mqtt_user.len() > 128
            || config.mqtt_password.len() > 256
        {
            return Err("invalid MQTT configuration".into());
        }

        let mut profiles = match config.wifi_profiles.take() {
            Some(profiles) => profiles,
            None => {
                if config.wifi_ssid.is_empty() {
                    if !config.wifi_password.is_empty() {
                        return Err("legacy Wi-Fi password supplied without an SSID".into());
                    }
                    Vec::new()
                } else {
                    vec![WifiProfile {
                        ssid: std::mem::take(&mut config.wifi_ssid),
                        password: (!config.wifi_password.is_empty())
                            .then(|| std::mem::take(&mut config.wifi_password)),
                        priority: 0,
                        hidden: false,
                    }]
                }
            }
        };
        if profiles.len() > MAX_WIFI_PROFILES {
            return Err(format!("at most {MAX_WIFI_PROFILES} Wi-Fi profiles are supported"));
        }
        for profile in &mut profiles {
            validate_profile(profile)?;
            if profile.password.as_deref() == Some("") {
                profile.password = None;
            }
        }
        profiles.sort_by_key(|profile| std::cmp::Reverse(profile.priority));
        config.wifi_profiles = Some(profiles);
        config.wifi_ssid.clear();
        config.wifi_password.clear();
        config.fallback_ap_config()?;
        Ok(config)
    }

    /// Return profiles in connection order. `hidden` is retained as metadata;
    /// ESP-IDF has no hidden field in `ClientConfiguration`, so the driver uses
    /// the explicit SSID with a full scan (rather than an SSID-less scan).
    pub fn prioritized_wifi_profiles(&self) -> Result<Vec<WifiProfile>, String> {
        let mut profiles = match &self.wifi_profiles {
            Some(profiles) => profiles.clone(),
            None if !self.wifi_ssid.is_empty() => vec![WifiProfile {
                ssid: self.wifi_ssid.clone(),
                password: (!self.wifi_password.is_empty()).then(|| self.wifi_password.clone()),
                priority: 0,
                hidden: false,
            }],
            None => Vec::new(),
        };
        if profiles.len() > MAX_WIFI_PROFILES {
            return Err(format!("at most {MAX_WIFI_PROFILES} Wi-Fi profiles are supported"));
        }
        for profile in &mut profiles {
            validate_profile(profile)?;
            if profile.password.as_deref() == Some("") {
                profile.password = None;
            }
        }
        profiles.sort_by_key(|profile| std::cmp::Reverse(profile.priority));
        Ok(profiles)
    }

    /// Return a validated WPA2 AP configuration only when a password is
    /// provisioned. `fallbackApSsid` defaults to the public, non-secret SSID
    /// constant; an AP password never has a firmware default.
    pub fn fallback_ap_config(&self) -> Result<Option<FallbackApConfig>, String> {
        if self.fallback_ap_password.is_empty() {
            // An optional saved AP SSID is not active until private credentials
            // exist, but still validate it now to avoid persisting bad config.
            if !self.fallback_ap_ssid.is_empty()
                && (self.fallback_ap_ssid.len() > 32 || self.fallback_ap_ssid.contains('\0'))
            {
                return Err("fallback AP SSID must be 1..=32 bytes".into());
            }
            return Ok(None);
        }
        let config = FallbackApConfig {
            ssid: if self.fallback_ap_ssid.is_empty() {
                FALLBACK_AP_SSID.to_string()
            } else {
                self.fallback_ap_ssid.clone()
            },
            password: self.fallback_ap_password.clone(),
            ..FallbackApConfig::default()
        };
        config.validate().map_err(str::to_string)?;
        Ok(Some(config))
    }

    /// Serialize the canonical schema after validation. Legacy Wi-Fi keys are
    /// never emitted; profile and MQTT settings remain intact.
    pub fn to_json(&self) -> Result<String, String> {
        let profiles = self.prioritized_wifi_profiles()?;
        let mut canonical = self.clone();
        canonical.wifi_profiles = Some(profiles);
        canonical.wifi_ssid.clear();
        canonical.wifi_password.clear();
        canonical.fallback_ap_config()?;
        let json = serde_json::to_string(&canonical).map_err(|_| "cannot serialize network configuration")?;
        if json.len() > MAX_NETWORK_CONFIG_BYTES {
            return Err("network configuration exceeds 4096 bytes".into());
        }
        Ok(json)
    }

    pub fn load(path: &Path) -> Result<Option<Self>, String> {
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("cannot open network configuration".into()),
        };
        let mut content = String::new();
        file.take((MAX_NETWORK_CONFIG_BYTES + 1) as u64)
            .read_to_string(&mut content)
            .map_err(|_| "cannot read network configuration")?;
        Self::parse(&content).map(Some)
    }
}

fn validate_profile(profile: &WifiProfile) -> Result<(), String> {
    if profile.ssid.trim().is_empty() || profile.ssid.len() > 32 || profile.ssid.contains('\0') {
        return Err("Wi-Fi SSID must be 1..=32 bytes and not blank".into());
    }
    if let Some(password) = profile.password.as_deref() {
        if !password.is_empty() && !(8..=64).contains(&password.len()) {
            return Err("Wi-Fi password must be empty or 8..=64 bytes".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_schema_and_safe_defaults() {
        let config = NetworkConfig::parse(
            r#"{"wifiSsid":"example","wifiPassword":"test-only","mqttHost":"localhost"}"#,
        )
        .unwrap();
        assert_eq!(config.mqtt_port, 1883);
        assert_eq!(config.prioritized_wifi_profiles().unwrap()[0].ssid, "example");
        assert!(NetworkConfig::parse(r#"{"mqttPort":0}"#).is_err());
        assert!(NetworkConfig::parse(r#"{"mqttHost":"bad/host"}"#).is_err());
        assert!(NetworkConfig::parse(&" ".repeat(MAX_NETWORK_CONFIG_BYTES + 1)).is_err());
    }

    #[test]
    fn prioritized_profiles_are_validated_and_sorted_with_hidden_metadata() {
        let config = NetworkConfig::parse(
            r#"{"wifiProfiles":[{"ssid":"later","password":"password2","priority":20},{"ssid":"hidden-net","password":"password1","priority":10,"hidden":true}],"mqttHost":"broker","mqttPort":1884,"mqttUser":"mqtt-user","mqttPassword":"mqtt-pass"}"#,
        )
        .unwrap();
        let profiles = config.prioritized_wifi_profiles().unwrap();
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].ssid, "later");
        assert!(!profiles[0].hidden);
        assert_eq!(profiles[1].ssid, "hidden-net");
        assert!(profiles[1].hidden);
        assert_eq!(config.mqtt_host, "broker");
        assert_eq!(config.mqtt_port, 1884);
        assert_eq!(config.mqtt_user, "mqtt-user");
        assert_eq!(config.mqtt_password, "mqtt-pass");
    }

    #[test]
    fn legacy_wifi_fields_migrate_to_a_single_profile() {
        let config = NetworkConfig::parse(
            r#"{"wifiSsid":"legacy-net","wifiPassword":"legacy-pass","mqttHost":"broker"}"#,
        )
        .unwrap();
        let profiles = config.prioritized_wifi_profiles().unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].ssid, "legacy-net");
        assert_eq!(profiles[0].password.as_deref(), Some("legacy-pass"));
        assert_eq!(profiles[0].priority, 0);
        assert!(!profiles[0].hidden);
        let canonical = config.to_json().unwrap();
        assert!(canonical.contains("\"wifiProfiles\""));
        assert!(!canonical.contains("wifiSsid"));
    }

    #[test]
    fn invalid_profile_count_and_credentials_are_rejected() {
        assert!(NetworkConfig::parse(r#"{"wifiProfiles":[]}"#).is_ok());
        assert!(NetworkConfig::parse(
            r#"{"wifiProfiles":[{"ssid":"","password":"password1"}]}"#
        )
        .is_err());
        assert!(NetworkConfig::parse(
            r#"{"wifiProfiles":[{"ssid":"network","password":"short"}]}"#
        )
        .is_err());

        let too_many = (0..=MAX_WIFI_PROFILES)
            .map(|i| format!(r#"{{"ssid":"net{i}","password":"password{i}"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        assert!(NetworkConfig::parse(&format!(r#"{{"wifiProfiles":[{too_many}]}}"#)).is_err());
    }

    #[test]
    fn fallback_ap_ssid_without_password_is_retained_but_does_not_enable_ap() {
        let config = NetworkConfig::parse(r#"{"wifiProfiles":[{"ssid":"test-sta"}],"fallbackApSsid":"my-device"}"#).unwrap();
        assert_eq!(config.fallback_ap_ssid, "my-device");
        assert!(config.fallback_ap_config().unwrap().is_none());
    }

    #[test]
    fn fallback_ap_requires_explicit_password() {
        assert!(NetworkConfig::parse(r#"{"fallbackApSsid":"my-device"}"#).unwrap().fallback_ap_config().unwrap().is_none());
        let config = NetworkConfig::parse(r#"{"fallbackApPassword":"bootstrap-pass"}"#).unwrap();
        assert!(config.fallback_ap_config().unwrap().is_some());
        assert!(NetworkConfig::default().fallback_ap_config().unwrap().is_none());
    }

    #[test]
    fn serialized_config_retains_mqtt_fields_and_profile_order() {
        let config = NetworkConfig::parse(
            r#"{"wifiProfiles":[{"ssid":"lower","password":"password1","priority":1},{"ssid":"higher","password":"password2","priority":2}],"mqttHost":"mqtt.local","mqttPort":1885,"mqttUser":"client","mqttPassword":"mqtt-secret"}"#,
        )
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&config.to_json().unwrap()).unwrap();
        assert_eq!(value["wifiProfiles"][0]["ssid"], "higher");
        assert_eq!(value["mqttHost"], "mqtt.local");
        assert_eq!(value["mqttPort"], 1885);
        assert_eq!(value["mqttUser"], "client");
        assert_eq!(value["mqttPassword"], "mqtt-secret");
    }
}
