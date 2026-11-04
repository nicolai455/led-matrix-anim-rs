//! Target configuration required by ESP-IDF's GDMA-backed I2S driver.
const S3_CONFIG: &str = include_str!("../sdkconfig.defaults.esp32s3");

#[test]
fn s3_i2s_callback_context_uses_internal_memory() {
    assert!(S3_CONFIG.contains("CONFIG_I2S_ISR_IRAM_SAFE=y"));
}
