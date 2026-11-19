//! Pure connection policy for the Wi-Fi worker: profile rotation, backoff and
//! priority upgrades. No ESP-IDF types, so the whole state machine is testable
//! on the host with a logical clock in seconds.
//!
//! Timing contract: every attempt gets `STA_CONNECT_TIMEOUT_SECS` to associate.
//! A failed attempt is retried after an exponential backoff (18 s, 36 s, 72 s,
//! … capped at 300 s) and the profile is marked `failed` after
//! `FAILURES_BEFORE_FAILED` unsuccessful attempts. While connected, a higher
//! priority profile is adopted at most once per `PRIORITY_RECHECK_SECS` when it
//! is visible in the scan cache.
use crate::network_config::WifiProfile;
use crate::net::status::ProfileStatus;

pub const INITIAL_BACKOFF_SECS: u64 = 18;
pub const MAX_BACKOFF_SECS: u64 = 300;
pub const FAILURES_BEFORE_FAILED: u32 = 3;
pub const PRIORITY_RECHECK_SECS: u64 = 60;

/// `18, 36, 72, 144, 288, 300, 300, …` — the retry delay after the n-th
/// consecutive failure of one profile.
pub fn backoff_for(failures: u32) -> u64 {
    if failures == 0 {
        return 0;
    }
    let exponent = failures.min(8) - 1;
    INITIAL_BACKOFF_SECS
        .saturating_mul(1u64 << exponent.min(20))
        .min(MAX_BACKOFF_SECS)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileState {
    Idle,
    Connecting,
    Connected,
    Failed,
}

impl ProfileState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug)]
struct Track {
    ssid: String,
    priority: u8,
    state: ProfileState,
    failures: u32,
    /// Logical second (worker uptime) from which a retry is allowed.
    ready_at: u64,
}

#[derive(Clone, Debug)]
pub struct ConnectPolicy {
    tracks: Vec<Track>,
    active: Option<usize>,
    connected: Option<usize>,
    last_upgrade_check: Option<u64>,
}

