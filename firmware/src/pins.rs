//! Static board pin mapping and hardware profile definitions.
//!
//! Traced from `include/BoardPins.h`.

pub const NUM_OUTPUTS: usize = 4;
pub const TOTAL_LEDS: usize = 1024;
pub const PANEL_LEDS: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardProfile {
    pub name: &'static str,
    pub panel_pins: [u8; NUM_OUTPUTS],
    pub data_pin: u8,
    pub button_pin: Option<u8>,
    pub mic_pins: Option<MicPins>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MicPins {
    pub sd: u8,
    pub ws: u8,
    pub sck: u8,
}

/// ESP32-WROOM with standard I2S mic
pub const PROFILE_ESP32_WROOM: BoardProfile = BoardProfile {
    name: "ESP32-WROOM",
    panel_pins: [4, 16, 17, 18],
    data_pin: 4,
    button_pin: Some(0),
    mic_pins: Some(MicPins {
        sd: 32,
        ws: 15,
        sck: 14,
    }),
};

/// ESP32-WROOM bare profile (no microphone hardware attached)
pub const PROFILE_ESP32_BARE: BoardProfile = BoardProfile {
    name: "ESP32-WROOM Bare",
    panel_pins: [4, 16, 17, 18],
    data_pin: 4,
    button_pin: Some(0),
    mic_pins: None,
};

/// ESP32-S3 N16R8 profile
///
/// NOTE ON GPIO 46:
/// In the legacy C++ `BoardPins.h`, S3 parallel pins were listed as `{8, 3, 46, 9}`.
/// However, on ESP32-S3, GPIO 46 is strictly an INPUT-ONLY strapping pin (ROM boot control).
/// It has no output driver in silicon (SOC_GPIO_VALID_OUTPUT_GPIO_MASK excludes it from output),
/// and `esp-idf-hal` only implements `InputPin` for `Gpio46`.
/// To make parallel output work genuinely across 4 panels on S3 without silent failure,
/// Panel 2 data is assigned to GPIO 10 (valid high-speed output pin not used by Octal PSRAM/Flash).
pub const PROFILE_ESP32_S3: BoardProfile = BoardProfile {
    name: "ESP32-S3 N16R8",
    panel_pins: [8, 3, 10, 9], // GPIO 10 replaces invalid input-only GPIO 46
    data_pin: 8,
    button_pin: Some(0),
    mic_pins: None, // Can be configured when microphone is wired
};

/// Validates whether a given GPIO number is a valid output pin on the target architecture.
pub fn is_valid_output_pin(pin: u8, is_s3: bool) -> bool {
    if is_s3 {
        // ESP32-S3: 0..48 except 22..25, and NOT 46 (which is input only)
        // Flash/OPI PSRAM pins (26..37) must also be avoided for LED data output
        if pin == 46 {
            return false;
        }
        if (22..=25).contains(&pin) {
            return false;
        }
        if (26..=37).contains(&pin) {
            return false;
        }
        pin <= 48
    } else {
        // ESP32 GPIO20/24/28..31 do not exist; 6..11 are flash,
        // 34..39 are input only on the original WROOM target.
        matches!(pin, 0..=5 | 12..=19 | 21..=23 | 25..=27 | 32..=33)
    }
}

pub fn get_default_profile() -> BoardProfile {
    #[cfg(any(feature = "esp32s3", esp32s3))]
    return PROFILE_ESP32_S3;

    #[cfg(feature = "bare")]
    return PROFILE_ESP32_BARE;

    #[cfg(not(any(feature = "esp32s3", esp32s3, feature = "bare")))]
    return PROFILE_ESP32_WROOM;
}
