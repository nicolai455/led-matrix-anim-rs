// Device identification matching legacy snprintf("matrix_%012llx", (unsigned long long)ESP.getEfuseMac())

/// Formats the little-endian eFuse MAC as a 12-character lowercase hex string prefixed with "matrix_".
///
/// On ESP32 / ESP32-S3, `esp_efuse_mac_get_default` fills 6 bytes in memory.
/// When read as a little-endian `u64`:
///   `val = b0 | (b1 << 8) | (b2 << 16) | (b3 << 24) | (b4 << 32) | (b5 << 40)`
/// And formatted with `%012llx`, the bytes appear in reverse (b5 is most significant, b0 is least).
/// This function reproduces that exact representation so Home Assistant MQTT entity IDs
/// remain identical and do not create duplicates.
pub fn format_device_id_from_mac_u64(mac_u64: u64) -> String {
    format!("matrix_{:012x}", mac_u64)
}

pub fn format_device_id_from_mac_bytes(mac_bytes: &[u8; 6]) -> String {
    let mut mac_u64 = 0u64;
    for (i, &byte) in mac_bytes.iter().enumerate() {
        mac_u64 |= (byte as u64) << (i * 8);
    }
    format_device_id_from_mac_u64(mac_u64)
}

#[cfg(target_os = "espidf")]
pub fn get_device_id() -> String {
    let mut mac_bytes = [0u8; 6];
    unsafe {
        esp_idf_sys::esp_efuse_mac_get_default(mac_bytes.as_mut_ptr());
    }
    format_device_id_from_mac_bytes(&mac_bytes)
}

#[cfg(not(target_os = "espidf"))]
pub fn get_device_id() -> String {
    "matrix_hostsim00000".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_id_formatting() {
        // Suppose MAC is 24:0a:c4:00:11:22
        // In memory: [0x24, 0x0a, 0xc4, 0x00, 0x11, 0x22]
        // u64 value = 0x0000221100c40a24
        // formatted as %012llx -> "221100c40a24"
        let mac = [0x24, 0x0a, 0xc4, 0x00, 0x11, 0x22];
        let id = format_device_id_from_mac_bytes(&mac);
        assert_eq!(id, "matrix_221100c40a24");
    }
}
