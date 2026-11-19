//! Wi-Fi scan cache shared between the HTTP server and the Wi-Fi worker.
//!
//! `esp_wifi_scan_start` blocks for roughly two seconds, so the HTTP handler
//! never scans: it drops a request flag and reads the cache. The worker thread
//! performs the blocking scan and stores a normalized result.
//!
//! Responses are deliberately small and privacy-preserving: at most 20
//! networks, SSIDs truncated to 32 bytes, BSSIDs never included.
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const MAX_SCAN_NETWORKS: usize = 20;
pub const MAX_SSID_BYTES: usize = 32;
/// Cached results older than this are reported as `stale` with an empty list.
pub const SCAN_MAX_AGE: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannedNetwork {
    pub ssid: String,
    pub rssi: i8,
    pub auth: String,
    pub channel: u8,
}

impl ScannedNetwork {
    pub fn to_json(&self) -> Value {
        // No BSSID, no protocol bitmap, no vendor data: SSID and link quality
        // are all the UI needs to offer a profile.
        json!({
            "ssid": self.ssid,
            "rssi": self.rssi,
            "auth": self.auth,
            "channel": self.channel,
        })
    }
}

/// Truncate an SSID to `MAX_SSID_BYTES` on a character boundary. Returns `None`
/// for empty or NUL-only SSIDs (hidden networks are not offered).
pub fn truncate_ssid(ssid: &str) -> Option<String> {
    let ssid = ssid.split('\0').next().unwrap_or("");
    if ssid.is_empty() {
        return None;
    }
    let mut out = String::new();
    for character in ssid.chars() {
        if out.len() + character.len_utf8() > MAX_SSID_BYTES {
            break;
        }
        out.push(character);
    }
    (!out.is_empty()).then_some(out)
}

/// Drop hidden/empty SSIDs, truncate, keep the strongest duplicate, sort by
/// signal strength and cap the list.
pub fn normalize(entries: impl IntoIterator<Item = ScannedNetwork>) -> Vec<ScannedNetwork> {
    let mut unique: Vec<ScannedNetwork> = Vec::new();
    for entry in entries {
        let Some(ssid) = truncate_ssid(&entry.ssid) else {
            continue;
        };
        let candidate = ScannedNetwork { ssid, ..entry };
        match unique.iter_mut().find(|known| known.ssid == candidate.ssid) {
            Some(known) if candidate.rssi > known.rssi => *known = candidate,
            Some(_) => {}
            None => unique.push(candidate),
        }
    }
    unique.sort_by(|a, b| b.rssi.cmp(&a.rssi).then_with(|| a.ssid.cmp(&b.ssid)));
    unique.truncate(MAX_SCAN_NETWORKS);
    unique
}

#[derive(Default)]
struct Inner {
    requested: bool,
    last: Option<(Instant, Vec<ScannedNetwork>)>,
}

pub struct ScanCache {
    inner: Mutex<Inner>,
}

