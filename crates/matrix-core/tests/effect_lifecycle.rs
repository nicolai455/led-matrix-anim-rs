use matrix_core::{AudioSnapshot, Effect, Engine, Frame, RenderContext};
use std::sync::{Arc, Mutex};

struct Probe {
    id: &'static str,
    events: Arc<Mutex<Vec<String>>>,
}
impl Effect for Probe {
    fn id(&self) -> &'static str {
        self.id
    }
    fn name(&self) -> &'static str {
        self.id
    }
    fn render(&mut self, _: &mut Frame, _: &RenderContext<'_>) {}
    fn deactivate(&mut self) {
        self.events.lock().unwrap().push(self.id.into());
    }
}
fn engine() -> (Engine, Arc<Mutex<Vec<String>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let effects = ["a", "b"]
        .into_iter()
        .map(|id| {
            Box::new(Probe {
                id,
                events: events.clone(),
            }) as Box<dyn Effect>
        })
        .collect();
    (Engine::new(effects).unwrap(), events)
}
#[test]
fn manual_switch_releases_previous_effect_only_after_valid_selection() {
    let (mut engine, events) = engine();
    engine
        .apply("light", r#"{"effect":"missing"}"#)
        .unwrap_err();
    assert!(events.lock().unwrap().is_empty());
    engine.apply("light", r#"{"effect":"b"}"#).unwrap();
    assert_eq!(*events.lock().unwrap(), ["a"]);
}
#[test]
fn direct_switch_releases_previous_effect_and_invalid_switch_is_atomic() {
    let (mut engine, events) = engine();
    assert!(!engine.switch_to_effect("missing"));
    assert!(events.lock().unwrap().is_empty());
    assert!(engine.switch_to_effect("b"));
    assert_eq!(*events.lock().unwrap(), ["a"]);
}

#[test]
fn power_off_releases_active_effect() {
    let (mut engine, events) = engine();
    engine.apply("light", r#"{"state":"ON"}"#).unwrap();
    engine.apply("light", r#"{"state":"OFF"}"#).unwrap();
    assert_eq!(*events.lock().unwrap(), ["a"]);
}
#[test]
fn automatic_cycle_releases_previous_effect() {
    let (mut engine, events) = engine();
    engine.set_auto_cycle_interval_ms(10);
    engine.apply("cycle", "ON").unwrap();
    engine.render(1, &AudioSnapshot::default());
    engine.render(11, &AudioSnapshot::default());
    assert_eq!(*events.lock().unwrap(), ["a"]);
}
