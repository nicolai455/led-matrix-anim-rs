//! Persistent recovery policy and independent progress supervision.
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Journal {
    pub failures: u32,
    #[serde(default)]
    pub trial: bool,
    #[serde(default)]
    pub trial_partition: Option<u32>,
    pub in_progress: bool,
    pub safe_mode: bool,
    pub reason: String,
}
impl Journal {
    pub fn begin(&mut self) {
        self.begin_partition(None);
    }
    pub fn begin_partition(&mut self, partition: Option<u32>) {
        if self.in_progress {
            self.failures = self.failures.saturating_add(1);
            self.reason = "previous run ended without planned shutdown".into();
        }
        self.safe_mode |= self.failures >= 3;
        if self.trial && self.trial_partition == partition {
            self.safe_mode = false;
            self.trial = false;
        }
        self.trial = false;
        self.trial_partition = None;
        self.in_progress = true;
    }
    pub fn healthy(&mut self) {
        self.failures = 0;
    }
    pub fn planned(&mut self) {
        self.in_progress = false;
    }
    pub fn arm_trial(&mut self) {
        self.trial = self.safe_mode;
    }
    pub fn retry(&mut self) {
        *self = Self::default();
    }
}

pub struct Watchdog {
    render: u32,
    output: u32,
    render_at: u64,
    output_at: u64,
}
impl Watchdog {
    pub fn new(now: u64, render: u32, output: u32) -> Self {
        Self {
            render,
            output,
            render_at: now,
            output_at: now,
        }
    }
    pub fn stalled(&mut self, now: u64, render: u32, output: u32, paused: bool) -> bool {
        if render != self.render {
            self.render = render;
            self.render_at = now;
        }
        if output != self.output || paused {
            self.output = output;
            self.output_at = now;
        }
        now.saturating_sub(self.render_at) > 5000 || now.saturating_sub(self.output_at) > 5000
    }
}

/// Recovery deliberately does not touch a LED peripheral. It never reports a
/// successful physical transfer; latched LEDs may require a panel power cycle.
pub enum RecoveryOutput {
    Hardware(crate::output::RmtLedOutput),
    Inactive(crate::output::OutputQuiesce),
}
impl RecoveryOutput {
    pub fn control(&self) -> crate::output::OutputQuiesce {
        match self {
            Self::Hardware(o) => o.get_quiesce_handle(),
            Self::Inactive(q) => q.clone(),
        }
    }
}
impl matrix_control::LedOutput for RecoveryOutput {
    fn show(&mut self, frame: &matrix_core::Frame, brightness: u8) -> Result<(), String> {
        match self {
            Self::Hardware(o) => o.show(frame, brightness),
            Self::Inactive(_) => {
                std::thread::sleep(std::time::Duration::from_millis(20));
                Err("Recovery mode: LED peripheral inactive".into())
            }
        }
    }
    fn set_power_limit(&mut self, value: u32) -> Result<(), String> {
        match self {
            Self::Hardware(o) => o.set_power_limit(value),
            Self::Inactive(_) => Ok(()),
        }
    }
    fn update_config(&mut self, config: &matrix_core::PanelConfig) -> Result<(), String> {
        match self {
            Self::Hardware(o) => o.update_config(config),
            Self::Inactive(_) => Ok(()),
        }
    }
}

