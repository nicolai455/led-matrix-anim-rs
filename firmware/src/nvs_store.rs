use matrix_control::persistence::StorageBackend;
use matrix_core::audio::MicSettings;
use matrix_core::geometry::PanelConfig;
use serde_json::Value;
use std::sync::Arc;

pub const NVS_NAMESPACE: &str = "matrix_cfg";
pub const KEY_PANEL_CONFIG: &str = "panel_cfg";
pub const KEY_SAVED_STATE: &str = "saved_state";
pub const KEY_MIC_SETTINGS: &str = "mic_cfg";

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
            let mut buf = vec![0u8; 4096];
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

    pub fn load_saved_state_json(&self) -> Option<String> {
        self.get_str(KEY_SAVED_STATE).ok().flatten()
    }

    pub fn load_saved_state(&self) -> Option<Value> {
        self.get_str(KEY_SAVED_STATE)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
    }

    pub fn save_saved_state(&self, state: &Value) -> Result<(), String> {
        let json_str = serde_json::to_string(state).map_err(|e| e.to_string())?;
        self.set_str(KEY_SAVED_STATE, &json_str)
    }

    pub fn clear_saved_state(&self) -> Result<(), String> {
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
        self.nvs.get_str(self.key)
    }

    fn store(&self, content: &str) -> Result<(), String> {
        self.nvs.set_str(self.key, content)
    }

    fn clear(&self) -> Result<(), String> {
        self.nvs.remove(self.key)
    }
}