impl ConnectPolicy {
    pub fn new(profiles: &[WifiProfile]) -> Self {
        Self {
            tracks: profiles
                .iter()
                .map(|profile| Track {
                    ssid: profile.ssid.clone(),
                    priority: profile.priority,
                    state: ProfileState::Idle,
                    failures: 0,
                    ready_at: 0,
                })
                .collect(),
            active: None,
            connected: None,
            last_upgrade_check: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn ssid(&self, index: usize) -> Option<&str> {
        self.tracks.get(index).map(|track| track.ssid.as_str())
    }

    pub fn active(&self) -> Option<usize> {
        self.active
    }

    pub fn connected(&self) -> Option<usize> {
        self.connected
    }

    /// Profile to configure next. The current profile is preferred while its
    /// backoff allows another attempt; otherwise the next eligible profile in
    /// rotation is used, so one misconfigured profile cannot block a working
    /// one. `failed` profiles and profiles still backing off are skipped.
    pub fn next_target(&self, now: u64) -> Option<usize> {
        if self.connected.is_some() || self.tracks.is_empty() {
            return None;
        }
        let eligible = |index: usize| {
            self.tracks.get(index).is_some_and(|track| {
                track.state != ProfileState::Failed
                    && track.state != ProfileState::Connected
                    && track.ready_at <= now
            })
        };
        if self.active.is_some_and(eligible) {
            return self.active;
        }
        let start = self.active.map(|index| index + 1).unwrap_or(0);
        (0..self.tracks.len())
            .map(|offset| (start + offset) % self.tracks.len())
            .find(|index| eligible(*index))
    }

    /// Mark that the driver was pointed at `index` (configuration + connect).
    pub fn begin_attempt(&mut self, index: usize, now: u64) {
        if let Some(track) = self.tracks.get_mut(index) {
            track.state = ProfileState::Connecting;
            track.ready_at = now;
        }
        self.active = Some(index);
    }

    pub fn record_connected(&mut self, index: usize, _now: u64) {
        if let Some(track) = self.tracks.get_mut(index) {
            track.state = ProfileState::Connected;
            track.failures = 0;
            track.ready_at = 0;
        }
        // Any previously connected profile returns to `idle` so it can be
        // retried if the new link drops.
        if let Some(previous) = self.connected.filter(|previous| *previous != index) {
            if let Some(track) = self.tracks.get_mut(previous) {
                track.state = ProfileState::Idle;
            }
        }
        self.connected = Some(index);
        self.active = Some(index);
    }

    /// The link dropped; retry the same profile immediately, without penalty.
    pub fn record_disconnected(&mut self, now: u64) {
        if let Some(index) = self.connected.take() {
            if let Some(track) = self.tracks.get_mut(index) {
                track.state = ProfileState::Idle;
                track.ready_at = now;
            }
        }
    }

    /// A timeout or driver error for `index`. After three failures the profile
    /// stops being tried until the next restart or configuration change.
    pub fn record_failure(&mut self, index: usize, now: u64) {
        let Some(track) = self.tracks.get_mut(index) else {
            return;
        };
        track.failures = track.failures.saturating_add(1);
        if self.connected == Some(index) {
            self.connected = None;
        }
        if track.failures >= FAILURES_BEFORE_FAILED {
            track.state = ProfileState::Failed;
            track.ready_at = u64::MAX;
        } else {
            track.state = ProfileState::Idle;
            track.ready_at = now.saturating_add(backoff_for(track.failures));
        }
    }

    pub fn state_of(&self, index: usize) -> Option<ProfileState> {
        self.tracks.get(index).map(|track| track.state)
    }

    pub fn failures(&self, index: usize) -> u32 {
        self.tracks.get(index).map(|track| track.failures).unwrap_or(0)
    }

    /// True when every profile is `failed` — the caller should stay in the
    /// fallback AP and stop reconfiguring the station.
    pub fn all_failed(&self) -> bool {
        !self.tracks.is_empty() && self.tracks.iter().all(|track| track.state == ProfileState::Failed)
    }

    /// Decide whether a better profile should be adopted.
    ///
    /// Returns `Some(index)` for a strictly higher priority profile that is
    /// visible (`is_visible`) and healthy. Checks are rate limited to one per
    /// `PRIORITY_RECHECK_SECS`; the first call arms the timer, because the
    /// freshly connected profile should not be abandoned immediately.
    pub fn upgrade_target(&mut self, now: u64, is_visible: impl Fn(&str) -> bool) -> Option<usize> {
        let current = self.connected?;
        match self.last_upgrade_check {
            None => {
                self.last_upgrade_check = Some(now);
                return None;
            }
            Some(last) if now.saturating_sub(last) < PRIORITY_RECHECK_SECS => return None,
            _ => {}
        }
        self.last_upgrade_check = Some(now);
        let current_priority = self.tracks.get(current).map(|track| track.priority).unwrap_or(0);
        self.tracks.iter().enumerate().find_map(|(index, track)| {
            (index != current
                && track.priority > current_priority
                && track.state != ProfileState::Failed
                && is_visible(&track.ssid))
            .then_some(index)
        })
    }

    /// Snapshot for `/api/state`. The connected profile wins over a stale
    /// `Connecting` flag left behind by a concurrent transition.
    pub fn statuses(&self) -> Vec<ProfileStatus> {
        self.tracks
            .iter()
            .enumerate()
            .map(|(index, track)| ProfileStatus {
                ssid: track.ssid.clone(),
                priority: track.priority,
                state: if self.connected == Some(index) {
                    ProfileState::Connected.as_str().to_string()
                } else {
                    track.state.as_str().to_string()
                },
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(ssid: &str, priority: u8) -> WifiProfile {
        WifiProfile {
            ssid: ssid.into(),
            password: Some("fake-password".into()),
            priority,
            hidden: false,
        }
    }

    #[test]
    fn backoff_doubles_and_stops_at_five_minutes() {
        assert_eq!(backoff_for(0), 0);
        assert_eq!(backoff_for(1), 18);
        assert_eq!(backoff_for(2), 36);
        assert_eq!(backoff_for(3), 72);
        assert_eq!(backoff_for(4), 144);
        assert_eq!(backoff_for(5), 288);
        assert_eq!(backoff_for(6), 300);
        assert_eq!(backoff_for(50), 300);
    }

    #[test]
    fn a_wrong_password_retries_at_18_36_72_then_marks_failed() {
        let mut policy = ConnectPolicy::new(&[profile("home", 10)]);
        let mut now = 0u64;
        policy.begin_attempt(0, now);
        now += 18;
        policy.record_failure(0, now);
        assert_eq!(policy.state_of(0), Some(ProfileState::Idle));
        assert_eq!(policy.failures(0), 1);
        assert_eq!(policy.next_target(now), None, "backoff must block an immediate retry");
        assert_eq!(policy.next_target(now + 17), None);
        assert_eq!(policy.next_target(now + 18), Some(0), "retry after 18 s of backoff");

        now += 18;
        policy.begin_attempt(0, now);
        now += 18;
        policy.record_failure(0, now);
        assert_eq!(policy.next_target(now + 35), None);
        assert_eq!(policy.next_target(now + 36), Some(0), "retry after 36 s of backoff");

        now += 36;
        policy.begin_attempt(0, now);
        now += 18;
        policy.record_failure(0, now);
        assert_eq!(policy.state_of(0), Some(ProfileState::Failed));
        assert_eq!(policy.failures(0), 3);
        assert_eq!(policy.next_target(now + 300), None, "a failed profile is not retried");
        assert!(policy.all_failed());
    }

    #[test]
    fn a_failing_profile_hands_over_to_the_next_one_immediately() {
        let mut policy = ConnectPolicy::new(&[profile("home", 10), profile("garden", 5)]);
        policy.begin_attempt(0, 0);
        policy.record_failure(0, 18);
        assert_eq!(policy.next_target(18), Some(1), "profile 2 takes over while 1 backs off");
        policy.begin_attempt(1, 18);
        policy.record_connected(1, 25);
        assert_eq!(policy.statuses()[0].state, "idle");
        assert_eq!(policy.statuses()[1].state, "connected");
    }

    #[test]
    fn profiles_rotate_one_to_two_to_three_and_back_to_one() {
        let mut policy = ConnectPolicy::new(&[
            profile("one", 30),
            profile("two", 20),
            profile("three", 10),
        ]);
        let mut now = 0u64;
        let mut order = Vec::new();
        for _ in 0..4 {
            let index = policy.next_target(now).expect("a profile must be eligible");
            order.push(index);
            policy.begin_attempt(index, now);
            now += 18;
            policy.record_failure(index, now);
        }
        assert_eq!(order, vec![0, 1, 2, 0]);
        // Each profile has failed once, none is exhausted yet.
        assert!(!policy.all_failed());
    }

    #[test]
    fn a_successful_connection_resets_failures_and_reports_status() {
        let mut policy = ConnectPolicy::new(&[profile("home", 10)]);
        policy.begin_attempt(0, 0);
        policy.record_failure(0, 18);
        policy.begin_attempt(0, 36);
        policy.record_connected(0, 54);
        assert_eq!(policy.failures(0), 0);
        assert_eq!(policy.connected(), Some(0));
        assert_eq!(policy.next_target(60), None, "a connected profile is stable");
        let statuses = policy.statuses();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].state, "connected");
        assert_eq!(statuses[0].priority, 10);
    }

    #[test]
    fn disconnects_retry_the_same_profile_without_a_failure_penalty() {
        let mut policy = ConnectPolicy::new(&[profile("home", 10), profile("garden", 5)]);
        policy.begin_attempt(0, 0);
        policy.record_connected(0, 5);
        policy.record_disconnected(100);
        assert_eq!(policy.failures(0), 0);
        assert_eq!(policy.next_target(100), Some(0));
        assert_eq!(policy.state_of(0), Some(ProfileState::Idle));
    }

    #[test]
    fn higher_priority_visible_profile_is_adopted_once_per_minute() {
        let mut policy = ConnectPolicy::new(&[
            profile("best", 30),
            profile("current", 10),
        ]);
        policy.begin_attempt(0, 0);
        policy.record_failure(0, 18);
        policy.begin_attempt(1, 40);
        policy.record_connected(1, 45);
        // Not visible yet, and the 60 s timer is not armed at first.
        assert_eq!(policy.upgrade_target(45, |_| false), None);
        assert_eq!(policy.upgrade_target(100, |ssid| ssid == "best"), None);
        // Backoff from profile 0 has expired by now; a visible "best" wins.
        assert_eq!(policy.upgrade_target(110, |ssid| ssid == "best"), Some(0));
        assert_eq!(policy.upgrade_target(120, |ssid| ssid == "best"), None);
        // A lower priority profile is never adopted.
        let mut policy = ConnectPolicy::new(&[profile("best", 30), profile("current", 10)]);
        policy.begin_attempt(0, 0);
        policy.record_connected(0, 5);
        assert_eq!(policy.upgrade_target(5, |_| true), None);
        assert_eq!(policy.upgrade_target(70, |_| true), None);
    }

    #[test]
    fn failed_profiles_are_never_adopted_as_upgrades() {
        let mut policy = ConnectPolicy::new(&[profile("best", 30), profile("current", 10)]);
        for _ in 0..3 {
            policy.begin_attempt(0, 0);
            policy.record_failure(0, 18);
        }
        policy.begin_attempt(1, 200);
        policy.record_connected(1, 205);
        assert_eq!(policy.state_of(0), Some(ProfileState::Failed));
        assert_eq!(policy.upgrade_target(205, |_| true), None);
        assert_eq!(policy.upgrade_target(500, |_| true), None);
    }
}
