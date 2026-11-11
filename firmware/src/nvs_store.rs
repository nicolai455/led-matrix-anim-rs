use matrix_control::persistence::StorageBackend;
use matrix_core::audio::MicSettings;
use matrix_core::geometry::PanelConfig;
use serde_json::Value;
use std::sync::Arc;

pub const NVS_NAMESPACE: &str = "matrix_cfg";
pub const MAX_NVS_STRING_BYTES: usize = 4096;
pub const MAX_NVS_VALUE_BYTES: usize = 16 * 1024;

pub const KEY_PANEL_CONFIG: &str = "panel_cfg";
pub const KEY_SAVED_STATE: &str = "saved_state";
pub const KEY_SAVED_STATE_LEN: &str = "saved_len";
pub const KEY_SAVED_STATE_COUNT: &str = "saved_count";
pub const KEY_SAVED_STATE_GENERATION: &str = "saved_generation";
pub const MAX_NVS_CHUNK_BYTES: usize = 3900;
pub const KEY_MIC_SETTINGS: &str = "mic_cfg";
pub const KEY_NETWORK_CONFIG: &str = "network_cfg";

pub struct FirmwareNvs {
    #[cfg(target_os = "espidf")]
    nvs: std::sync::Mutex<esp_idf_svc::nvs::EspNvs<esp_idf_svc::nvs::NvsDefault>>,
    #[cfg(not(target_os = "espidf"))]
    mem_store: std::sync::RwLock<std::collections::HashMap<String, String>>,
}

impl FirmwareNvs {
    #[cfg(target_os = "espidf")]
    pub fn new(
        default_partition: esp_idf_svc::nvs::EspDefaultNvsPartition,
    ) -> Result<Arc<Self>, String> {
        let nvs = esp_idf_svc::nvs::EspNvs::new(default_partition, NVS_NAMESPACE, true)
            .map_err(|e| format!("Failed to open NVS namespace {NVS_NAMESPACE}: {e}"))?;

        Ok(Arc::new(Self {
            nvs: std::sync::Mutex::new(nvs),
        }))
    }

    #[cfg(not(target_os = "espidf"))]
    pub fn new() -> Result<Arc<Self>, String> {
        Ok(Arc::new(Self {
            mem_store: std::sync::RwLock::new(std::collections::HashMap::new()),
        }))
    }

    pub fn get_str(&self, key: &str) -> Result<Option<String>, String> {
        #[cfg(target_os = "espidf")]
        {
            let lock = self.nvs.lock().map_err(|_| "NVS poisoned".to_string())?;
            let len = lock.str_len(key).map_err(|e| format!("NVS get_str length failed for key '{key}': {e}"))?;
            let Some(len) = len else { return Ok(None); };
            if len > MAX_NVS_STRING_BYTES {
                return Err(format!("NVS string value exceeds {MAX_NVS_STRING_BYTES} byte read limit"));
            }
            let mut buf = vec![0u8; len];
            match lock.get_str(key, &mut buf) {
                Ok(Some(s)) => Ok(Some(s.to_string())),
                Ok(None) => Ok(None),
                Err(e) => Err(format!("NVS get_str failed for key '{key}': {e}")),
            }
        }

        #[cfg(not(target_os = "espidf"))]
        {
            let lock = self
                .mem_store
                .read()
                .map_err(|_| "Mem store poisoned".to_string())?;
            Ok(lock.get(key).cloned())
        }
    }

    pub fn set_str(&self, key: &str, value: &str) -> Result<(), String> {
        #[cfg(target_os = "espidf")]
        {
            if value.len() > MAX_NVS_STRING_BYTES {
                return Err(format!("NVS string value exceeds {MAX_NVS_STRING_BYTES} byte limit"));
            }
            let mut lock = self.nvs.lock().map_err(|_| "NVS poisoned".to_string())?;
            lock.set_str(key, value)
                .map_err(|e| format!("NVS set_str failed for key '{key}': {e}"))?;
            Ok(())
        }

        #[cfg(not(target_os = "espidf"))]
        {
            let mut lock = self
                .mem_store
                .write()
                .map_err(|_| "Mem store poisoned".to_string())?;
            lock.insert(key.to_string(), value.to_string());
            Ok(())
        }
    }

    pub fn get_u32(&self, key: &str) -> Result<Option<u32>, String> {
        #[cfg(target_os = "espidf")]
        {
            self.nvs.lock().map_err(|_| "NVS poisoned".to_string())?.get_u32(key)
                .map_err(|e| format!("NVS get_u32 failed for key '{key}': {e}"))
        }
        #[cfg(not(target_os = "espidf"))]
        {
            let lock = self.mem_store.read().map_err(|_| "Mem store poisoned".to_string())?;
            lock.get(key).map(|value| value.parse::<u32>().map_err(|e| e.to_string())).transpose()
        }
    }

