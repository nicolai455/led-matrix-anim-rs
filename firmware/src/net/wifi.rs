pub const FALLBACK_AP_SSID: &str = "JDJLIGHT";
pub const FALLBACK_AP_PASSWORD: &str = "JDJLIGHT1337";
pub const STA_CONNECT_TIMEOUT_SECS: u64 = 18; // WLED standard timeout (~18s)

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WifiConfig {
    pub ssid: String,
    pub password: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FallbackApConfig {
    pub ssid: String,
    pub password: String,
    pub channel: u8,
    pub max_connections: u16,
}

impl Default for FallbackApConfig {
    fn default() -> Self {
        Self {
            ssid: FALLBACK_AP_SSID.to_string(),
            password: FALLBACK_AP_PASSWORD.to_string(),
            channel: 1,
            max_connections: 4,
        }
    }
}

impl FallbackApConfig {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.ssid.is_empty() || self.ssid.len() > 32 {
            return Err("Fallback AP SSID must be 1..=32 chars");
        }
        if self.password.len() < 8 || self.password.len() > 64 {
            return Err("Fallback AP password must be 8..=64 chars for WPA2");
        }
        if self.channel == 0 || self.channel > 14 {
            return Err("Wi-Fi channel must be 1..=14");
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WifiOperatingMode {
    /// Dedicated Access Point mode when no STA credentials exist.
    ApOnly,
    /// Standard Station mode actively attempting to connect or connected.
    StaOnly,
    /// Mixed AP+STA mode: AP is broadcasting fallback while STA continues background retries.
    MixedApSta,
}

#[derive(Clone, Debug)]
pub struct WifiPolicy {
    pub timeout_secs: u64,
    pub ap_config: FallbackApConfig,
}

impl Default for WifiPolicy {
    fn default() -> Self {
        Self {
            timeout_secs: STA_CONNECT_TIMEOUT_SECS,
            ap_config: FallbackApConfig::default(),
        }
    }
}

impl WifiPolicy {
    pub fn new(timeout_secs: u64, ap_config: FallbackApConfig) -> Self {
        Self {
            timeout_secs,
            ap_config,
        }
    }

    /// Evaluates target operating mode based on STA credentials and connection duration.
    pub fn evaluate_mode(
        &self,
        has_sta_config: bool,
        is_sta_connected: bool,
        elapsed_connecting_secs: u64,
    ) -> WifiOperatingMode {
        if !has_sta_config {
            return WifiOperatingMode::ApOnly;
        }

        if is_sta_connected {
            return WifiOperatingMode::StaOnly;
        }

        if elapsed_connecting_secs >= self.timeout_secs {
            WifiOperatingMode::MixedApSta
        } else {
            WifiOperatingMode::StaOnly
        }
    }
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
        use esp_idf_svc::wifi::{
            AccessPointConfiguration, AuthMethod, ClientConfiguration, Configuration, EspWifi,
        };
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::time::{Duration, Instant};

        let shutdown = std::sync::Arc::new(AtomicBool::new(false));
        let policy = WifiPolicy::default();
        policy.ap_config.validate()?;

        let mut wifi = EspWifi::new(modem, sysloop, Some(nvs)).map_err(|e| e.to_string())?;

        let ap_conf = AccessPointConfiguration {
            ssid: policy
                .ap_config
                .ssid
                .as_str()
                .try_into()
                .map_err(|_| "Fallback AP SSID too long")?,
            password: policy
                .ap_config
                .password
                .as_str()
                .try_into()
                .map_err(|_| "Fallback AP password too long")?,
            auth_method: AuthMethod::WPA2Personal,
            channel: policy.ap_config.channel,
            max_connections: policy.ap_config.max_connections,
            ..Default::default()
        };

        let (init_mode, client_conf) = match config {
            Some(ref sta) if !sta.ssid.is_empty() => {
                let password = sta.password.clone().unwrap_or_default();
                let client = ClientConfiguration {
                    ssid: sta
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
                };
                (Configuration::Client(client.clone()), Some(client))
            }
            _ => (Configuration::AccessPoint(ap_conf.clone()), None),
        };

        wifi.set_configuration(&init_mode).map_err(|e| e.to_string())?;
        wifi.start().map_err(|e| e.to_string())?;

        let power = unsafe {
            esp_idf_svc::sys::esp_wifi_set_ps(esp_idf_svc::sys::wifi_ps_type_t_WIFI_PS_NONE)
        };
        if power != esp_idf_svc::sys::ESP_OK {
            return Err(format!("Cannot disable Wi-Fi modem sleep: {power}"));
        }

        if client_conf.is_none() {
            log::info!(
                "No STA Wi-Fi configured - Fallback AP '{}' active on 192.168.4.1 (Password: '{}')",
                FALLBACK_AP_SSID,
                FALLBACK_AP_PASSWORD
            );
        }

        let stop = shutdown.clone();
        let worker = std::thread::Builder::new()
            .name("wifi-manager".into())
            .stack_size(4096)
            .spawn(move || {
                let mut current_mode = match client_conf {
                    Some(_) => WifiOperatingMode::StaOnly,
                    None => WifiOperatingMode::ApOnly,
                };

                let mut connecting_start = Instant::now();
                let mut sta_ready = false;

                while !stop.load(Ordering::Relaxed) {
                    if let Some(ref client) = client_conf {
                        let connected = wifi.is_connected().unwrap_or(false);
                        let up = connected && wifi.sta_netif().is_up().unwrap_or(false);

                        if up && !sta_ready {
                            log::info!("Wi-Fi STA IP ready: {:?}", wifi.sta_netif().get_ip_info().ok());
                            connecting_start = Instant::now();
                        }

                        if !connected {
                            let _ = wifi.connect();
                        }

                        let elapsed = connecting_start.elapsed().as_secs();
                        let desired_mode = policy.evaluate_mode(true, up, elapsed);

                        if desired_mode != current_mode {
                            match desired_mode {
                                WifiOperatingMode::MixedApSta => {
                                    log::warn!(
                                        "Wi-Fi connection to '{}' timed out after {}s - Activating fallback AP '{}' (PW: '{}')",
                                        client.ssid,
                                        policy.timeout_secs,
                                        FALLBACK_AP_SSID,
                                        FALLBACK_AP_PASSWORD
                                    );
                                    let mixed = Configuration::Mixed(client.clone(), ap_conf.clone());
                                    if let Err(e) = wifi.set_configuration(&mixed) {
                                        log::error!("Failed switching to Mixed AP+STA: {e}");
                                    } else {
                                        current_mode = WifiOperatingMode::MixedApSta;
                                        log::info!(
                                            "Fallback AP '{}' broadcasting on 192.168.4.1. Web UI accessible via AP.",
                                            FALLBACK_AP_SSID
                                        );
                                    }
                                }
                                WifiOperatingMode::StaOnly => {
                                    log::info!("Wi-Fi STA connection re-established, returning to STA mode.");
                                    let sta_only = Configuration::Client(client.clone());
                                    if let Err(e) = wifi.set_configuration(&sta_only) {
                                        log::error!("Failed switching back to STA only: {e}");
                                    } else {
                                        current_mode = WifiOperatingMode::StaOnly;
                                    }
                                }
                                WifiOperatingMode::ApOnly => {}
                            }
                        }

                        sta_ready = up;
                    }

                    for _ in 0..20 {
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(100));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_ap_default_configuration() {
        let ap = FallbackApConfig::default();
        assert_eq!(ap.ssid, "JDJLIGHT");
        assert_eq!(ap.password, "JDJLIGHT1337");
        assert_eq!(ap.channel, 1);
        assert!(ap.validate().is_ok());
    }

    #[test]
    fn test_fallback_ap_validation_rejects_invalid_credentials() {
        let mut ap = FallbackApConfig::default();
        ap.ssid = String::new();
        assert!(ap.validate().is_err());

        ap.ssid = "ValidSSID".to_string();
        ap.password = "short".to_string(); // WPA2 requires >= 8 chars
        assert!(ap.validate().is_err());

        ap.password = "valid_password_1337".to_string();
        ap.channel = 0;
        assert!(ap.validate().is_err());

        ap.channel = 15;
        assert!(ap.validate().is_err());

        ap.channel = 6;
        assert!(ap.validate().is_ok());
    }

    #[test]
    fn test_policy_evaluates_immediate_ap_when_no_sta_configured() {
        let policy = WifiPolicy::default();
        let mode = policy.evaluate_mode(false, false, 0);
        assert_eq!(mode, WifiOperatingMode::ApOnly);

        let mode_later = policy.evaluate_mode(false, false, 30);
        assert_eq!(mode_later, WifiOperatingMode::ApOnly);
    }

    #[test]
    fn test_policy_transitions_to_fallback_ap_on_sta_timeout() {
        let policy = WifiPolicy::new(18, FallbackApConfig::default());

        // Initial phase (< 18s): StaOnly
        assert_eq!(
            policy.evaluate_mode(true, false, 0),
            WifiOperatingMode::StaOnly
        );
        assert_eq!(
            policy.evaluate_mode(true, false, 10),
            WifiOperatingMode::StaOnly
        );
        assert_eq!(
            policy.evaluate_mode(true, false, 17),
            WifiOperatingMode::StaOnly
        );

        // Timeout reached (>= 18s): Activates MixedApSta (Fallback AP active)
        assert_eq!(
            policy.evaluate_mode(true, false, 18),
            WifiOperatingMode::MixedApSta
        );
        assert_eq!(
            policy.evaluate_mode(true, false, 60),
            WifiOperatingMode::MixedApSta
        );

        // Once connected, reverts to StaOnly
        assert_eq!(
            policy.evaluate_mode(true, true, 25),
            WifiOperatingMode::StaOnly
        );
    }
}
