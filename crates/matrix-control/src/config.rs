pub use matrix_core::geometry::{
    PanelConfig, PanelDirection, PanelStartCorner, DEFAULT_MAX_MILLIAMPS, MAX_MILLIAMPS_LIMIT,
    MIN_MILLIAMPS_LIMIT, NUM_PANELS, PANEL_SIZE, TOTAL_SIZE,
};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

pub use crate::persistence::{FsStorageBackend, StorageBackend};

pub type ConfigListener = Arc<dyn Fn(&PanelConfig, &PanelConfig) + Send + Sync>;

/// Shared panel configuration store with atomic validation, listener notification, and persistence.
pub struct ConfigStore {
    backend: Option<Box<dyn StorageBackend>>,
    config: RwLock<PanelConfig>,
    listeners: RwLock<Vec<ConfigListener>>,
}

impl ConfigStore {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self::try_new(path).unwrap_or_else(|_| Self {
            backend: None,
            config: RwLock::new(PanelConfig::default()),
            listeners: RwLock::new(Vec::new()),
        })
    }

    pub fn try_new(path: Option<PathBuf>) -> Result<Self, String> {
        if let Some(p) = path {
            let backend = FsStorageBackend::new(p);
            let mut config = PanelConfig::default();
            if let Some(content) = backend.load()? {
                let loaded: PanelConfig = serde_json::from_str(&content)
                    .map_err(|e| format!("invalid panel config JSON: {e}"))?;
                loaded
                    .validate()
                    .map_err(|e| format!("invalid panel configuration: {e}"))?;
                config = loaded;
            }
            Ok(Self {
                backend: Some(Box::new(backend)),
                config: RwLock::new(config),
                listeners: RwLock::new(Vec::new()),
            })
        } else {
            Ok(Self {
                backend: None,
                config: RwLock::new(PanelConfig::default()),
                listeners: RwLock::new(Vec::new()),
            })
        }
    }

    pub fn with_backend(
        backend: Box<dyn StorageBackend>,
        default_config: Option<PanelConfig>,
    ) -> Self {
        let mut config = default_config.unwrap_or_default();
        if let Ok(Some(content)) = backend.load() {
            if let Ok(loaded) = serde_json::from_str::<PanelConfig>(&content) {
                if loaded.validate().is_ok() {
                    config = loaded;
                }
            }
        }
        Self {
            backend: Some(backend),
            config: RwLock::new(config),
            listeners: RwLock::new(Vec::new()),
        }
    }

    pub fn get(&self) -> PanelConfig {
        self.config.read().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn to_json(&self) -> Value {
        serde_json::to_value(self.get()).unwrap_or(Value::Null)
    }

    pub fn add_listener<F>(&self, listener: F)
    where
        F: Fn(&PanelConfig, &PanelConfig) + Send + Sync + 'static,
    {
        if let Ok(mut lock) = self.listeners.write() {
            lock.push(Arc::new(listener));
        }
    }

    pub fn update_from_json(&self, val: &Value) -> Result<PanelConfig, String> {
        let object = val.as_object().ok_or("invalid config object")?;
        let mut merged = self.to_json();
        let target = merged.as_object_mut().ok_or("invalid current config")?;
        for (key, value) in object {
            if !target.contains_key(key) {
                return Err(format!("unknown configuration field: {key}"));
            }
            target.insert(key.clone(), value.clone());
        }
        let candidate: PanelConfig =
            serde_json::from_value(merged).map_err(|e| format!("invalid configuration: {e}"))?;
        candidate.validate()?;
        self.save(&candidate)?;
        let old = self.get();
        *self
            .config
            .write()
            .map_err(|_| "configuration lock poisoned")? = candidate.clone();
        if let Ok(listeners) = self.listeners.read() {
            for listener in listeners.iter() {
                listener(&old, &candidate);
            }
        }
        Ok(candidate)
    }

    pub fn save(&self, config: &PanelConfig) -> Result<(), String> {
        let json = serde_json::to_string_pretty(config)
            .map_err(|e| format!("serialization error: {e}"))?;
        if let Some(ref b) = self.backend {
            b.store(&json)?;
        }
        Ok(())
    }
}
