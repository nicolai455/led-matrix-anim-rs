pub const FALLBACK_AP_SSID: &str = "JDJLIGHT";
/// Legacy compatibility export. A password is never supplied by firmware.
pub const FALLBACK_AP_PASSWORD: &str = "";
pub const STA_CONNECT_TIMEOUT_SECS: u64 = 18; // WLED standard timeout (~18s)

use crate::network_config::{NetworkConfig, WifiProfile};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WifiConfig {
    pub profiles: Vec<WifiProfile>,
    pub fallback_ap: Option<FallbackApConfig>,
    /// Legacy single-profile fields retained for the existing firmware caller.
    pub ssid: String,
    pub password: Option<String>,
}

impl WifiConfig {
    /// Convert validated persisted settings into ordered runtime Wi-Fi settings.
    pub fn from_network_config(config: &NetworkConfig) -> Result<Self, String> {
        let profiles = config.prioritized_wifi_profiles()?;
        let ssid = profiles
            .first()
            .map(|profile| profile.ssid.clone())
            .unwrap_or_default();
        let password = profiles
            .first()
            .and_then(|profile| profile.password.clone());
        Ok(Self {
            profiles,
            fallback_ap: config.fallback_ap_config()?,
            ssid,
            password,
        })
    }
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
            password: String::new(),
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

#[cfg(any(target_os = "espidf", test))]
struct ConnectingTimer {
    started: std::time::Instant,
    ready: bool,
}

#[cfg(any(target_os = "espidf", test))]
impl ConnectingTimer {
    fn new(now: std::time::Instant) -> Self {
        Self {
            started: now,
            ready: false,
        }
    }

    fn update(&mut self, up: bool, now: std::time::Instant) -> u64 {
        if !up && self.ready {
            self.started = now;
        }
        self.ready = up;
        now.duration_since(self.started).as_secs()
    }

