use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
use matrix_core::{Engine, Frame};
use matrix_control::{ControlHub, LedOutput};
use matrix_control::persistence::StateStore;

struct Meter;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 { let p=unsafe {System.alloc(l)}; if !p.is_null(){let n=LIVE.fetch_add(l.size(),Ordering::SeqCst)+l.size(); PEAK.fetch_max(n,Ordering::SeqCst);} p }
    unsafe fn dealloc(&self,p:*mut u8,l:Layout){LIVE.fetch_sub(l.size(),Ordering::SeqCst);unsafe {System.dealloc(p,l)}}
}
#[global_allocator] static ALLOC: Meter = Meter;
struct Sink;
impl LedOutput for Sink { fn show(&mut self,_:&Frame,_:u8)->Result<(),String>{ std::thread::sleep(Duration::from_millis(5)); Ok(()) } }
#[test]
fn actor_and_storage_peak_are_bounded_with_full_catalog() {
    let _guard = TEST_LOCK.lock().unwrap();
    let engine=Engine::new(matrix_effects::builtin_effects()).unwrap();
    assert_eq!(engine.effects().len(),82);
    let base=LIVE.load(Ordering::SeqCst); PEAK.store(base,Ordering::SeqCst);
    let hub=ControlHub::spawn(engine,Sink,None).unwrap();
    let store=StateStore::new(None);
    for _ in 0..4 {
        std::thread::sleep(Duration::from_millis(110));
        let state=hub.snapshot_json();
        store.save_json(&state).unwrap();
        matrix_control::discovery::discovery_json_for_each("test",&hub.catalog_json(), |_| Ok::<(),()>(())).unwrap();
    }
    let peak=PEAK.load(Ordering::SeqCst)-base;
    eprintln!("peak additional bytes={peak}");
    assert!(peak < 220_000,"excessive actor/storage/discovery heap peak: {peak}");
}

#[test]
fn streamed_discovery_matches_tree_and_restore_keeps_parameters() {
    let _guard = TEST_LOCK.lock().unwrap();
    let mut engine=Engine::new(matrix_effects::builtin_effects()).unwrap();
    engine.apply("effect/fire/audio_reactive", "OFF").unwrap();
    engine.apply("effect/freq_bars/palette", "Ocean").unwrap();
    let original=engine.snapshot();
    assert_eq!(original["effects"]["fire"]["audio_reactive"], "OFF");
    assert_eq!(original["effects"]["freq_bars"]["palette"], "Ocean");
    assert!(original["effects"].as_object().unwrap().values().any(|e| e.as_object().unwrap().values().any(|v| v.is_number())));
    assert_eq!(engine.catalog().as_array().unwrap().len(), 82);
    let mixed = r#"{"params":{"fire":{"audio_reactive":"OFF"}},"effects":{"fire":{"audio_reactive":"ON"}}}"#;
    let normalized = matrix_control::persistence::normalize_json(mixed).unwrap();
    let mut mixed_engine=Engine::new(matrix_effects::builtin_effects()).unwrap();
    matrix_core::restore_runtime_state(&mut mixed_engine, &normalized).unwrap();
    assert_eq!(mixed_engine.snapshot()["effects"]["fire"]["audio_reactive"], "OFF");
    let catalog=engine.catalog_json().unwrap();
    let expected=matrix_control::discovery::discovery("test", &serde_json::from_slice(&catalog).unwrap());
    let mut streamed=Vec::new();
    matrix_control::discovery::discovery_json_for_each("test",&catalog,|v| {streamed.push(v); Ok::<(),()>(())}).unwrap();
    assert_eq!(streamed, expected);
    let store=StateStore::new(None);
    store.save_json(&engine.snapshot_json(&serde_json::Map::new()).unwrap()).unwrap();
    let mut restored=Engine::new(matrix_effects::builtin_effects()).unwrap();
    store.restore(&mut restored).unwrap();
    assert_eq!(restored.snapshot()["effects"], original["effects"]);
    assert_eq!(restored.snapshot()["state"], "OFF");
}