impl Default for ScanCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ScanCache {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
        }
    }

    /// Ask the worker for a fresh scan. Idempotent while a scan is pending.
    pub fn request(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.requested = true;
        }
    }

    /// Consumed by the worker; returns `true` when a scan should be started.
    pub fn take_request(&self) -> bool {
        self.inner
            .lock()
            .map(|mut inner| std::mem::take(&mut inner.requested))
            .unwrap_or(false)
    }

    pub fn is_pending(&self) -> bool {
        self.inner.lock().map(|inner| inner.requested).unwrap_or(false)
    }

    pub fn store(&self, networks: impl IntoIterator<Item = ScannedNetwork>, now: Instant) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.requested = false;
            inner.last = Some((now, normalize(networks)));
        }
    }

    pub fn last(&self) -> Option<(Instant, Vec<ScannedNetwork>)> {
        self.inner.lock().ok().and_then(|inner| inner.last.clone())
    }

    /// Drop a pending request without producing a result (scan failed). The UI
    /// then keeps polling and sees `stale: true` until a scan succeeds.
    pub fn finish_request(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.requested = false;
        }
    }

    /// Snapshot for `GET /api/network/scan`. Older results are withheld rather
    /// than presented as current: `stale` plus an empty list tells the UI to
    /// keep polling.
    pub fn json(&self, now: Instant, connection_may_drop: bool) -> Value {
        let Ok(inner) = self.inner.lock() else {
            return json!({
                "networks": [],
                "stale": true,
                "scanning": false,
                "connectionMayDrop": connection_may_drop,
            });
        };
        let pending = inner.requested;
        match inner.last.as_ref() {
            Some((at, networks)) if now.saturating_duration_since(*at) <= SCAN_MAX_AGE => json!({
                "networks": networks.iter().map(ScannedNetwork::to_json).collect::<Vec<_>>(),
                "stale": false,
                "scanning": pending,
                "connectionMayDrop": connection_may_drop,
            }),
            _ => json!({
                "networks": [],
                "stale": true,
                "scanning": pending,
                "connectionMayDrop": connection_may_drop,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn network(ssid: &str, rssi: i8, channel: u8) -> ScannedNetwork {
        ScannedNetwork {
            ssid: ssid.to_string(),
            rssi,
            auth: "WPA2".into(),
            channel,
        }
    }

    #[test]
    fn normalization_dedupes_keeps_the_strongest_and_sorts_by_signal() {
        let merged = normalize(vec![
            network("home", -70, 1),
            network("home", -40, 1),
            network("garden", -55, 6),
        ]);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].ssid, "home");
        assert_eq!(merged[0].rssi, -40);
        assert_eq!(merged[1].ssid, "garden");
    }

    #[test]
    fn normalization_caps_the_list_and_drops_hidden_networks() {
        let mut entries: Vec<ScannedNetwork> = (0..40)
            .map(|index| network(&format!("net{index:02}"), -(index as i8), 1))
            .collect();
        entries.push(network("", -10, 3));
        let merged = normalize(entries);
        assert_eq!(merged.len(), MAX_SCAN_NETWORKS);
        assert!(merged.iter().all(|entry| !entry.ssid.is_empty()));
    }

    #[test]
    fn ssids_are_truncated_to_32_bytes_on_a_character_boundary() {
        let long = "abcdefghijklmnopqrstuvwxyz0123456789";
        assert_eq!(truncate_ssid(long).unwrap().len(), MAX_SSID_BYTES);
        // 11 × 3 bytes = 33 bytes → 10 characters fit.
        let umlauts = "€€€€€€€€€€€";
        let truncated = truncate_ssid(umlauts).unwrap();
        assert_eq!(truncated.len(), 30);
        assert_eq!(truncated.chars().count(), 10);
        assert_eq!(truncate_ssid(""), None);
        assert_eq!(truncate_ssid("\0hidden"), None);
        assert_eq!(truncate_ssid("ok\0junk").unwrap(), "ok");
    }

    #[test]
    fn json_never_exposes_bssids_and_reports_staleness() {
        let cache = ScanCache::new();
        let start = Instant::now();
        assert_eq!(cache.json(start, false)["stale"], true);
        assert_eq!(cache.json(start, false)["networks"].as_array().unwrap().len(), 0);

        cache.store(vec![network("home", -52, 6)], start);
        let fresh = cache.json(start + Duration::from_secs(1), true);
        assert_eq!(fresh["stale"], false);
        assert_eq!(fresh["connectionMayDrop"], true);
        assert_eq!(fresh["networks"][0]["ssid"], "home");
        assert_eq!(fresh["networks"][0]["rssi"], -52);
        assert_eq!(fresh["networks"][0]["channel"], 6);
        assert!(fresh["networks"][0].get("bssid").is_none());

        let stale = cache.json(start + SCAN_MAX_AGE + Duration::from_millis(1), false);
        assert_eq!(stale["stale"], true);
        assert_eq!(stale["networks"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn requests_are_consumed_exactly_once() {
        let cache = ScanCache::new();
        assert!(!cache.take_request());
        cache.request();
        cache.request();
        assert!(cache.is_pending());
        assert!(cache.take_request());
        assert!(!cache.take_request());
        assert!(!cache.is_pending());
        // Storing a result clears any pending flag.
        cache.request();
        cache.store(Vec::new(), Instant::now());
        assert!(!cache.is_pending());
    }
}
