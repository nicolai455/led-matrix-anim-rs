//! Runtime Wi-Fi state shared with the HTTP layer (`/api/state`, `/api/firmware`).
//!
//! The Wi-Fi worker publishes a snapshot roughly every second; HTTP handlers
//! only read it, so no handler ever touches the driver.
use serde_json::{json, Value};
use std::net::Ipv4Addr;
use std::sync::RwLock;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StaStatus {
    pub connected: bool,
    pub ssid: String,
    pub rssi: i8,
    pub ip: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileStatus {
    pub ssid: String,
    pub priority: u8,
    pub state: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WifiStatus {
    pub mode: String,
    pub sta: StaStatus,
    pub ap_active: bool,
    pub ap_clients: u32,
    pub profiles: Vec<ProfileStatus>,
}

impl Default for WifiStatus {
    fn default() -> Self {
        Self {
            mode: "off".into(),
            sta: StaStatus::default(),
            ap_active: false,
            ap_clients: 0,
            profiles: Vec::new(),
        }
    }
}

impl WifiStatus {
    pub fn to_json(&self) -> Value {
        json!({
            "mode": self.mode,
            "sta": {
                "connected": self.sta.connected,
                "ssid": self.sta.ssid,
                "rssi": self.sta.rssi,
                "ip": self.sta.ip,
            },
            "ap": {
                "active": self.ap_active,
                "clients": self.ap_clients,
            },
            "profiles": self.profiles.iter().map(|profile| json!({
                "ssid": profile.ssid,
                "priority": profile.priority,
                "state": profile.state,
            })).collect::<Vec<_>>(),
        })
    }

    pub fn is_mixed(&self) -> bool {
        self.mode == "mixed"
    }
}

/// Single shared handle. `station_subnet` is the device's own address plus
/// prefix length; it is what makes the home network reachable for management.
#[derive(Default)]
pub struct WifiState {
    status: RwLock<WifiStatus>,
    station_subnet: RwLock<Option<(Ipv4Addr, u8)>>,
}

impl WifiState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_status(&self, status: WifiStatus) {
        if let Ok(mut current) = self.status.write() {
            *current = status;
        }
    }

    pub fn status(&self) -> WifiStatus {
        self.status.read().map(|status| status.clone()).unwrap_or_default()
    }

    pub fn set_station_subnet(&self, subnet: Option<(Ipv4Addr, u8)>) {
        if let Ok(mut current) = self.station_subnet.write() {
            *current = subnet;
        }
    }

    pub fn station_subnet(&self) -> Option<(Ipv4Addr, u8)> {
        self.station_subnet.read().ok().and_then(|value| *value)
    }

    pub fn json(&self) -> Value {
        self.status().to_json()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_shape_matches_the_documented_contract() {
        let state = WifiState::new();
        state.set_status(WifiStatus {
            mode: "mixed".into(),
            sta: StaStatus {
                connected: true,
                ssid: "home".into(),
                rssi: -52,
                ip: "192.168.178.35".into(),
            },
            ap_active: true,
            ap_clients: 1,
            profiles: vec![ProfileStatus {
                ssid: "home".into(),
                priority: 10,
                state: "connected".into(),
            }],
        });
        let json = state.json();
        assert_eq!(json["mode"], "mixed");
        assert_eq!(json["sta"]["connected"], true);
        assert_eq!(json["sta"]["ssid"], "home");
        assert_eq!(json["sta"]["rssi"], -52);
        assert_eq!(json["sta"]["ip"], "192.168.178.35");
        assert_eq!(json["ap"]["active"], true);
        assert_eq!(json["ap"]["clients"], 1);
        assert_eq!(json["profiles"][0]["ssid"], "home");
        assert_eq!(json["profiles"][0]["priority"], 10);
        assert_eq!(json["profiles"][0]["state"], "connected");
        assert!(state.status().is_mixed());
    }

    #[test]
    fn default_status_is_disabled_and_subnet_is_optional() {
        let state = WifiState::new();
        assert_eq!(state.json()["mode"], "off");
        assert_eq!(state.json()["sta"]["connected"], false);
        assert!(state.station_subnet().is_none());
        state.set_station_subnet(Some((Ipv4Addr::new(192, 168, 178, 35), 24)));
        assert_eq!(state.station_subnet(), Some((Ipv4Addr::new(192, 168, 178, 35), 24)));
        state.set_station_subnet(None);
        assert!(state.station_subnet().is_none());
    }
}
