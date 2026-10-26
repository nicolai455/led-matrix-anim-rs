use matrix_core::{Engine, MicSettings};

#[test]
fn microphone_float_deserializer_preserves_schema_and_rejects_overflow() {
    let parsed: MicSettings =
        serde_json::from_str(r#"{"gainDb":12.5,"squelchDb":-72,"beatThreshold":2.1,"agc":false}"#)
            .unwrap();
    assert_eq!(parsed.gain_db, 12.5);
    assert_eq!(parsed.squelch_db, -72.0);
    assert!(!parsed.agc);
    assert_eq!(parsed.attack_ms, MicSettings::default().attack_ms);
    let value = serde_json::to_value(&parsed).unwrap();
    assert_eq!(
        serde_json::from_value::<MicSettings>(value).unwrap(),
        parsed
    );
    assert!(serde_json::from_str::<MicSettings>(r#"{"gainDb":1e100}"#).is_err());
    assert!(serde_json::from_str::<MicSettings>(r#"{"gainDb":"30"}"#).is_err());
    let mut engine = Engine::new(matrix_effects::builtin_effects()).unwrap();
    engine.apply("mic", r#"{"gainDb":12.5}"#).unwrap();
    assert_eq!(engine.snapshot()["mic"]["gainDb"], 12.5);
}
