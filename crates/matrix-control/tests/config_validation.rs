use matrix_control::config::ConfigStore;
use serde_json::json;

#[test]
fn rejects_malformed_configuration_atomically_without_integer_truncation() {
    let store = ConfigStore::new(None);
    let before = store.to_json();
    for change in [
        json!({"panelOrder":[0,0,2,3]}),
        json!({"matrixWidth":3}),
        json!({"dataPin":256}),
        json!({"ledMaxMilliamps":4294979296u64}),
        json!({"mirrorX":"false"}),
        json!({"ledBrightness":-1}),
        json!({"unknown":true}),
    ] {
        assert!(
            store.update_from_json(&change).is_err(),
            "accepted {change}"
        );
        assert_eq!(store.to_json(), before);
    }
    assert!(
        store
            .update_from_json(&json!({"mirrorX":true}))
            .unwrap()
            .mirror_x
    );
}
