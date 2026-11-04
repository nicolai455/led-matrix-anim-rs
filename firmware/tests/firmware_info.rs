use matrix_firmware::firmware_info::{firmware_info_json, FirmwareInfo};

#[test]
fn firmware_info_identifies_running_image_and_ota_state() {
    let info = FirmwareInfo {
        version: "0.1.0".into(),
        elf_sha256: "ab".repeat(32),
        partition: "ota_1".into(),
        partition_address: 0x420000,
        ota_state: Some("valid".into()),
    };
    let json = firmware_info_json(&info);
    assert_eq!(json["version"], "0.1.0");
    assert_eq!(json["elfSha256"], "ab".repeat(32));
    assert_eq!(json["partition"], "ota_1");
    assert_eq!(json["partitionAddress"], 0x420000);
    assert_eq!(json["otaState"], "valid");
}

#[test]
fn missing_ota_state_is_not_reported_as_valid() {
    let info = FirmwareInfo {
        version: "0.1.0".into(),
        elf_sha256: "00".repeat(32),
        partition: "factory".into(),
        partition_address: 0x10000,
        ota_state: None,
    };
    assert!(firmware_info_json(&info)["otaState"].is_null());
}