    pub fn set_u32(&self, key: &str, value: u32) -> Result<(), String> {
        #[cfg(target_os = "espidf")]
        {
            self.nvs.lock().map_err(|_| "NVS poisoned".to_string())?.set_u32(key, value)
                .map_err(|e| format!("NVS set_u32 failed for key '{key}': {e}"))
        }
        #[cfg(not(target_os = "espidf"))]
        {
            self.mem_store.write().map_err(|_| "Mem store poisoned".to_string())?.insert(key.to_owned(), value.to_string());
            Ok(())
        }
    }

    pub fn remove(&self, key: &str) -> Result<(), String> {
        #[cfg(target_os = "espidf")]
        {
            let mut lock = self.nvs.lock().map_err(|_| "NVS poisoned".to_string())?;
            lock.remove(key)
                .map_err(|error| format!("NVS remove failed: {error}"))?;
            Ok(())
        }

        #[cfg(not(target_os = "espidf"))]
        {
            let mut lock = self
                .mem_store
                .write()
                .map_err(|_| "Mem store poisoned".to_string())?;
            lock.remove(key);
            Ok(())
        }
    }

    pub fn load_panel_config(&self) -> Option<PanelConfig> {
        self.get_str(KEY_PANEL_CONFIG)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
    }

    pub fn save_panel_config(&self, cfg: &PanelConfig) -> Result<(), String> {
        let json_str = serde_json::to_string(cfg).map_err(|e| e.to_string())?;
        self.set_str(KEY_PANEL_CONFIG, &json_str)
    }

    pub fn get_saved_state(&self) -> Result<Option<String>, String> {
        let serialized_state = self.get_u32(KEY_SAVED_STATE_LEN)?;
        let Some(len) = serialized_state.map(|value| value as usize) else {
            return self.get_str(KEY_SAVED_STATE);
        };
        if len > MAX_NVS_VALUE_BYTES { return Err(format!("saved runtime state exceeds {MAX_NVS_VALUE_BYTES} byte NVS bound")); }
        let generation = self.get_u32(KEY_SAVED_STATE_GENERATION)?.unwrap_or(0);
        let count = self.get_u32(KEY_SAVED_STATE_COUNT)?.ok_or("saved state chunk count missing")? as usize;
        if count != len.div_ceil(MAX_NVS_CHUNK_BYTES) {
            return Err("saved state chunk count does not match saved length".into());
        }
        let mut bytes = Vec::with_capacity(len);
        for index in 0..count {
            let key = format!("{KEY_SAVED_STATE}_{index}_{generation}");
            let chunk = self.get_str(&key)?.ok_or("saved state generation chunk missing")?;
            bytes.extend_from_slice(chunk.as_bytes());
        }
        if bytes.len() < len {
            return Err("saved state generation chunks are truncated".into());
        }
        bytes.truncate(len);
        String::from_utf8(bytes).map(Some).map_err(|e| e.to_string())
    }

    pub fn set_saved_state(&self, state: &str) -> Result<(), String> {
        if state.len() > MAX_NVS_VALUE_BYTES {
            return Err(format!("saved runtime state exceeds {MAX_NVS_VALUE_BYTES} byte NVS bound"));
        }
        let generation = self.get_u32(KEY_SAVED_STATE_GENERATION)?.unwrap_or(0);
        let next_generation = generation.wrapping_add(1);
        let mut old_count = self.get_u32(KEY_SAVED_STATE_COUNT)?.unwrap_or(0) as usize;
        let count = state.len().div_ceil(MAX_NVS_CHUNK_BYTES);
        if old_count == 0 {
            if let Some(legacy) = self.get_str(KEY_SAVED_STATE)? {
                old_count = legacy.len().div_ceil(MAX_NVS_CHUNK_BYTES);
                for (index, chunk) in legacy.as_bytes().chunks(MAX_NVS_CHUNK_BYTES).enumerate() {
                    let chunk = std::str::from_utf8(chunk).map_err(|e| e.to_string())?;
                    self.set_str(&format!("{KEY_SAVED_STATE}_{index}_{generation}"), chunk)?;
                }
                self.set_u32(KEY_SAVED_STATE_LEN, legacy.len() as u32)?;
                self.set_u32(KEY_SAVED_STATE_COUNT, old_count as u32)?;
                self.set_u32(KEY_SAVED_STATE_GENERATION, generation)?;
            }
        }
        for (index, chunk) in state.as_bytes().chunks(MAX_NVS_CHUNK_BYTES).enumerate() {
            let chunk = std::str::from_utf8(chunk).map_err(|e| e.to_string())?;
            self.set_str(&format!("{KEY_SAVED_STATE}_{index}_{next_generation}"), chunk)?;
        }
        let mut verified = Vec::with_capacity(state.len());
        for index in 0..count {
            let chunk = self.get_str(&format!("{KEY_SAVED_STATE}_{index}_{next_generation}"))?
                .ok_or("new saved state generation chunk missing")?;
            verified.extend_from_slice(chunk.as_bytes());
        }
        if verified != state.as_bytes() {
            return Err("saved state generation verification failed".into());
        }
        self.set_u32(KEY_SAVED_STATE_LEN, state.len() as u32)?;
        self.set_u32(KEY_SAVED_STATE_COUNT, count as u32)?;
        self.set_u32(KEY_SAVED_STATE_GENERATION, next_generation)?;
        for index in 0..old_count {
            self.remove(&format!("{KEY_SAVED_STATE}_{index}_{generation}"))?;
        }
        Ok(())
    }

