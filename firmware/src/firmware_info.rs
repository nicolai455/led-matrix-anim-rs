//! Identity and OTA state of the running firmware image.
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareInfo {
    pub version: String,
    pub elf_sha256: String,
    pub partition: String,
    pub partition_address: u32,
    pub ota_state: Option<String>,
}

pub fn firmware_info_json(info: &FirmwareInfo) -> serde_json::Value {
    serde_json::to_value(info).expect("firmware info is serializable")
}

#[cfg(target_os = "espidf")]
pub fn running() -> Result<FirmwareInfo, String> {
    use esp_idf_sys as idf;
    use std::ffi::c_char;

    fn fixed_string(bytes: &[c_char]) -> String {
        let bytes: Vec<u8> = bytes.iter().take_while(|&&b| b != 0).map(|&b| b as u8).collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    let description = unsafe { idf::esp_app_get_description().as_ref() }
        .ok_or("application description unavailable")?;
    let partition = unsafe { idf::esp_ota_get_running_partition().as_ref() }
        .ok_or("running partition unavailable")?;
    let mut state = idf::esp_ota_img_states_t_ESP_OTA_IMG_UNDEFINED;
    let rc = unsafe { idf::esp_ota_get_state_partition(partition, &mut state) };
    let ota_state = if rc == idf::ESP_ERR_NOT_FOUND {
        None
    } else if rc != idf::ESP_OK {
        return Err(format!("cannot query OTA state: {rc}"));
    } else {
        Some(match state {
            idf::esp_ota_img_states_t_ESP_OTA_IMG_NEW => "new",
            idf::esp_ota_img_states_t_ESP_OTA_IMG_PENDING_VERIFY => "pending_verify",
            idf::esp_ota_img_states_t_ESP_OTA_IMG_VALID => "valid",
            idf::esp_ota_img_states_t_ESP_OTA_IMG_INVALID => "invalid",
            idf::esp_ota_img_states_t_ESP_OTA_IMG_ABORTED => "aborted",
            _ => "undefined",
        }.to_owned())
    };
    Ok(FirmwareInfo {
        version: fixed_string(&description.version),
        elf_sha256: description.app_elf_sha256.iter().map(|b| format!("{b:02x}")).collect(),
        partition: fixed_string(&partition.label),
        partition_address: partition.address,
        ota_state,
    })
}
