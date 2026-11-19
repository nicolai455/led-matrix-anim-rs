use matrix_firmware::network_config::{NetworkConfig, WifiProfile};
use matrix_firmware::net::{
    backoff_for, ConnectPolicy, FallbackApConfig, ProfileState, WifiConfig, WifiOperatingMode,
    WifiPolicy, FALLBACK_AP_SSID, STA_CONNECT_TIMEOUT_SECS,
};
use matrix_firmware::request_guard::{classify_peer, trusted_write, ControlScope};
use std::net::Ipv4Addr;

#[test]
fn fallback_ap_has_no_universal_default_password() {
    let ap = FallbackApConfig::default();
    assert_eq!(ap.ssid, FALLBACK_AP_SSID);
    assert!(ap.password.is_empty());
    assert!(ap.validate().is_err());
    assert!(NetworkConfig::default()
        .fallback_ap_config()
        .unwrap()
        .is_none());
}

#[test]
fn prioritized_profiles_and_hidden_ssids_are_preserved_for_runtime() {
    let config = NetworkConfig::parse(
        r#"{"wifiProfiles":[{"ssid":"first","password":"fake-sta-alpha","priority":3},{"ssid":"hidden","password":"fake-sta-bravo","priority":9,"hidden":true}],"fallbackApPassword":"fake-ap-charlie","mqttHost":"broker.local","mqttPort":1884,"mqttUser":"mqtt-user","mqttPassword":"fake-mqtt-delta"}"#,
    )
    .unwrap();
    let runtime = WifiConfig::from_network_config(&config).unwrap();

    assert_eq!(runtime.profiles.len(), 2);
    assert_eq!(runtime.profiles[0].ssid, "hidden");
    assert!(runtime.profiles[0].hidden);
    assert_eq!(runtime.profiles[1].ssid, "first");
    assert_eq!(runtime.fallback_ap.as_ref().unwrap().ssid, FALLBACK_AP_SSID);
    assert_eq!(runtime.fallback_ap.as_ref().unwrap().password, "fake-ap-charlie");
    assert_eq!(config.mqtt_host, "broker.local");
    assert_eq!(config.mqtt_port, 1884);
    assert_eq!(config.mqtt_user, "mqtt-user");
    assert_eq!(config.mqtt_password, "fake-mqtt-delta");
}

#[test]
fn wifi_can_be_disabled_without_an_ap_or_sta_profile() {
    let runtime = WifiConfig::from_network_config(&NetworkConfig::default()).unwrap();
    assert!(runtime.profiles.is_empty());
    assert!(runtime.fallback_ap.is_none());
}

#[test]
fn fallback_ap_configuration_validation() {
    let mut config = FallbackApConfig {
        ssid: "test-device".into(),
        password: "fake-ap-pass".into(),
        ..FallbackApConfig::default()
    };
    assert!(config.validate().is_ok());
    config.password = "short".into();
    assert!(config.validate().is_err());
    config.password = "12345678".into();
    assert!(config.validate().is_ok());
    config.ssid = String::new();
    assert!(config.validate().is_err());
}

#[test]
fn fallback_ap_activation_on_timeout() {
    let policy = WifiPolicy::default();
    assert_eq!(STA_CONNECT_TIMEOUT_SECS, 18);
    assert_eq!(policy.evaluate_mode(false, false, 0), WifiOperatingMode::ApOnly);
    assert_eq!(policy.evaluate_mode(true, false, 0), WifiOperatingMode::StaOnly);
    assert_eq!(policy.evaluate_mode(true, false, 10), WifiOperatingMode::StaOnly);
    assert_eq!(policy.evaluate_mode(true, false, 18), WifiOperatingMode::MixedApSta);
    assert_eq!(policy.evaluate_mode(true, false, 30), WifiOperatingMode::MixedApSta);
    assert_eq!(policy.evaluate_mode(true, true, 20), WifiOperatingMode::StaOnly);
}

