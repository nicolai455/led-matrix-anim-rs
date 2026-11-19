pub const FALLBACK_AP_SSID: &str = "JDJLIGHT";
/// Legacy compatibility export. A password is never supplied by firmware.
pub const FALLBACK_AP_PASSWORD: &str = "";
pub const STA_CONNECT_TIMEOUT_SECS: u64 = 18; // WLED standard timeout (~18s)
/// The driver needs time to leave the old channel before `connect()`, otherwise
/// the first association attempt races the mode switch and fails spuriously.
pub const CONFIG_SETTLE_MILLIS: u64 = 500;
/// While connected, request a background scan this often so priority upgrades
/// have fresh visibility data without the user pressing Scan.
pub const UPGRADE_SCAN_INTERVAL_SECS: u64 = 300;

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

impl WifiOperatingMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ApOnly => "ap",
            Self::StaOnly => "sta",
            Self::MixedApSta => "mixed",
        }
    }
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
}

pub struct WifiManager {
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl WifiManager {
    #[cfg(target_os = "espidf")]
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        modem: esp_idf_hal::modem::Modem,
        sysloop: esp_idf_svc::eventloop::EspSystemEventLoop,
        nvs: esp_idf_svc::nvs::EspDefaultNvsPartition,
        config: Option<WifiConfig>,
        scan_cache: std::sync::Arc<crate::net::scan::ScanCache>,
        status: std::sync::Arc<crate::net::status::WifiState>,
    ) -> Result<Self, String> {
        use esp_idf_svc::ipv4;
        use esp_idf_svc::netif::{EspNetif, NetifStack};
        use esp_idf_svc::wifi::{
            AccessPointConfiguration, AuthMethod, Configuration, EspWifi, WifiDriver,
        };
        use std::sync::atomic::AtomicBool;
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

        // Boot starts in station mode (or AP-only without profiles) exactly like
        // before. If the driver rejects that configuration, escalate to Mixed and
        // then AP-only so a rejected profile never leaves the device unreachable.
        let init_mode = match (clients.first(), ap_conf.as_ref()) {
            (Some(client), _) => Configuration::Client(client.clone()),
            (None, Some(ap)) => Configuration::AccessPoint(ap.clone()),
            (None, None) => Configuration::None,
        };
        let init_kind = match (clients.first(), ap_conf.as_ref()) {
            (Some(_), None) => WifiOperatingMode::StaOnly,
            (None, Some(_)) => WifiOperatingMode::ApOnly,
            (None, None) => WifiOperatingMode::StaOnly,
            (Some(_), Some(_)) => WifiOperatingMode::StaOnly,
        };

        set_configuration_safely(&mut wifi, &init_mode)
            .map_err(|e| format!("set_configuration: {e}"))?;
        crate::memory_diagnostics::log_heap("wifi-after-config");
        wifi.start().map_err(|e| format!("start: {e}"))?;
        crate::memory_diagnostics::log_heap("wifi-after-start");
        if !clients.is_empty() {
            // Same settle delay as every later switch (see CONFIG_SETTLE_MILLIS).
            std::thread::sleep(Duration::from_millis(CONFIG_SETTLE_MILLIS));
            if let Err(error) = wifi.connect() {
                log::error!("Initial Wi-Fi connect failed: {error}");
            }
        }

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
        let mut initial_status = crate::net::status::WifiStatus {
            mode: if clients.is_empty() && ap_conf.is_none() {
                "off".into()
            } else {
                init_kind.as_str().into()
            },
            ap_active: ap_conf.is_some(),
            ..Default::default()
        };
        initial_status.profiles = profiles
            .iter()
            .map(|profile| crate::net::status::ProfileStatus {
                ssid: profile.ssid.clone(),
                priority: profile.priority,
                state: "idle".into(),
            })
            .collect();
        status.set_status(initial_status);
        status.set_station_subnet(None);

        let worker = std::thread::Builder::new()
            .name("wifi-manager".into())
            .stack_size(8192)
            .spawn(move || {
                let _event_loop_guard = event_loop_guard;
                run_worker(
                    &mut wifi,
                    clients,
                    ap_conf,
                    fallback_ap,
                    profiles,
                    policy,
                    scan_cache,
                    status,
                    stop,
                    Instant::now(),
                );
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

/// Worker loop. Timing decisions live in `ConnectPolicy` (host-tested); this
/// function only performs driver I/O and publishes the status snapshot.
#[cfg(target_os = "espidf")]
#[allow(clippy::too_many_arguments)]
fn run_worker(
    wifi: &mut esp_idf_svc::wifi::EspWifi<'_>,
    clients: Vec<esp_idf_svc::wifi::ClientConfiguration>,
    ap_conf: Option<esp_idf_svc::wifi::AccessPointConfiguration>,
    fallback_ap: Option<FallbackApConfig>,
    profiles: Vec<WifiProfile>,
    policy: WifiPolicy,
    scan_cache: std::sync::Arc<crate::net::scan::ScanCache>,
    status: std::sync::Arc<crate::net::status::WifiState>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    started: std::time::Instant,
) {
    use crate::net::connect_policy::{ConnectPolicy, ProfileState};
    use crate::net::status::{StaStatus, WifiStatus};
    use esp_idf_svc::wifi::Configuration;
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};

    let mut policy_state = ConnectPolicy::new(&profiles);
    let mut attempt: Option<(usize, Instant)> = None;
    let mut fallback_active = ap_conf.is_some() && clients.is_empty();
    let mut applied_mode = if ap_conf.is_some() {
        if clients.is_empty() {
            WifiOperatingMode::ApOnly
        } else {
            WifiOperatingMode::MixedApSta
        }
    } else {
        WifiOperatingMode::StaOnly
    };
    let mut last_status = Instant::now() - Duration::from_secs(5);
    let mut last_published_up = false;
    let mut last_self_scan = started;
    let mut timer = ConnectingTimer::new(started);
    if fallback_ap.is_some() && !clients.is_empty() {
        log::info!("Fallback AP stays active; station connects in the background");
    }

    while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        let now_secs = now.duration_since(started).as_secs();

        // --- 1. Blocking scan on behalf of the HTTP layer -----------------
        // Never while an association attempt is in flight: a scan switches the
        // radio channel and would abort the very connect we are waiting on.
        if scan_cache.is_pending() && attempt.is_none() {
            match wifi.scan() {
                Ok(found) => {
                    let networks = found
                        .into_iter()
                        .map(|ap| crate::net::scan::ScannedNetwork {
                            ssid: ap.ssid.to_string(),
                            rssi: ap.signal_strength,
                            auth: auth_label(ap.auth_method),
                            channel: ap.channel,
                        })
                        .collect::<Vec<_>>();
                    scan_cache.store(networks, Instant::now());
                    log::info!("Wi-Fi scan finished");
                }
                Err(error) => {
                    scan_cache.finish_request();
                    log::error!("Wi-Fi scan failed: {error}");
                }
            }
        }

        // --- 2. Observe the link ------------------------------------------
        let connected = wifi.is_connected().unwrap_or(false);
        let up = connected && wifi.sta_netif().is_up().unwrap_or(false);
        let _elapsed = timer.update(up, now);

        if up {
            if policy_state.connected().is_none() {
                match attempt.take() {
                    Some((index, _)) => policy_state.record_connected(index, now_secs),
                    None => {
                        if let Some(active) = policy_state.active() {
                            policy_state.record_connected(active, now_secs);
                        }
                    }
                }
                log::info!("Wi-Fi STA link is up");
            }
            if fallback_active {
                if let Some(client) = policy_state
                    .connected()
                    .and_then(|index| clients.get(index))
                    .cloned()
                {
                    // Keep the AP: dropping it would remove the only documented
                    // recovery path while the station link is still young.
                    if applied_mode == WifiOperatingMode::ApOnly {
                        match set_configuration_safely(
                            wifi,
                            &Configuration::Client(client),
                        ) {
                            Ok(()) => {
                                applied_mode = WifiOperatingMode::StaOnly;
                                fallback_active = false;
                                log::info!("Fallback AP stopped; station-only mode");
                            }
                            Err(error) => log::error!("Cannot leave AP-only mode: {error}"),
                        }
                    }
                }
            }
        } else if policy_state.connected().is_some() {
            // Link drop after a working connection: no failure penalty.
            policy_state.record_disconnected(now_secs);
            attempt = None;
            timer = ConnectingTimer::new(now);
            log::warn!("Wi-Fi STA link lost; retrying the same profile");
        }

        // --- 3. Timeout of the running attempt ----------------------------
        if let Some((index, at)) = attempt {
            if now.duration_since(at).as_secs() >= policy.timeout_secs {
                policy_state.record_failure(index, now_secs);
                let exhausted = policy_state.state_of(index) == Some(ProfileState::Failed);
                attempt = None;
                timer = ConnectingTimer::new(now);
                if exhausted {
                    log::error!(
                        "Wi-Fi profile {} failed {} times; giving up on it",
                        index + 1,
                        crate::net::connect_policy::FAILURES_BEFORE_FAILED
                    );
                } else {
                    log::warn!(
                        "Wi-Fi profile {} timed out; next attempt in {} s",
                        index + 1,
                        crate::net::connect_policy::backoff_for(policy_state.failures(index))
                    );
                }
            }
        }

        // --- 4. Start the next attempt ------------------------------------
        if !up && attempt.is_none() && !clients.is_empty() {
            if let Some(target) = policy_state.next_target(now_secs) {
                let client = clients[target].clone();
                let mode = if ap_conf.is_some() {
                    WifiOperatingMode::MixedApSta
                } else {
                    WifiOperatingMode::StaOnly
                };
                match configure_and_connect(wifi, &client, ap_conf.as_ref(), mode) {
                    Ok(applied) => {
                        applied_mode = applied;
                        fallback_active = matches!(applied, WifiOperatingMode::MixedApSta);
                        // AP-only means the driver refused station mode; do not
                        // start a timeout for an attempt that cannot happen.
                        if applied != WifiOperatingMode::ApOnly {
                            policy_state.begin_attempt(target, now_secs);
                            attempt = Some((target, Instant::now()));
                            timer = ConnectingTimer::new(Instant::now());
                            log::info!(
                                "Wi-Fi connecting to profile {} of {} ({})",
                                target + 1,
                                clients.len(),
                                applied.as_str()
                            );
                        }
                    }
                    Err(error) => {
                        // Count it so a permanently broken configuration cannot spin.
                        policy_state.record_failure(target, now_secs);
                        timer = ConnectingTimer::new(now);
                        log::error!("Failed to apply Wi-Fi profile {}: {error}", target + 1);
                    }
                }
            }
        }

        // --- 5. Priority upgrade ------------------------------------------
        if up {
            if let Some(target) = policy_state.upgrade_target(now_secs, |ssid| {
                scan_cache
                    .last()
                    .is_some_and(|(_, networks)| networks.iter().any(|network| network.ssid == ssid))
            }) {
                let client = clients[target].clone();
                let mode = if ap_conf.is_some() {
                    WifiOperatingMode::MixedApSta
                } else {
                    WifiOperatingMode::StaOnly
                };
                match configure_and_connect(wifi, &client, ap_conf.as_ref(), mode) {
                    Ok(applied) => {
                        applied_mode = applied;
                        fallback_active = matches!(applied, WifiOperatingMode::MixedApSta);
                        attempt = Some((target, Instant::now()));
                        timer = ConnectingTimer::new(Instant::now());
                        log::info!(
                            "Switching to higher priority Wi-Fi profile {} ({})",
                            target + 1,
                            applied.as_str()
                        );
                    }
                    Err(error) => log::error!("Priority switch failed: {error}"),
                }
            }
            // Refresh visibility data occasionally; this briefly interrupts the
            // station link exactly like a manual scan.
            if now.duration_since(last_self_scan).as_secs() >= UPGRADE_SCAN_INTERVAL_SECS
                && !scan_cache.is_pending()
            {
                last_self_scan = now;
                scan_cache.request();
            }
        }

        // --- 6. Publish the snapshot --------------------------------------
        if now.duration_since(last_status).as_secs() >= 1 || up != last_published_up {
            last_status = now;
            last_published_up = up;
            let (ssid, rssi, ip, subnet) = if up {
                let ap_info = wifi.get_ap_info().ok();
                let ip_info = wifi.sta_netif().get_ip_info().ok();
                let ip = ip_info
                    .as_ref()
                    .map(|info| info.ip.to_string())
                    .unwrap_or_default();
                let subnet = ip_info
                    .as_ref()
                    .filter(|info| !info.ip.is_unspecified())
                    .map(|info| (info.ip, info.subnet.mask.0));
                (
                    ap_info.as_ref().map(|ap| ap.ssid.to_string()).unwrap_or_default(),
                    ap_info.as_ref().map(|ap| ap.signal_strength).unwrap_or(0),
                    ip,
                    subnet,
                )
            } else {
                (String::new(), 0, String::new(), None)
            };
            status.set_station_subnet(subnet);
            status.set_status(WifiStatus {
                mode: if clients.is_empty() && ap_conf.is_none() {
                    "off".into()
                } else {
                    applied_mode.as_str().into()
                },
                sta: StaStatus {
                    connected: up,
                    ssid,
                    rssi,
                    ip,
                },
                ap_active: ap_conf.is_some(),
                ap_clients: if ap_conf.is_some() {
                    ap_client_count()
                } else {
                    0
                },
                profiles: policy_state.statuses(),
            });
        }

        // Short tick keeps Scan latency acceptable without busy-waiting.
        for _ in 0..4 {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    let _ = fallback_ap;
}

/// Apply a station configuration and start the association.
///
/// `set_configuration` fails when the driver rejects the mode switch. The device
/// must never end up with neither station nor AP, so a failure falls back to
/// Mixed (client + AP) and finally AP-only instead of leaving the AP off. After
/// a success the driver gets `CONFIG_SETTLE_MILLIS` before `connect()`.
#[cfg(target_os = "espidf")]
fn configure_and_connect(
    wifi: &mut esp_idf_svc::wifi::EspWifi<'_>,
    client: &esp_idf_svc::wifi::ClientConfiguration,
    ap: Option<&esp_idf_svc::wifi::AccessPointConfiguration>,
    mode: WifiOperatingMode,
) -> Result<WifiOperatingMode, String> {
    use esp_idf_svc::wifi::Configuration;
    use std::time::Duration;

    let primary = match (mode, ap) {
        (WifiOperatingMode::MixedApSta, Some(ap)) => Configuration::Mixed(client.clone(), ap.clone()),
        _ => Configuration::Client(client.clone()),
    };
    let mut applied = mode;
    if let Err(error) = set_configuration_safely(wifi, &primary) {
        log::error!("set_configuration failed ({error}); keeping the fallback AP reachable");
        let _ = wifi.start();
        match ap {
            Some(ap) => {
                if set_configuration_safely(wifi, &Configuration::Mixed(client.clone(), ap.clone()))
                    .is_ok()
                {
                    applied = WifiOperatingMode::MixedApSta;
                } else if set_configuration_safely(wifi, &Configuration::AccessPoint(ap.clone()))
                    .is_ok()
                {
                    let _ = wifi.start();
                    // Nothing to associate with: the AP is the only interface.
                    return Ok(WifiOperatingMode::ApOnly);
                } else {
                    return Err(format!("set_configuration failed: {error}"));
                }
            }
            None => return Err(format!("set_configuration failed: {error}")),
        }
    }
    let _ = wifi.start();
    std::thread::sleep(Duration::from_millis(CONFIG_SETTLE_MILLIS));
    wifi.connect().map_err(|error| format!("connect: {error}"))?;
    Ok(applied)
}

#[cfg(target_os = "espidf")]
fn auth_label(auth: Option<embedded_svc::wifi::AuthMethod>) -> String {
    use embedded_svc::wifi::AuthMethod;
    match auth {
        None | Some(AuthMethod::None) => "open".into(),
        Some(AuthMethod::WEP) => "WEP".into(),
        Some(AuthMethod::WPA) => "WPA".into(),
        Some(AuthMethod::WPA2Personal) => "WPA2".into(),
        Some(AuthMethod::WPAWPA2Personal) => "WPA/WPA2".into(),
        Some(AuthMethod::WPA2Enterprise) => "WPA2-Enterprise".into(),
        Some(AuthMethod::WPA3Personal) => "WPA3".into(),
        Some(AuthMethod::WPA2WPA3Personal) => "WPA2/WPA3".into(),
        Some(AuthMethod::WAPIPersonal) => "WAPI".into(),
    }
}

/// Number of clients associated with the fallback AP. Read-only syscall; a
/// failure (AP disabled, driver busy) is reported as zero.
#[cfg(target_os = "espidf")]
fn ap_client_count() -> u32 {
    let mut list = std::mem::MaybeUninit::<esp_idf_sys::wifi_sta_list_t>::zeroed();
    let result = unsafe { esp_idf_sys::esp_wifi_ap_get_sta_list(list.as_mut_ptr()) };
    if result != esp_idf_sys::ESP_OK {
        return 0;
    }
    let list = unsafe { list.assume_init() };
    u32::try_from(list.num).unwrap_or(0)
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

    #[test]
    fn settle_delay_is_short_but_non_zero() {
        assert!((500..=2000).contains(&CONFIG_SETTLE_MILLIS));
    }
}
