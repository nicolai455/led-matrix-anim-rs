use matrix_firmware::network_config::{NetworkConfig, WifiProfile};
use matrix_firmware::net::{
    FallbackApConfig, WifiConfig, WifiOperatingMode, WifiPolicy, FALLBACK_AP_SSID,
    STA_CONNECT_TIMEOUT_SECS,
};

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