#[test]
fn prioritized_wifi_profiles_order_stably() {
    let profiles = vec![
        WifiProfile { ssid: "low".into(), password: None, priority: 1, hidden: false },
        WifiProfile { ssid: "high".into(), password: Some("fake-sta-echo".into()), priority: 5, hidden: true },
        WifiProfile { ssid: "also-high".into(), password: None, priority: 5, hidden: false },
    ];
    let input = serde_json::json!({"wifiProfiles": profiles});
    let config = NetworkConfig::parse(&input.to_string()).unwrap();
    let ordered = config.prioritized_wifi_profiles().unwrap();
    assert_eq!(ordered.iter().map(|p| p.ssid.as_str()).collect::<Vec<_>>(), ["high", "also-high", "low"]);
}

/// Same composition the firmware POST /api/network handler uses: classify the
/// peer against the station subnet, then require the CSRF guard plus (for the
/// home network) the setup token. Returns the HTTP status the handler would send.
#[allow(clippy::too_many_arguments)]
fn network_post_status(
    peer: Option<Ipv4Addr>,
    station_subnet: Option<(Ipv4Addr, u8)>,
    origin: Option<&str>,
    host: Option<&str>,
    marker: Option<&str>,
    token: Option<&str>,
    expected_token: &str,
) -> u16 {
    let scope = classify_peer(peer, station_subnet);
    if trusted_write(scope, marker, origin, host, token, expected_token) {
        200
    } else {
        403
    }
}

#[test]
fn home_network_writes_are_allowed_with_the_setup_token() {
    let home = Some((Ipv4Addr::new(192, 168, 178, 35), 24));
    let peer = Some(Ipv4Addr::new(192, 168, 178, 35));
    assert_eq!(
        network_post_status(
            peer,
            home,
            Some("http://192.168.178.35"),
            Some("192.168.178.35"),
            Some("1"),
            Some("ABCDEFGH"),
            "ABCDEFGH"
        ),
        200
    );
}

#[test]
fn home_network_writes_without_a_token_are_forbidden() {
    let home = Some((Ipv4Addr::new(192, 168, 178, 35), 24));
    let peer = Some(Ipv4Addr::new(192, 168, 178, 35));
    for token in [None, Some(""), Some("WRONGTOK")] {
        assert_eq!(
            network_post_status(
                peer,
                home,
                Some("http://192.168.178.35"),
                Some("192.168.178.35"),
                Some("1"),
                token,
                "ABCDEFGH"
            ),
            403,
            "token {token:?} must not unlock the home network"
        );
    }
}

#[test]
fn foreign_networks_are_rejected_even_with_a_valid_token() {
    let home = Some((Ipv4Addr::new(192, 168, 178, 35), 24));
    for peer in [
        Ipv4Addr::new(10, 0, 0, 5),
        Ipv4Addr::new(192, 168, 179, 5),
        Ipv4Addr::new(172, 16, 0, 9),
    ] {
        let origin = format!("http://{peer}");
        assert_eq!(
            network_post_status(
                Some(peer),
                home,
                Some(&origin),
                Some(&peer.to_string()),
                Some("1"),
                Some("ABCDEFGH"),
                "ABCDEFGH"
            ),
            403,
            "{peer} must not reach the configuration endpoint"
        );
    }
}

#[test]
fn cross_site_requests_are_rejected_even_with_a_valid_token() {
    let home = Some((Ipv4Addr::new(192, 168, 178, 35), 24));
    let peer = Some(Ipv4Addr::new(192, 168, 178, 35));
    // Foreign Origin, missing marker, and a lookalike host all stay blocked.
    assert_eq!(
        network_post_status(
            peer,
            home,
            Some("http://evil.test"),
            Some("192.168.178.35"),
            Some("1"),
            Some("ABCDEFGH"),
            "ABCDEFGH"
        ),
        403
    );
    assert_eq!(
        network_post_status(peer, home, None, None, None, Some("ABCDEFGH"), "ABCDEFGH"),
        403
    );
    assert_eq!(
        network_post_status(
            peer,
            home,
            Some("http://192.168.178.35.attacker.test"),
            Some("192.168.178.35"),
            Some("1"),
            Some("ABCDEFGH"),
            "ABCDEFGH"
        ),
        403
    );
}