#[cfg(target_os = "espidf")]
mod runtime {
    use super::*;
    use crate::nvs_store::FirmwareNvs;
    use std::sync::{Arc, Mutex, OnceLock};
    static STATE: OnceLock<(Arc<FirmwareNvs>, Mutex<Journal>)> = OnceLock::new();
    const KEY: &str = "recovery";
    static ATTACHED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    fn persist(nvs: &FirmwareNvs, journal: &Journal) -> Result<(), String> {
        nvs.set_str(
            KEY,
            &serde_json::to_string(journal).map_err(|e| e.to_string())?,
        )
    }
    pub fn begin(nvs: Arc<FirmwareNvs>) -> Result<bool, String> {
        let mut journal: Journal = match nvs.get_str(KEY)? {
            Some(s) => {
                serde_json::from_str(&s).map_err(|e| format!("Recovery journal invalid: {e}"))?
            }
            None => Journal::default(),
        };
        let partition = unsafe { esp_idf_sys::esp_ota_get_running_partition() };
        if partition.is_null() {
            return Err("Running partition unavailable".into());
        }
        journal.begin_partition(Some(unsafe { (*partition).address }));
        persist(&nvs, &journal)?;
        let safe = journal.safe_mode;
        STATE
            .set((nvs, Mutex::new(journal)))
            .map_err(|_| "Recovery initialized twice")?;
        std::thread::Builder::new()
            .name("startup-watch".into())
            .stack_size(4096)
            .spawn(|| {
                std::thread::sleep(std::time::Duration::from_secs(60));
                if !ATTACHED.load(std::sync::atomic::Ordering::Acquire) {
                    unsafe {
                        esp_idf_sys::esp_restart();
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(safe)
    }
    pub fn status() -> serde_json::Value {
        STATE.get().and_then(|(_, j)| j.try_lock().ok()).map(|j| serde_json::json!({"safeMode":j.safe_mode,"failures":j.failures,"reason":j.reason}))
            .unwrap_or_else(|| serde_json::json!({"error":"recovery state unavailable"}))
    }
    pub fn startup_ready() {
        ATTACHED.store(true, std::sync::atomic::Ordering::Release);
    }
    pub fn arm_ota_trial(partition: u32) -> Result<(), String> {
        change(|j| {
            j.arm_trial();
            j.trial_partition = Some(partition);
        })
    }
    pub fn planned() -> Result<(), String> {
        change(|j| j.planned())
    }
    pub fn retry() -> Result<(), String> {
        change(|j| j.retry())
    }
    fn change(f: impl FnOnce(&mut Journal)) -> Result<(), String> {
        let (nvs, lock) = STATE.get().ok_or("Recovery unavailable")?;
        let mut j = lock.try_lock().map_err(|_| "Recovery busy")?;
        let mut next = j.clone();
        f(&mut next);
        persist(nvs, &next)?;
        *j = next;
        Ok(())
    }
    pub fn supervise(
        hub: matrix_control::hub::ControlHub,
        output: crate::output::OutputQuiesce,
        safe: bool,
    ) -> Result<(), String> {
        std::thread::Builder::new()
            .name("progress-guard".into())
            .stack_size(6144)
            .spawn(move || {
                let started = std::time::Instant::now();
                let mut watch = Watchdog::new(0, hub.render_progress(), hub.output_progress().0);
                let mut healthy_saved = false;
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    let now = started.elapsed().as_millis() as u64;
                    let stalled = watch.stalled(
                        now,
                        hub.render_progress(),
                        hub.output_progress().0,
                        safe || output.is_paused(),
                    );
                    if stalled {
                        // Failure count was durably armed at boot. If NVS is busy,
                        // reset anyway rather than blocking the independent guard.
                        log::error!(
                            "Render/output stalled; restarting (boot journal remains armed)"
                        );
                        unsafe {
                            esp_idf_sys::esp_restart();
                        }
                    }
                    if !healthy_saved
                        && now >= 60000
                        && !safe
                        && ATTACHED.load(std::sync::atomic::Ordering::Acquire)
                    {
                        healthy_saved = true;
                        // Flash/NVS serialization must never block watchdog
                        // polling. This one-shot maintenance task may wait.
                        let _maintenance = std::thread::Builder::new()
                            .name("recovery-stable".into())
                            .stack_size(4096)
                            .spawn(|| {
                                while change(|j| j.healthy()).is_err() {
                                    std::thread::sleep(std::time::Duration::from_secs(2));
                                }
                            });
                        if _maintenance.is_err() {
                            healthy_saved = false;
                        }
                    }
                }
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}
#[cfg(target_os = "espidf")]
pub use runtime::*;
