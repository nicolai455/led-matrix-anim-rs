use matrix_firmware::ota::OtaManager;

#[test]
fn rejects_empty_and_oversize_images_before_update_begins() {
    let slot = 1_966_080;
    assert!(OtaManager::validate_image_length(0, slot).is_err());
    assert!(OtaManager::validate_image_length(slot + 1, slot).is_err());
    assert!(OtaManager::validate_image_length(slot, slot).is_ok());
    assert!(OtaManager::validate_image_length(1_692_400, 4_194_304).is_ok());
    assert!(OtaManager::validate_header(&[]).is_err());
    assert!(OtaManager::validate_header(b"not an image").is_err());
    assert!(OtaManager::validate_header(&[0xe9]).is_ok());
}