#[test]
fn fallback_ap_writes_keep_working_without_a_token() {
    let home = Some((Ipv4Addr::new(192, 168, 178, 35), 24));
    assert_eq!(
        network_post_status(
            Some(Ipv4Addr::new(192, 168, 4, 7)),
            home,
            Some("http://192.168.4.1"),
            Some("192.168.4.1"),
            Some("1"),
            None,
            "ABCDEFGH"
        ),
        200
    );
    // ... but the CSRF guard still applies there.
    assert_eq!(
        network_post_status(
            Some(Ipv4Addr::new(192, 168, 4, 7)),
            home,
            Some("http://evil.test"),
            Some("192.168.4.1"),
            Some("1"),
            None,
            "ABCDEFGH"
        ),
        403
    );
}

#[test]
fn peer_classification_matches_the_documented_scopes() {
    let home = Some((Ipv4Addr::new(10, 1, 2, 3), 16));
    assert_eq!(
        classify_peer(Some(Ipv4Addr::new(192, 168, 4, 20)), home),
        ControlScope::FallbackAp
    );
    assert_eq!(
        classify_peer(Some(Ipv4Addr::new(10, 1, 200, 20)), home),
        ControlScope::LocalSubnet
    );
    assert_eq!(
        classify_peer(Some(Ipv4Addr::new(10, 2, 0, 1)), home),
        ControlScope::Untrusted
    );
    assert_eq!(classify_peer(None, home), ControlScope::Untrusted);
}

#[test]
fn backoff_series_and_failed_marking_are_stable() {
    assert_eq!(
        [1, 2, 3, 4, 5, 6].map(backoff_for),
        [18, 36, 72, 144, 288, 300]
    );
    let mut policy = ConnectPolicy::new(&[WifiProfile {
        ssid: "wrong-password".into(),
        password: Some("fake-sta-pass".into()),
        priority: 1,
        hidden: false,
    }]);
    for _ in 0..3 {
        policy.begin_attempt(0, 0);
        policy.record_failure(0, 18);
    }
    assert_eq!(policy.state_of(0), Some(ProfileState::Failed));
    assert!(policy.all_failed());
    assert_eq!(policy.next_target(10_000), None);
}

#[test]
fn all_profiles_failing_leaves_the_fallback_ap_as_the_only_access_path() {
    let mut policy = ConnectPolicy::new(&[
        WifiProfile { ssid: "one".into(), password: None, priority: 2, hidden: false },
        WifiProfile { ssid: "two".into(), password: None, priority: 1, hidden: false },
    ]);
    let mut now = 0u64;
    for _ in 0..6 {
        let Some(index) = policy.next_target(now) else { break };
        policy.begin_attempt(index, now);
        now += 18;
        policy.record_failure(index, now);
        // Skip past any backoff; only the 3-strike rule may stop an attempt.
        now += 300;
    }
    assert!(policy.all_failed());
    assert!(policy.statuses().iter().all(|profile| profile.state == "failed"));
    // The worker keeps the AP configured because the station gave up.
    assert_eq!(policy.next_target(now), None);
}

#[test]
fn a_single_profile_without_a_fallback_ap_stays_station_only() {
    let config = NetworkConfig::parse(r#"{"wifiProfiles":[{"ssid":"only","password":"fake-sta-pass"}]}"#).unwrap();
    let runtime = WifiConfig::from_network_config(&config).unwrap();
    assert_eq!(runtime.profiles.len(), 1);
    assert!(runtime.fallback_ap.is_none());
    let policy = WifiPolicy::new(STA_CONNECT_TIMEOUT_SECS, FallbackApConfig::default());
    // Without AP credentials there is no AP to fall back to; the policy still
    // reports Mixed after the timeout, and the worker maps that to Client-only.
    assert_eq!(policy.evaluate_mode(true, false, 0), WifiOperatingMode::StaOnly);
    assert_eq!(policy.evaluate_mode(true, false, 18), WifiOperatingMode::MixedApSta);
}

#[test]
fn open_networks_are_accepted_without_a_password() {
    let config = NetworkConfig::parse(r#"{"wifiProfiles":[{"ssid":"cafe","password":""}]}"#).unwrap();
    let profiles = config.prioritized_wifi_profiles().unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].password, None, "an empty password means an open network");
    assert!(WifiConfig::from_network_config(&config).unwrap().profiles[0].password.is_none());
}
