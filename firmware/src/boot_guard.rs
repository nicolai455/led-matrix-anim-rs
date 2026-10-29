//! Pending OTA startup validation (runs independently of initialization).
use crate::boot_validation::{decide, Decision, Observation};
use crate::output::OutputQuiesce;
use matrix_control::hub::ControlHub;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct BootGuard {
    pub hub: Option<ControlHub>,
    pub output: Option<OutputQuiesce>,
    pub ready: bool,
    pub failed: Arc<AtomicBool>,
}

pub fn start() -> Arc<Mutex<BootGuard>> {
    let health = Arc::new(Mutex::new(BootGuard::default()));
    let partition = unsafe { esp_idf_sys::esp_ota_get_running_partition() };
    assert!(!partition.is_null(), "Cannot identify running partition");
    let mut state = esp_idf_sys::esp_ota_img_states_t_ESP_OTA_IMG_UNDEFINED;
    let rc = unsafe { esp_idf_sys::esp_ota_get_state_partition(partition, &mut state) };
    if rc == esp_idf_sys::ESP_ERR_NOT_FOUND {
        log::warn!(
            "No OTA state record for running image; rollback capability cannot be established"
        );
        return health;
    }
    assert_eq!(rc, esp_idf_sys::ESP_OK, "Cannot query OTA image state");
    if state != esp_idf_sys::esp_ota_img_states_t_ESP_OTA_IMG_PENDING_VERIFY {
        return health;
    }
    let shared = health.clone();
    let failed_latch = health
        .lock()
        .expect("boot health lock poisoned")
        .failed
        .clone();
    std::thread::Builder::new()
        .name("boot-validation".into())
        .stack_size(8192)
        .spawn(move || {
            let started = Instant::now();
            let mut healthy_since = None;
            let mut previous = 0;
            let mut last_output = None;
            loop {
                std::thread::sleep(Duration::from_secs(1));
                let (hub, ready, output) = match shared.try_lock() {
                    Ok(h) => (h.hub.clone(), h.ready, h.output.clone()),
                    Err(_) => (None, false, None),
                };
                let failed = failed_latch.load(Ordering::Acquire);
                let ready = ready && output.as_ref().is_some_and(|o| !o.is_requested());
                if output.is_some() {
                    last_output = output;
                }
                let (frames, errors) = hub.as_ref().map(|h| h.output_progress()).unwrap_or((0, 0));
                let progressing = frames != previous;
                previous = frames;
                if ready && progressing && errors == 0 {
                    if healthy_since.is_none() {
                        healthy_since = Some(Instant::now());
                    }
                } else {
                    healthy_since = None;
                }
                let healthy_ms = healthy_since
                    .map(|t| t.elapsed().as_millis() as u64)
                    .unwrap_or(0);
                let elapsed_ms = started.elapsed().as_millis() as u64;
                let decision = if elapsed_ms >= 30_000 {
                    Decision::Fail
                } else {
                    decide(Observation {
                        pending: true,
                        elapsed_ms: healthy_ms,
                        ready,
                        failed,
                        frames,
                        errors,
                        progressing,
                    })
                };
                match decision {
                    Decision::Confirm => {
                        let rc = unsafe { esp_idf_sys::esp_ota_mark_app_valid_cancel_rollback() };
                        if rc == esp_idf_sys::ESP_OK {
                            log::warn!("Pending OTA confirmed after sustained output health");
                            return;
                        }
                        log::error!("OTA confirmation failed: {rc}");
                    }
                    Decision::Wait => continue,
                    Decision::Ignore => return,
                    Decision::Fail => log::error!("OTA startup health deadline expired"),
                }
                failed_latch.store(true, Ordering::Release);
                if unsafe { esp_idf_sys::esp_ota_check_rollback_is_possible() } {
                    let rc = unsafe { esp_idf_sys::esp_ota_mark_app_invalid_rollback_and_reboot() };
                    log::error!("OTA rollback failed: {rc}; entering safe recovery");
                } else {
                    log::error!("No rollback image available; entering safe recovery unconfirmed");
                }
                if let Some(output) = &last_output {
                    if let Err(e) = output.pause(Duration::from_secs(2)) {
                        log::error!("Safe output pause failed: {e}");
                    }
                }
                loop {
                    std::thread::sleep(Duration::from_secs(60));
                }
            }
        })
        .expect("Cannot start OTA validation worker");
    health
}
