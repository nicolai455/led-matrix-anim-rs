use serde_json::{json, Value};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::RwLock;

/// Maximum size allowed for loading storage files (1 MB).
pub const MAX_STORAGE_LOAD_BYTES: u64 = 1024 * 1024;

static TEMP_FILE_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Storage abstraction allowing either filesystem, NVS, or mock in-memory persistence.
pub trait StorageBackend: Send + Sync {
    fn load(&self) -> Result<Option<String>, String>;
    fn store(&self, content: &str) -> Result<(), String>;
    fn clear(&self) -> Result<(), String> {
        Ok(())
    }
}

/// Filesystem-backed storage implementation using standard I/O with atomic store and bounded load.
pub struct FsStorageBackend {
    path: PathBuf,
}

impl FsStorageBackend {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl StorageBackend for FsStorageBackend {
    fn load(&self) -> Result<Option<String>, String> {
        if !self.path.exists() {
            return Ok(None);
        }

        let metadata = fs::metadata(&self.path)
            .map_err(|e| format!("failed to read metadata for {}: {e}", self.path.display()))?;
        if metadata.len() > MAX_STORAGE_LOAD_BYTES {
            return Err(format!(
                "file {} exceeds maximum size bound ({} > {} bytes)",
                self.path.display(),
                metadata.len(),
                MAX_STORAGE_LOAD_BYTES
            ));
        }

        let file = fs::File::open(&self.path)
            .map_err(|e| format!("failed to open file {}: {e}", self.path.display()))?;
        let mut buf = String::with_capacity(metadata.len().min(MAX_STORAGE_LOAD_BYTES) as usize);
        file.take(MAX_STORAGE_LOAD_BYTES + 1)
            .read_to_string(&mut buf)
            .map_err(|e| format!("failed to read file {}: {e}", self.path.display()))?;

        if buf.len() as u64 > MAX_STORAGE_LOAD_BYTES {
            return Err(format!(
                "file {} exceeds maximum size bound of {} bytes",
                self.path.display(),
                MAX_STORAGE_LOAD_BYTES
            ));
        }

        Ok(Some(buf))
    }

    fn store(&self, content: &str) -> Result<(), String> {
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create directory {}: {e}", parent.display()))?;

        let file_name = self
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("storage");
        let pid = std::process::id();
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp_path = parent.join(format!(".{file_name}.tmp.{pid}.{counter}"));

        let write_res = (|| -> Result<(), std::io::Error> {
            let mut file = fs::File::create(&tmp_path)?;
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
            Ok(())
        })();

        if let Err(e) = write_res {
            let _ = fs::remove_file(&tmp_path);
            return Err(format!(
                "failed to write temporary file {}: {e}",
                tmp_path.display()
            ));
        }

        if let Err(e) = fs::rename(&tmp_path, &self.path) {
            let _ = fs::remove_file(&tmp_path);
            return Err(format!(
                "failed to rename {} to {}: {e}",
                tmp_path.display(),
                self.path.display()
            ));
        }

        Ok(())
    }

