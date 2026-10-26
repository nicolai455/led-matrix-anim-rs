use matrix_core::{AudioSnapshot, Engine};

#[test]
fn configured_cycle_interval_survives_toggle_and_is_not_fixed_to_fifteen_seconds() {
    let mut engine = Engine::new(matrix_effects::builtin_effects()).unwrap();
    engine.set_auto_cycle_interval_ms(1234);
    engine.apply("cycle", "ON").unwrap();
    assert_eq!(engine.auto_cycle_ms(), 1234);
    engine.render(1, &AudioSnapshot::default());
    let initial = engine.snapshot()["effect_id"].clone();
    engine.render(1234, &AudioSnapshot::default());
    assert_eq!(engine.snapshot()["effect_id"], initial);
    engine.render(1235, &AudioSnapshot::default());
    assert_ne!(engine.snapshot()["effect_id"], initial);
    engine.apply("cycle", "OFF").unwrap();
    engine.apply("cycle", "ON").unwrap();
    assert_eq!(engine.auto_cycle_ms(), 1234);
    engine.set_auto_cycle_interval_ms(0);
    assert_eq!(engine.auto_cycle_ms(), 0);
}
