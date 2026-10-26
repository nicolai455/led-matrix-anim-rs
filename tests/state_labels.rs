use matrix_core::{restore_runtime_state, Engine};

#[test]
fn persistent_selects_and_booleans_restore_from_canonical_labels() {
    let mut engine = Engine::new(matrix_effects::builtin_effects()).unwrap();
    restore_runtime_state(&mut engine,r#"{"params":{"fire":{"audio_reactive":"OFF"},"freq_bars":{"palette":"Ocean","peak_hold":"OFF"}}}"#).unwrap();
    let snapshot = engine.snapshot();
    assert_eq!(snapshot["effects"]["fire"]["audio_reactive"], "OFF");
    assert_eq!(snapshot["effects"]["freq_bars"]["palette"], "Ocean");
    assert_eq!(snapshot["effects"]["freq_bars"]["peak_hold"], "OFF");
    assert_eq!(snapshot["state"], "OFF");
}
