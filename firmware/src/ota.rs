use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub const ESP_IMAGE_HEADER_MAGIC: u8 = 0xE9;

pub struct OtaManager {
    quiesce_flags: Vec<Arc<AtomicBool>>,
    quiesce_fn: Option<Box<dyn Fn() -> Result<(), String> + Send + Sync>>,
    resume_fn: Option<Box<dyn Fn() + Send + Sync>>,
    header_checked: bool,
    total_written: usize,
    #[cfg(target_os = "espidf")]
    active_handle: Option<esp_idf_sys::esp_ota_handle_t>,
}

impl Default for OtaManager {
    fn default() -> Self {
        Self::new()
    }
}

impl OtaManager {
    pub fn new() -> Self {
        Self {
            quiesce_flags: Vec::new(),
            quiesce_fn: None,
            resume_fn: None,
            header_checked: false,
            total_written: 0,
            #[cfg(target_os = "espidf")]
            active_handle: None,
        }
    }

    pub fn set_quiesce_hooks<Q, R>(&mut self, quiesce: Q, resume: R)
    where
        Q: Fn() -> Result<(), String> + Send + Sync + 'static,
        R: Fn() + Send + Sync + 'static,
    {
        self.quiesce_fn = Some(Box::new(quiesce));
        self.resume_fn = Some(Box::new(resume));
    }

    pub fn register_quiesce_flag(&mut self, flag: Arc<AtomicBool>) {
        self.quiesce_flags.push(flag);
    }

    pub fn quiesce(&self) -> Result<(), String> {
        log::info!("OTA: initiating hardware quiesce (dark output + mic DMA stop)...");
        for flag in &self.quiesce_flags {
            flag.store(true, Ordering::SeqCst);
        }

        if let Some(ref q) = self.quiesce_fn {
            q()?;
        }

        log::info!("OTA: hardware quiesce confirmed");
        Ok(())
    }

    pub fn resume(&self) {
        log::info!("OTA: resuming hardware peripherals...");
        for flag in &self.quiesce_flags {
            flag.store(false, Ordering::SeqCst);
        }

        if let Some(ref r) = self.resume_fn {
            r();
        }
    }

    #[cfg(target_os = "espidf")]
    pub fn begin(&mut self) -> Result<(), String> {
        self.header_checked = false;
        self.total_written = 0;

        self.quiesce()?;

        unsafe {
            let update_partition = esp_idf_sys::esp_ota_get_next_update_partition(std::ptr::null());
            if update_partition.is_null() {
                self.resume();
                return Err("OTA: no valid update partition found".to_string());
            }

            let mut handle: esp_idf_sys::esp_ota_handle_t = 0;
            let err = esp_idf_sys::esp_ota_begin(
                update_partition,
                esp_idf_sys::OTA_SIZE_UNKNOWN as usize,
                &mut handle,
            );
            if err != esp_idf_sys::ESP_OK {
                self.resume();
                return Err(format!("OTA begin failed: error code {err}"));
            }

            self.active_handle = Some(handle);
            Ok(())
        }
    }

    #[cfg(not(target_os = "espidf"))]
    pub fn begin(&mut self) -> Result<(), String> {
        self.header_checked = false;
        self.total_written = 0;
        self.quiesce()?;
        log::info!("OTA begin (simulated)");
        Ok(())
    }

    pub fn abort(&mut self) {
        #[cfg(target_os = "espidf")]
        if let Some(handle) = self.active_handle.take() {
            unsafe {
                esp_idf_sys::esp_ota_abort(handle);
            }
        }
        self.header_checked = false;
        self.total_written = 0;
        self.resume();
    }

    /// Check the advertised transfer length before erasing or quiescing.
    pub fn validate_image_length(length: usize, partition_size: usize) -> Result<(), String> {
        if length == 0 {
            return Err("raw image Content-Length required".into());
        }
        if length > partition_size {
            return Err("firmware image exceeds update partition".into());
        }
        Ok(())
    }

    #[cfg(target_os = "espidf")]
    pub fn update_partition_size() -> Result<usize, String> {
        let partition = unsafe { esp_idf_sys::esp_ota_get_next_update_partition(std::ptr::null()) };
        if partition.is_null() {
            return Err("no update partition available".into());
        }
        Ok(unsafe { (*partition).size as usize })
    }

    pub fn validate_header(data: &[u8]) -> Result<(), String> {
        if data.is_empty() {
            return Err("OTA: empty initial payload".to_string());
        }
        if data[0] != ESP_IMAGE_HEADER_MAGIC {
            return Err(format!(
                "OTA: invalid firmware image magic 0x{:02X}, expected 0x{:02X}",
                data[0], ESP_IMAGE_HEADER_MAGIC
            ));
        }
        Ok(())
    }

    #[cfg(target_os = "espidf")]
    pub fn write_chunk(&mut self, data: &[u8]) -> Result<(), String> {
        let handle = self
            .active_handle
            .ok_or_else(|| "OTA: write called without active begin".to_string())?;

        if !self.header_checked && !data.is_empty() {
            Self::validate_header(data)?;
            self.header_checked = true;
        }

        unsafe {
            let err = esp_idf_sys::esp_ota_write(
                handle,
                data.as_ptr() as *const std::ffi::c_void,
                data.len(),
            );
            if err != esp_idf_sys::ESP_OK {
                let _ = esp_idf_sys::esp_ota_abort(handle);
                self.active_handle = None;
                self.resume();
                return Err(format!("OTA write failed: error code {err}"));
            }
        }
        self.total_written += data.len();
        Ok(())
    }

    #[cfg(not(target_os = "espidf"))]
    pub fn write_chunk(&mut self, data: &[u8]) -> Result<(), String> {
        if !self.header_checked && !data.is_empty() {
            Self::validate_header(data)?;
            self.header_checked = true;
        }
        self.total_written += data.len();
        Ok(())
    }

    #[cfg(target_os = "espidf")]
    pub fn finish(&mut self) -> Result<(), String> {
        let handle = self
            .active_handle
            .take()
            .ok_or_else(|| "OTA: finish called without active begin".to_string())?;

        unsafe {
            let end_err = esp_idf_sys::esp_ota_end(handle);
            if end_err != esp_idf_sys::ESP_OK {
                self.resume();
                return Err(format!("OTA end validation failed: error code {end_err}"));
            }

            let update_partition = esp_idf_sys::esp_ota_get_next_update_partition(std::ptr::null());
            let set_err = esp_idf_sys::esp_ota_set_boot_partition(update_partition);
            if set_err != esp_idf_sys::ESP_OK {
                self.resume();
                return Err(format!(
                    "OTA set boot partition failed: error code {set_err}"
                ));
            }
        }

        log::info!(
            "OTA: firmware successfully written ({} bytes) and validated. Reboot required.",
            self.total_written
        );
        Ok(())
    }

    #[cfg(not(target_os = "espidf"))]
    pub fn finish(&mut self) -> Result<(), String> {
        self.resume();
        log::info!(
            "OTA finish (simulated): total {} bytes validated",
            self.total_written
        );
        Ok(())
    }

    pub fn schedule_restart() {
        log::info!("OTA: scheduling restart in 1 second...");
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(1000));
            #[cfg(target_os = "espidf")]
            unsafe {
                esp_idf_sys::esp_restart();
            }
            #[cfg(not(target_os = "espidf"))]
            {
                log::info!("Simulated reboot triggered");
            }
        });
    }
}
