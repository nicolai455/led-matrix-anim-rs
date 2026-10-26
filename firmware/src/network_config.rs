//! Legacy network.json compatibility. Credentials are never part of UI state.
use serde::Deserialize;
use std::io::Read;
use std::path::Path;

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConfig {
    #[serde(default)]
    pub wifi_ssid: String,
    #[serde(default)]
    pub wifi_password: String,
    #[serde(default)]
    pub mqtt_host: String,
    #[serde(default = "default_port")]
    pub mqtt_port: u16,
    #[serde(default)]
    pub mqtt_user: String,
    #[serde(default)]
    pub mqtt_password: String,
}
fn default_port() -> u16 {
    1883
}
impl NetworkConfig {
    pub fn parse(content: &str) -> Result<Self, String> {
        if content.len() > 4096 {
            return Err("network configuration exceeds 4096 bytes".into());
        }
        let config: Self =
            serde_json::from_str(content).map_err(|_| "invalid network configuration JSON")?;
        if config.wifi_ssid.len() > 32 || config.wifi_password.len() > 64 {
            return Err("Wi-Fi credentials exceed supported lengths".into());
        }
        if config.mqtt_port == 0
            || config.mqtt_host.len() > 253
            || config.mqtt_host.contains(['/', '\0', '\n', '\r', '@'])
            || config.mqtt_user.len() > 128
            || config.mqtt_password.len() > 256
        {
            return Err("invalid MQTT configuration".into());
        }
        Ok(config)
    }
    pub fn load(path: &Path) -> Result<Option<Self>, String> {
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("cannot open network configuration".into()),
        };
        let mut content = String::new();
        file.take(4097)
            .read_to_string(&mut content)
            .map_err(|_| "cannot read network configuration")?;
        Self::parse(&content).map(Some)
    }
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
        assert_eq!(config.wifi_ssid, "example");
        assert!(NetworkConfig::parse(r#"{"mqttPort":0}"#).is_err());
        assert!(NetworkConfig::parse(r#"{"mqttHost":"bad/host"}"#).is_err());
        assert!(NetworkConfig::parse(&" ".repeat(4097)).is_err());
    }
}