    pub fn load_saved_state(&self) -> Option<Value> {
        self.get_saved_state().ok().flatten().and_then(|s| serde_json::from_str(&s).ok())
    }

    pub fn save_saved_state(&self, state: &Value) -> Result<(), String> {
        self.set_saved_state(&serde_json::to_string(state).map_err(|e| e.to_string())?)
    }

    pub fn clear_saved_state(&self) -> Result<(), String> {
        let generation = self.get_u32(KEY_SAVED_STATE_GENERATION)?.unwrap_or(0);
        let count = self.get_u32(KEY_SAVED_STATE_COUNT)?.unwrap_or_else(|| {
            self.get_u32(KEY_SAVED_STATE_LEN).ok().flatten()
                .map(|value| (value as usize).div_ceil(MAX_NVS_CHUNK_BYTES) as u32)
                .unwrap_or(0)
        }) as usize;
        for index in 0..count {
            self.remove(&format!("{KEY_SAVED_STATE}_{index}_{generation}"))?;
            self.remove(&format!("{KEY_SAVED_STATE}_{index}_{}", generation.wrapping_sub(1)))?;
            self.remove(&format!("{KEY_SAVED_STATE}_{index}"))?;
        }
        if let Some(legacy) = self.get_str(KEY_SAVED_STATE)? {
            for index in 0..legacy.len().div_ceil(MAX_NVS_CHUNK_BYTES) {
                self.remove(&format!("{KEY_SAVED_STATE}_{index}"))?;
            }
        }
        self.remove(KEY_SAVED_STATE_LEN)?;
        self.remove(KEY_SAVED_STATE_COUNT)?;
        self.remove(KEY_SAVED_STATE_GENERATION)?;
        self.remove(KEY_SAVED_STATE)
    }

    pub fn load_mic_settings(&self) -> Option<MicSettings> {
        self.get_str(KEY_MIC_SETTINGS)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
    }

    pub fn save_mic_settings(&self, settings: &MicSettings) -> Result<(), String> {
        let json_str = serde_json::to_string(settings).map_err(|e| e.to_string())?;
        self.set_str(KEY_MIC_SETTINGS, &json_str)
    }

    pub fn config_backend(self: &Arc<Self>) -> Box<dyn StorageBackend> {
        Box::new(NvsStorageBackend {
            nvs: Arc::clone(self),
            key: KEY_PANEL_CONFIG,
        })
    }

    pub fn state_backend(self: &Arc<Self>) -> Box<dyn StorageBackend> {
        Box::new(NvsStorageBackend {
            nvs: Arc::clone(self),
            key: KEY_SAVED_STATE,
        })
    }
}

pub struct NvsStorageBackend {
    nvs: Arc<FirmwareNvs>,
    key: &'static str,
}

impl StorageBackend for NvsStorageBackend {
    fn load(&self) -> Result<Option<String>, String> {
        if self.key == KEY_SAVED_STATE { self.nvs.get_saved_state() }
        else { self.nvs.get_str(self.key) }
    }

    fn store(&self, content: &str) -> Result<(), String> {
        if self.key == KEY_SAVED_STATE { self.nvs.set_saved_state(content) }
        else { self.nvs.set_str(self.key, content) }
    }

    fn clear(&self) -> Result<(), String> {
        if self.key == KEY_SAVED_STATE {
            self.nvs.clear_saved_state()
        } else {
            self.nvs.remove(self.key)
        }
    }
}

impl FirmwareNvs {
    /// Load validated Wi-Fi and MQTT configuration from the private NVS key.
    pub fn load_network_config(&self) -> Result<Option<crate::network_config::NetworkConfig>, String> {
        self.get_str(KEY_NETWORK_CONFIG)?
            .map(|json| crate::network_config::NetworkConfig::parse(&json))
            .transpose()
    }