    #[cfg(target_os = "espidf")]
    fn reset(&mut self, now: std::time::Instant) {
        self.started = now;
        self.ready = false;
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
        use esp_idf_svc::ipv4;
        use esp_idf_svc::netif::{EspNetif, NetifStack};
        use esp_idf_svc::wifi::{
            AccessPointConfiguration, AuthMethod, Configuration, EspWifi, WifiDriver,
        };
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::time::{Duration, Instant};

        // esp-idf-svc 0.51 debug-logs configuration structs that include AP
        // and STA passwords. Never permit debug-level config logging to be
        // enabled later by the application, and keep third-party debug logs
        // suppressed for the entire Wi-Fi manager lifetime.
        log::set_max_level(log::LevelFilter::Info);

        let shutdown = std::sync::Arc::new(AtomicBool::new(false));
        let config = config.unwrap_or_default();
        // Prefer the user's ordered, configured profile list to arbitrary scan RSSI.
        // A hidden SSID that is absent from a full scan remains eligible: the
        // explicit client configuration still targets that SSID directly.
        let profiles = if config.profiles.is_empty() && !config.ssid.is_empty() {
            vec![WifiProfile {
                ssid: config.ssid,
                password: config.password,
                priority: 0,
                hidden: false,
            }]
        } else {
            config.profiles
        };
        let fallback_ap = config.fallback_ap;
        if let Some(ap) = fallback_ap.as_ref() {
            ap.validate()?;
        }
        let policy = WifiPolicy::new(
            STA_CONNECT_TIMEOUT_SECS,
            fallback_ap.clone().unwrap_or_default(),
        );

        // Keep the system event loop alive for default Wi-Fi handlers and HTTP.
        let event_loop_guard = sysloop;
        #[cfg(target_os = "espidf")]
        crate::memory_diagnostics::log_heap("wifi-before-driver");
        let driver = WifiDriver::new(modem, event_loop_guard.clone(), Some(nvs))
            .map_err(|e| format!("WifiDriver::new: {e}"))?;
        let sta_netif = EspNetif::new(NetifStack::Sta)
            .map_err(|e| format!("STA network interface setup failed: {e}"))?;
        let ap_netif = if fallback_ap.is_some() {
            let mut ap_netif_conf = NetifStack::Ap.default_configuration();
            let ap_ip = ipv4::Ipv4Addr::new(192, 168, 4, 1);
            ap_netif_conf.ip_configuration =
                Some(ipv4::Configuration::Router(ipv4::RouterConfiguration {
                    subnet: ipv4::Subnet {
                        gateway: ap_ip,
                        mask: ipv4::Mask(24),
                    },
                    dhcp_enabled: true,
                    dns: Some(ap_ip),
                    secondary_dns: None,
                }));
            EspNetif::new_with_conf(&ap_netif_conf)
                .map_err(|e| format!("fallback AP network interface setup failed: {e}"))?
        } else {
            EspNetif::new(NetifStack::Ap)
                .map_err(|e| format!("AP network interface setup failed: {e}"))?
        };
        let mut wifi = EspWifi::wrap_all(driver, sta_netif, ap_netif)
            .map_err(|e| format!("EspWifi::wrap_all: {e}"))?;
        crate::memory_diagnostics::log_heap("wifi-after-driver");

        let ap_conf = fallback_ap
            .as_ref()
            .map(|ap| {
                Ok::<_, String>(AccessPointConfiguration {
                    ssid: ap
                        .ssid
                        .as_str()
                        .try_into()
                        .map_err(|_| "Fallback AP SSID too long")?,
                    password: ap
                        .password
                        .as_str()
                        .try_into()
                        .map_err(|_| "Fallback AP password too long")?,
                    auth_method: AuthMethod::WPA2Personal,
                    channel: ap.channel,
                    max_connections: ap.max_connections,
                    ..Default::default()
                })
            })
            .transpose()?;
        let clients = profiles
            .iter()
            .map(client_configuration)
            .collect::<Result<Vec<_>, _>>()?;

        let init_mode = match (clients.first(), ap_conf.as_ref()) {
            (Some(client), _) => Configuration::Client(client.clone()),
            (None, Some(ap)) => Configuration::AccessPoint(ap.clone()),
            (None, None) => Configuration::None,
        };

        set_configuration_safely(&mut wifi, &init_mode)
            .map_err(|e| format!("set_configuration: {e}"))?;
        crate::memory_diagnostics::log_heap("wifi-after-config");
        wifi.start().map_err(|e| format!("start: {e}"))?;
        crate::memory_diagnostics::log_heap("wifi-after-start");

        let power = unsafe {
            esp_idf_svc::sys::esp_wifi_set_ps(esp_idf_svc::sys::wifi_ps_type_t_WIFI_PS_NONE)
        };
        if power != esp_idf_svc::sys::ESP_OK {
            return Err(format!("Cannot disable Wi-Fi modem sleep: {power}"));
        }

        if clients.is_empty() {
            if let Some(ap) = fallback_ap.as_ref() {
                log::info!(
                    "No STA profiles configured; fallback AP '{}' is active",
                    ap.ssid
                );
            } else {
                log::info!("No Wi-Fi profiles or fallback AP configured; Wi-Fi is disabled");
            }
        }

        let stop = shutdown.clone();
        let worker = std::thread::Builder::new()
            .name("wifi-manager".into())
            .stack_size(4096)
            .spawn(move || {
                let _event_loop_guard = event_loop_guard;
                let mut connecting_timer = ConnectingTimer::new(Instant::now());
                let mut sta_ready = false;
                let mut profile_index = 0usize;
                let mut fallback_active = false;

                while !stop.load(Ordering::Relaxed) {
                    if !clients.is_empty() {
                        let client = &clients[profile_index];
                        let connected = wifi.is_connected().unwrap_or(false);
                        let up = connected && wifi.sta_netif().is_up().unwrap_or(false);

                        if up && !sta_ready {
                            log::info!("Wi-Fi STA IP ready: {:?}", wifi.sta_netif().get_ip_info().ok());
                        }

                        let now = Instant::now();
                        let elapsed = connecting_timer.update(up, now);
                        if up {
                            if fallback_active {
                                let sta_only = Configuration::Client(client.clone());
                                match set_configuration_safely(&mut wifi, &sta_only) {
                                    Ok(()) => {
                                        fallback_active = false;
                                        log::info!("Wi-Fi STA connection restored; fallback AP stopped");
                                    }
                                    Err(error) => log::error!("Failed switching back to STA only: {error}"),
                                }
                            }
                        } else if elapsed >= policy.timeout_secs {
                            let next_index = (profile_index + 1) % clients.len();
                            let next_client = &clients[next_index];
                            let enable_fallback = fallback_active || fallback_ap.is_some();
                            let next_mode = match (enable_fallback, ap_conf.as_ref()) {
                                (true, Some(ap)) => Configuration::Mixed(next_client.clone(), ap.clone()),
                                _ => Configuration::Client(next_client.clone()),
                            };
                            match set_configuration_safely(&mut wifi, &next_mode) {
                                Ok(()) => {
                                    profile_index = next_index;
                                    fallback_active = enable_fallback && ap_conf.is_some();
                                    connecting_timer.reset(now);
                                    log::warn!(
                                        "Wi-Fi connection attempt timed out; retrying configured profile {} of {}",
                                        profile_index + 1,
                                        clients.len()
                                    );
                                    if fallback_active {
                                        if let Some(ap) = fallback_ap.as_ref() {
                                            log::info!("Fallback AP '{}' is active on 192.168.4.1", ap.ssid);
                                        }
                                    }
                                    let _ = wifi.connect();
                                }
                                Err(error) => {
                                    connecting_timer.reset(now);
                                    log::error!("Failed updating Wi-Fi profile configuration: {error}");
                                }
                            }
                        } else if !connected {
                            let _ = wifi.connect();
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

#[cfg(target_os = "espidf")]
fn client_configuration(
    profile: &WifiProfile,
) -> Result<esp_idf_svc::wifi::ClientConfiguration, String> {
    use esp_idf_svc::wifi::{AuthMethod, ClientConfiguration};

    let password = profile.password.as_deref().unwrap_or_default();
    Ok(ClientConfiguration {
        ssid: profile
            .ssid
            .as_str()
            .try_into()
            .map_err(|_| "Wi-Fi SSID too long")?,
        password: password.try_into().map_err(|_| "Wi-Fi password too long")?,
        auth_method: if password.is_empty() {
            AuthMethod::None
        } else {
            AuthMethod::WPA2Personal
        },
        ..Default::default()
    })
}

#[cfg(target_os = "espidf")]
fn set_configuration_safely(
    wifi: &mut esp_idf_svc::wifi::EspWifi<'_>,
    configuration: &esp_idf_svc::wifi::Configuration,
) -> Result<(), esp_idf_svc::sys::EspError> {
    // Route configuration through this helper so the max-level guard in
    // `WifiManager::start` applies to esp-idf-svc's debug-formatted configs.
    wifi.set_configuration(configuration)
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
    fn fallback_timer_tracks_each_disconnection_not_connected_uptime() {
        use std::time::{Duration, Instant};
        let start = Instant::now();
        let mut timer = ConnectingTimer::new(start);
        let policy = WifiPolicy::default();
        let mut mode = |up, seconds| {
            let elapsed = timer.update(up, start + Duration::from_secs(seconds));
            policy.evaluate_mode(true, up, elapsed)
        };
        assert_eq!(mode(false, 17), WifiOperatingMode::StaOnly);
        assert_eq!(mode(false, 18), WifiOperatingMode::MixedApSta);
        assert_eq!(mode(true, 20), WifiOperatingMode::StaOnly);
        assert_eq!(mode(true, 3600), WifiOperatingMode::StaOnly);
        assert_eq!(mode(false, 3601), WifiOperatingMode::StaOnly);
        assert_eq!(mode(false, 3618), WifiOperatingMode::StaOnly);
        assert_eq!(mode(false, 3619), WifiOperatingMode::MixedApSta);
        assert_eq!(mode(true, 3620), WifiOperatingMode::StaOnly);
        assert_eq!(mode(false, 3621), WifiOperatingMode::StaOnly);
        assert_eq!(mode(false, 3639), WifiOperatingMode::MixedApSta);
    }

    #[test]
    fn test_fallback_ap_default_configuration() {
        let ap = FallbackApConfig::default();
        assert_eq!(ap.ssid, "JDJLIGHT");
        assert!(ap.password.is_empty());
        assert_eq!(ap.channel, 1);
        assert!(ap.validate().is_err());
    }

    #[test]
    fn test_fallback_ap_validation_rejects_invalid_credentials() {
        let mut ap = FallbackApConfig {
            ssid: String::new(),
            ..FallbackApConfig::default()
        };
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
