#[derive(Clone)]
pub struct WifiConfig {
    pub ssid: String,
    pub password: Option<String>,
}

pub struct WifiManager {
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl WifiManager {
    #[cfg(target_os = "espidf")]
    pub fn start(
        modem: esp_idf_hal::modem::Modem,
        sysloop: esp_idf_svc::eventloop::EspSystemEventLoop,
        nvs: esp_idf_svc::nvs::EspDefaultNvsPartition,
        config: Option<WifiConfig>,
    ) -> Result<Self, String> {
        use esp_idf_svc::wifi::{AuthMethod, ClientConfiguration, Configuration, EspWifi};
        use std::sync::atomic::{AtomicBool, Ordering};
        let shutdown = std::sync::Arc::new(AtomicBool::new(false));
        let Some(config) = config else {
            return Ok(Self {
                shutdown,
                worker: None,
            });
        };
        let mut wifi = EspWifi::new(modem, sysloop, Some(nvs)).map_err(|e| e.to_string())?;
        let password = config.password.unwrap_or_default();
        wifi.set_configuration(&Configuration::Client(ClientConfiguration {
            ssid: config
                .ssid
                .as_str()
                .try_into()
                .map_err(|_| "Wi-Fi SSID too long")?,
            password: password
                .as_str()
                .try_into()
                .map_err(|_| "Wi-Fi password too long")?,
            auth_method: if password.is_empty() {
                AuthMethod::None
            } else {
                AuthMethod::WPA2Personal
            },
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        wifi.start().map_err(|e| e.to_string())?;
        let power = unsafe {
            esp_idf_svc::sys::esp_wifi_set_ps(esp_idf_svc::sys::wifi_ps_type_t_WIFI_PS_NONE)
        };
        if power != esp_idf_svc::sys::ESP_OK {
            return Err(format!("Cannot disable Wi-Fi modem sleep: {power}"));
        }
        let stop = shutdown.clone();
        let worker = std::thread::Builder::new()
            .name("wifi-reconnect".into())
            .stack_size(4096)
            .spawn(move || {
                let mut ready = false;
                while !stop.load(Ordering::Relaxed) {
                    let connected = wifi.is_connected().unwrap_or(false);
                    let up = connected && wifi.sta_netif().is_up().unwrap_or(false);
                    if up && !ready {
                        log::info!("Wi-Fi IP ready: {:?}", wifi.sta_netif().get_ip_info().ok());
                    }
                    if !connected {
                        let _ = wifi.connect();
                    }
                    ready = up;
                    for _ in 0..20 {
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
                let _ = wifi.stop();
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            shutdown,
            worker: Some(worker),
        })
    }
    #[cfg(not(target_os = "espidf"))]
    pub fn start(_config: Option<WifiConfig>) -> Result<Self, String> {
        Err("ESP Wi-Fi hardware unavailable on host".into())
    }
}
impl Drop for WifiManager {
    fn drop(&mut self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
