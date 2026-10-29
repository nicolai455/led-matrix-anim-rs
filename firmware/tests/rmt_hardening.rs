//! Source-level contracts for target-only safety settings. Cross-builds also
//! verify that the ESP-IDF APIs and generated configuration really compile.
const OUTPUT: &str = include_str!("../src/output.rs");
const S3_CONFIG: &str = include_str!("../sdkconfig.defaults.esp32s3");

#[test]
fn refill_interrupt_has_priority_and_cache_safe_support() {
    assert!(OUTPUT.contains("tx_chan_config.intr_priority = 3;"));
    assert!(S3_CONFIG.contains("CONFIG_RMT_ISR_IRAM_SAFE=y"));
}

#[test]
fn encoder_payload_cannot_fall_back_to_psram() {
    assert!(OUTPUT.contains("esp_idf_sys::heap_caps_calloc("));
    assert!(OUTPUT.contains("esp_idf_sys::MALLOC_CAP_INTERNAL | esp_idf_sys::MALLOC_CAP_8BIT"));
    assert!(OUTPUT.contains("esp_idf_sys::heap_caps_free("));
}

#[test]
fn bounded_completion_preserves_borrowed_buffer_safety() {
    assert!(!OUTPUT.contains("rmt_tx_wait_all_done(hw.channels[i], -1)"));
    assert!(OUTPUT.contains("rmt_tx_wait_all_done("));
    assert!(OUTPUT.contains("RMT_COMPLETION_TIMEOUT_MS"));
    assert!(OUTPUT.contains("esp_idf_sys::esp_restart();"));
}

#[test]
fn keep_concurrent_output_and_existing_core_affinity() {
    assert!(OUTPUT.contains("for i in 0..started"));
    assert!(OUTPUT.contains("pin_to_core: Some(esp_idf_hal::cpu::Core::Core1)"));
    assert!(!OUTPUT.contains("TransmissionCache"));
    assert!(!OUTPUT.contains("matrix_fastled"));
}