    fn clear(&self) -> Result<(), String> {
        if self.path.exists() {
            fs::remove_file(&self.path)
                .map_err(|e| format!("failed to delete file {}: {e}", self.path.display()))
        } else {
            Ok(())
        }
    }
}

/// Normalizes runtime settings to the canonical core restore schema.
/// Omits transient telemetry and prevents state ON upon reboot.
pub fn normalize_runtime_state(val: &Value) -> Value {
    let obj = match val.as_object() {
        Some(o) => o,
        None => return json!({}),
    };

    let mut out = serde_json::Map::new();
    out.insert("version".to_string(), json!(1));

    if let Some(b) = obj.get("brightness") {
        if b.is_number() {
            out.insert("brightness".to_string(), b.clone());
        }
    }

    if let Some(eff) = obj.get("effect") {
        if eff.is_string() {
            out.insert("effect".to_string(), eff.clone());
        }
    }

    if let Some(ac) = obj.get("autoCycle").and_then(|v| v.as_bool()) {
        out.insert("autoCycle".to_string(), json!(ac));
    } else if let Some(cycle) = obj.get("cycle") {
        let is_on = match cycle {
            Value::String(s) => s.eq_ignore_ascii_case("on") || s == "1",
            Value::Bool(b) => *b,
            Value::Number(n) => n.as_i64() == Some(1),
            _ => false,
        };
        out.insert("autoCycle".to_string(), json!(is_on));
    }

    if let Some(text) = obj.get("text") {
        if text.is_string() {
            out.insert("text".to_string(), text.clone());
        }
    }

    if let Some(speed) = obj.get("speed") {
        if speed.is_number() {
            out.insert("speed".to_string(), speed.clone());
        }
    }

    if let Some(color) = obj.get("color") {
        if color.is_object() {
            out.insert("color".to_string(), color.clone());
        }
    }

    if let Some(audio) = obj.get("audio").and_then(|v| v.as_str()) {
        out.insert("audio".to_string(), json!(audio));
    } else if let Some(audio_src) = obj.get("audio_source").and_then(|v| v.as_str()) {
        out.insert("audio".to_string(), json!(audio_src));
    }

    if let Some(mic) = obj.get("mic") {
        if mic.is_object() {
            out.insert("mic".to_string(), mic.clone());
        }
    }

    let params_source = obj.get("params").or_else(|| obj.get("effects"));
    if let Some(effects_obj) = params_source.and_then(|v| v.as_object()) {
        let mut params_map = serde_json::Map::new();
        for (eff_id, p_map) in effects_obj {
            if let Some(p_obj) = p_map.as_object() {
                let mut single_effect_params = serde_json::Map::new();
                for (param_name, val) in p_obj {
                    if let Some(n) = val.as_f64() {
                        single_effect_params.insert(param_name.clone(), json!(n));
                    } else if let Some(s) = val.as_str() {
                        if s.eq_ignore_ascii_case("on") || s.eq_ignore_ascii_case("true") {
                            single_effect_params.insert(param_name.clone(), json!(1.0));
                        } else if s.eq_ignore_ascii_case("off") || s.eq_ignore_ascii_case("false") {
                            single_effect_params.insert(param_name.clone(), json!(0.0));
                        } else if let Ok(n) = s.parse::<f64>() {
                            single_effect_params.insert(param_name.clone(), json!(n));
                        } else {
                            single_effect_params.insert(param_name.clone(), json!(s));
                        }
                    } else if let Some(b) = val.as_bool() {
                        single_effect_params
                            .insert(param_name.clone(), json!(if b { 1.0 } else { 0.0 }));
                    }
                }
                if !single_effect_params.is_empty() {
                    params_map.insert(eff_id.clone(), Value::Object(single_effect_params));
                }
            }
        }
        if !params_map.is_empty() {
            out.insert("params".to_string(), Value::Object(params_map));
        }
    }

    Value::Object(out)
}

/// Manages persistence of runtime state (survives reboots).
pub struct StateStore {
    backend: Option<Box<dyn StorageBackend>>,
    saved: RwLock<Option<Value>>,
}

impl StateStore {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self::try_new(path).unwrap_or_else(|_| Self {
            backend: None,
            saved: RwLock::new(None),
        })
    }

    pub fn try_new(path: Option<PathBuf>) -> Result<Self, String> {
        if let Some(p) = path {
            let backend = FsStorageBackend::new(p);
            let mut saved = None;
            if let Some(content) = backend.load()? {
                let val: Value = serde_json::from_str(&content)
                    .map_err(|e| format!("invalid state JSON: {e}"))?;
                saved = Some(normalize_runtime_state(&val));
            }
            Ok(Self {
                backend: Some(Box::new(backend)),
                saved: RwLock::new(saved),
            })
        } else {
            Ok(Self {
                backend: None,
                saved: RwLock::new(None),
            })
        }
    }

    pub fn with_backend(backend: Box<dyn StorageBackend>) -> Self {
        let mut saved = None;
        if let Ok(Some(content)) = backend.load() {
            if let Ok(val) = serde_json::from_str::<Value>(&content) {
                saved = Some(normalize_runtime_state(&val));
            }
        }
        Self {
            backend: Some(backend),
            saved: RwLock::new(saved),
        }
    }

    pub fn save(&self, snapshot: &Value) -> Result<(), String> {
        let to_store = normalize_runtime_state(snapshot);

        let json = serde_json::to_string_pretty(&to_store)
            .map_err(|e| format!("serialization error: {e}"))?;

        if let Some(ref b) = self.backend {
            b.store(&json)?;
        }

        if let Ok(mut lock) = self.saved.write() {
            *lock = Some(to_store);
        }

        Ok(())
    }

    pub fn load(&self) -> Option<Value> {
        self.saved.read().ok().and_then(|lock| (*lock).clone())
    }

    /// Restores saved settings into the core Engine and guarantees display boots OFF.
    pub fn restore(&self, engine: &mut matrix_core::Engine) -> Result<(), String> {
        let saved_val = match self.load() {
            Some(v) => v,
            None => return Ok(()),
        };

        let normalized = normalize_runtime_state(&saved_val);
        let json_str =
            serde_json::to_string(&normalized).map_err(|e| format!("serialization error: {e}"))?;

        matrix_core::restore_runtime_state(engine, &json_str)?;

        // Apply string parameters (e.g. select options) if skipped by restore_runtime_state f64 parsing
        if let Some(params_obj) = normalized.get("params").and_then(|v| v.as_object()) {
            for (effect_id, param_map) in params_obj {
                if let Some(map) = param_map.as_object() {
                    for (param_id, val) in map {
                        if let Some(s) = val.as_str() {
                            let _ = engine.apply(&format!("effect/{effect_id}/{param_id}"), s);
                        }
                    }
                }
            }
        }

        // Hardware invariant: always boot OFF
        let _ = engine.apply("light", "{\"state\":\"OFF\"}");

        Ok(())
    }

    pub fn clear(&self) -> Result<(), String> {
        if let Some(ref b) = self.backend {
            b.clear()?;
        }
        if let Ok(mut lock) = self.saved.write() {
            *lock = None;
        }
        Ok(())
    }
}
