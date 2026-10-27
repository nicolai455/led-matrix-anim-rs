use matrix_firmware::net::{
    FallbackApConfig, WifiOperatingMode, WifiPolicy, FALLBACK_AP_PASSWORD, FALLBACK_AP_SSID,
    STA_CONNECT_TIMEOUT_SECS,
};

#[test]
fn test_wled_fallback_ap_constants() {
    assert_eq!(FALLBACK_AP_SSID, "JDJLIGHT");
    assert_eq!(FALLBACK_AP_PASSWORD, "JDJLIGHT1337");
    assert_eq!(STA_CONNECT_TIMEOUT_SECS, 18);
}

#[test]
fn test_fallback_ap_configuration_validation() {
    let mut config = FallbackApConfig::default();
    assert_eq!(config.ssid, "JDJLIGHT");
    assert_eq!(config.password, "JDJLIGHT1337");
    assert!(config.validate().is_ok());

    // Password must meet WPA2 length requirements (8..=64)
    config.password = "1234567".into();
    assert!(config.validate().is_err());
    config.password = "12345678".into();
    assert!(config.validate().is_ok());

    // SSID must be non-empty and <= 32 chars
    config.ssid = String::new();
    assert!(config.validate().is_err());
    config.ssid = "JDJLIGHT".into();
    assert!(config.validate().is_ok());
}

#[test]
fn test_wled_fallback_ap_activation_on_timeout() {
    let policy = WifiPolicy::default();

    // 1. Without STA config, starts immediately in AP Mode
    assert_eq!(
        policy.evaluate_mode(false, false, 0),
        WifiOperatingMode::ApOnly
    );

    // 2. With STA config, starts in StaOnly mode
    assert_eq!(
        policy.evaluate_mode(true, false, 0),
        WifiOperatingMode::StaOnly
    );
    assert_eq!(
        policy.evaluate_mode(true, false, 10),
        WifiOperatingMode::StaOnly
    );

    // 3. Exactly at or after timeout (18s), switches to MixedApSta (AP broadcasting JDJLIGHT)
    assert_eq!(
        policy.evaluate_mode(true, false, 18),
        WifiOperatingMode::MixedApSta
    );
    assert_eq!(
        policy.evaluate_mode(true, false, 30),
        WifiOperatingMode::MixedApSta
    );

    // 4. Once connected, drops fallback AP and returns to pure STA mode
    assert_eq!(
        policy.evaluate_mode(true, true, 20),
        WifiOperatingMode::StaOnly
    );
}
