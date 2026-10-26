pub mod device_id;
pub mod mic;
pub mod net;
pub mod network_config;
pub mod nvs_store;
pub mod ota;
pub mod output;
pub mod pins;

pub use device_id::{
    format_device_id_from_mac_bytes, format_device_id_from_mac_u64, get_device_id,
};
pub use mic::I2sMicAudioInput;
pub use net::{FirmwareHttpServer, FirmwareMqtt, MqttConfig, WifiConfig, WifiManager};
pub use nvs_store::FirmwareNvs;
pub use ota::OtaManager;
pub use output::{OutputQuiesce, OutputSettings, RmtLedOutput};
pub use pins::{
    get_default_profile, is_valid_output_pin, BoardProfile, PROFILE_ESP32_BARE, PROFILE_ESP32_S3,
    PROFILE_ESP32_WROOM,
};
