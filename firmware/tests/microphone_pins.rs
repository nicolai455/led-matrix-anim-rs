use matrix_firmware::pins::{MicPins, PROFILE_ESP32_BARE, PROFILE_ESP32_S3, PROFILE_ESP32_WROOM};

#[test]
fn microphone_profiles_keep_s3_off_memory_pins() {
    assert_eq!(
        PROFILE_ESP32_S3.mic_pins,
        Some(MicPins {
            sd: 16,
            ws: 15,
            sck: 41
        })
    );
    assert_eq!(PROFILE_ESP32_S3.panel_pins, [8, 3, 10, 9]);
    assert_eq!(
        PROFILE_ESP32_WROOM.mic_pins,
        Some(MicPins {
            sd: 32,
            ws: 15,
            sck: 14
        })
    );
    assert_eq!(PROFILE_ESP32_BARE.mic_pins, None);
}