    /// Validate and serialize the canonical schema before writing it to NVS.
    pub fn save_network_config(
        &self,
        config: &crate::network_config::NetworkConfig,
    ) -> Result<(), String> {
        let json = config.to_json()?;
        self.set_str(KEY_NETWORK_CONFIG, &json)
    }
}

#[cfg(test)]
mod network_config_tests {
    use super::*;
    use crate::network_config::NetworkConfig;
    use matrix_control::persistence::StateStore;

    #[test]
    fn legacy_single_key_saved_state_is_loaded_before_chunked_migration() {
        let nvs = FirmwareNvs::new().unwrap();
        let legacy = r#"{"version":1,"brightness":73,"text":"legacy-settings"}"#;
        nvs.set_str(KEY_SAVED_STATE, legacy).unwrap();

        assert_eq!(nvs.get_saved_state().unwrap().as_deref(), Some(legacy));
    }

    #[test]
    fn saved_state_round_trips_across_multiple_nvs_string_chunks() {
        let nvs = FirmwareNvs::new().unwrap();
        let value = serde_json::json!({"payload":"x".repeat(MAX_NVS_CHUNK_BYTES + 250),"schema":1});
        let text = serde_json::to_string(&value).unwrap();
        nvs.set_saved_state(&text).unwrap();
        assert_eq!(nvs.get_saved_state().unwrap().as_deref(), Some(text.as_str()));

        let small = "{\"schema\":2}";
        nvs.set_saved_state(small).unwrap();
        assert_eq!(nvs.get_saved_state().unwrap().as_deref(), Some(small));
        assert!(nvs.get_str(&format!("{KEY_SAVED_STATE}_1_2")).unwrap().is_none());
    }

    #[test]
    fn saving_named_playlists_fails_cleanly_when_nvs_bound_is_exceeded() {
        let nvs = FirmwareNvs::new().unwrap();
        let store = StateStore::with_backend(nvs.state_backend());
        let too_large = serde_json::json!({"playlistCollection":{"playlists":[],"selectedId":null},"playlistConfig":{"entries":[],"loop":false},"text":"x".repeat(MAX_NVS_VALUE_BYTES)});
        assert!(store.save(&too_large).is_err());
        assert!(store.load().is_none());
        assert!(nvs.get_saved_state().unwrap().is_none());
    }

    #[test]
    fn saved_state_chunk_bound_rejects_before_writing() {
        let nvs = FirmwareNvs::new().unwrap();
        let too_large = "x".repeat(MAX_NVS_VALUE_BYTES + 1);
        assert!(nvs.set_saved_state(&too_large).is_err());
    }

    #[test]
    fn saved_state_clear_removes_all_chunk_keys_and_metadata() {
        let nvs = FirmwareNvs::new().unwrap();
        let state = "x".repeat(MAX_NVS_CHUNK_BYTES * 2 + 100);
        nvs.set_saved_state(&state).unwrap();
        nvs.clear_saved_state().unwrap();
        assert!(nvs.get_saved_state().unwrap().is_none());
        assert!(nvs.get_str(&format!("{KEY_SAVED_STATE}_0")).unwrap().is_none());
        assert!(nvs.get_u32(KEY_SAVED_STATE_LEN).unwrap().is_none());
        assert!(nvs.get_u32(KEY_SAVED_STATE_COUNT).unwrap().is_none());
    }

    #[test]
    fn network_config_round_trips_profiles_and_mqtt_via_nvs() {
        let nvs = FirmwareNvs::new().unwrap();
        let config = NetworkConfig::parse(
            r#"{"wifiProfiles":[{"ssid":"office","password":"test-pass-123","priority":2,"hidden":true}],"mqttHost":"broker.local","mqttPort":1884,"mqttUser":"matrix","mqttPassword":"mqtt-pass"}"#,
        )
        .unwrap();
        nvs.save_network_config(&config).unwrap();
        let loaded = nvs.load_network_config().unwrap().unwrap();
        assert_eq!(loaded.prioritized_wifi_profiles().unwrap()[0].ssid, "office");
        assert!(loaded.prioritized_wifi_profiles().unwrap()[0].hidden);
        assert_eq!(loaded.mqtt_host, "broker.local");
        assert_eq!(loaded.mqtt_port, 1884);
        assert_eq!(loaded.mqtt_user, "matrix");
        assert_eq!(loaded.mqtt_password, "mqtt-pass");
    }

    #[test]
    fn invalid_persisted_network_config_is_rejected_when_loaded() {
        let nvs = FirmwareNvs::new().unwrap();
        nvs.set_str(
            KEY_NETWORK_CONFIG,
            r#"{"wifiProfiles":[{"ssid":"","password":"bad"}]}"#,
        )
        .unwrap();
        assert!(nvs.load_network_config().is_err());
    }
}
