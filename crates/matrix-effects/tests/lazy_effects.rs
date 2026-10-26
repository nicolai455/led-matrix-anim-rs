use matrix_core::{
    AudioSnapshot, Effect, Frame, FrameSource, ParameterDescriptor, ParameterKind, RenderContext,
    Rgb,
};
use matrix_effects::{builtin_effects, frame_animation, FramesEffect, LazyEffect};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

// Custom global allocator to track heap usage
struct TrackingAllocator;

static CURRENT_ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static PEAK_ALLOCATED: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            let prev = CURRENT_ALLOCATED.fetch_add(layout.size(), Ordering::SeqCst);
            let new_val = prev + layout.size();
            let mut current_peak = PEAK_ALLOCATED.load(Ordering::SeqCst);
            while new_val > current_peak {
                match PEAK_ALLOCATED.compare_exchange_weak(
                    current_peak,
                    new_val,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => break,
                    Err(actual) => current_peak = actual,
                }
            }
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        CURRENT_ALLOCATED.fetch_sub(layout.size(), Ordering::SeqCst);
    }
}

#[global_allocator]
static A: TrackingAllocator = TrackingAllocator;

static MOCK_CONSTRUCT_COUNT: AtomicUsize = AtomicUsize::new(0);
static MOCK_DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

const MOCK_PARAMS: [ParameterDescriptor; 1] = [ParameterDescriptor {
    id: "speed",
    label: "Speed",
    kind: ParameterKind::Number,
    min: 1.0,
    max: 100.0,
    step: 1.0,
    default: 10.0,
    options: &[],
    unit: "",
}];

struct MockEffect {
    speed: f32,
    reset_called: bool,
}

impl MockEffect {
    fn new() -> Self {
        MOCK_CONSTRUCT_COUNT.fetch_add(1, Ordering::SeqCst);
        Self {
            speed: 10.0,
            reset_called: false,
        }
    }
}

impl Drop for MockEffect {
    fn drop(&mut self) {
        MOCK_DROP_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

impl Effect for MockEffect {
    fn id(&self) -> &'static str {
        "mock"
    }

    fn name(&self) -> &'static str {
        "Mock"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &MOCK_PARAMS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        if id == "speed" {
            Some(self.speed)
        } else {
            None
        }
    }

    fn set_parameter(&mut self, id: &str, value: f32) -> Result<(), String> {
        let desc = self
            .parameters()
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("effect '{}' has no parameter '{}'", self.id(), id))?;
        desc.validate_value(value)?;
        if id == "speed" {
            self.speed = value;
            Ok(())
        } else {
            Err(format!("effect '{}' has no parameter '{}'", self.id(), id))
        }
    }

    fn reset(&mut self) {
        self.reset_called = true;
    }

    fn render(&mut self, frame: &mut Frame, _context: &RenderContext<'_>) {
        frame.clear(Rgb::new(self.speed as u8, 0, 0));
    }
}

fn mock_factory() -> Box<dyn Effect> {
    Box::new(MockEffect::new())
}

#[test]
fn test_lazy_effect_constructor_drop_counters_and_activation_lifecycle() {
    MOCK_CONSTRUCT_COUNT.store(0, Ordering::SeqCst);
    MOCK_DROP_COUNT.store(0, Ordering::SeqCst);

    // 1. Creation: factory called once for metadata extraction, then immediately dropped!
    let mut lazy = LazyEffect::new(mock_factory);
    assert_eq!(
        MOCK_CONSTRUCT_COUNT.load(Ordering::SeqCst),
        1,
        "Constructor should have been called once for metadata"
    );
    assert_eq!(
        MOCK_DROP_COUNT.load(Ordering::SeqCst),
        1,
        "Metadata temporary instance should have been dropped immediately"
    );
    assert!(
        !lazy.is_active(),
        "LazyEffect inner must be inactive initially"
    );

    // 2. Metadata queries do NOT activate
    assert_eq!(lazy.id(), "mock");
    assert_eq!(lazy.name(), "Mock");
    assert_eq!(lazy.parameters().len(), 1);
    assert_eq!(lazy.parameters()[0].id, "speed");
    assert_eq!(MOCK_CONSTRUCT_COUNT.load(Ordering::SeqCst), 1);
    assert_eq!(MOCK_DROP_COUNT.load(Ordering::SeqCst), 1);
    assert!(!lazy.is_active());

    // 3. Inactive parameter read does NOT activate
    assert_eq!(lazy.parameter("speed"), Some(10.0));
    assert_eq!(lazy.parameter("non_existent"), None);
    assert_eq!(MOCK_CONSTRUCT_COUNT.load(Ordering::SeqCst), 1);
    assert_eq!(MOCK_DROP_COUNT.load(Ordering::SeqCst), 1);
    assert!(!lazy.is_active());

    // 4. Inactive parameter write does NOT activate, but validates and preserves settings
    assert!(lazy.set_parameter("speed", 45.0).is_ok());
    assert_eq!(lazy.parameter("speed"), Some(45.0));
    // Invalid value should fail validation without activating
    assert!(lazy.set_parameter("speed", 500.0).is_err());
    assert!(lazy.set_parameter("unknown", 1.0).is_err());
    assert_eq!(MOCK_CONSTRUCT_COUNT.load(Ordering::SeqCst), 1);
    assert_eq!(MOCK_DROP_COUNT.load(Ordering::SeqCst), 1);
    assert!(!lazy.is_active());

    // 5. Inactive reset does NOT activate
    lazy.reset();
    assert_eq!(MOCK_CONSTRUCT_COUNT.load(Ordering::SeqCst), 1);
    assert_eq!(MOCK_DROP_COUNT.load(Ordering::SeqCst), 1);
    assert!(!lazy.is_active());

    // 6. First render activates runtime inner on demand with preserved settings
    let mut frame = Frame::default();
    let audio = AudioSnapshot::default();
    let ctx = RenderContext {
        now_ms: 0,
        delta_seconds: 0.016,
        audio: &audio,
        color: Rgb::WHITE,
        text: "",
        scroll_speed: 1.0,
    };

    lazy.render(&mut frame, &ctx);
    assert_eq!(
        MOCK_CONSTRUCT_COUNT.load(Ordering::SeqCst),
        2,
        "Constructor should have been called on render"
    );
    assert_eq!(
        MOCK_DROP_COUNT.load(Ordering::SeqCst),
        1,
        "Inner instance should remain alive during active rendering"
    );
    assert!(
        lazy.is_active(),
        "LazyEffect inner must be active after render"
    );
    // Verify inner received preserved setting (45.0)
    assert_eq!(frame.get(0, 0), Rgb::new(45, 0, 0));
    assert_eq!(lazy.parameter("speed"), Some(45.0));

    // 7. Subsequent render does NOT construct a new instance
    lazy.render(&mut frame, &ctx);
    assert_eq!(MOCK_CONSTRUCT_COUNT.load(Ordering::SeqCst), 2);
    assert_eq!(MOCK_DROP_COUNT.load(Ordering::SeqCst), 1);

    // 8. Active parameter write updates active inner and preserves setting
    assert!(lazy.set_parameter("speed", 75.0).is_ok());
    assert_eq!(lazy.parameter("speed"), Some(75.0));
    lazy.render(&mut frame, &ctx);
    assert_eq!(frame.get(0, 0), Rgb::new(75, 0, 0));

    // 9. Deactivate drops runtime inner, preserving settings
    lazy.deactivate();
    assert_eq!(
        MOCK_DROP_COUNT.load(Ordering::SeqCst),
        2,
        "Inner instance must be dropped on deactivate"
    );
    assert_eq!(
        MOCK_CONSTRUCT_COUNT.load(Ordering::SeqCst),
        2,
        "Deactivate should not construct a new instance"
    );
    assert!(
        !lazy.is_active(),
        "LazyEffect inner must be inactive after deactivate"
    );
    assert_eq!(
        lazy.parameter("speed"),
        Some(75.0),
        "Parameter must be preserved after deactivation"
    );

    // 10. Inactive parameter write while deactivated
    assert!(lazy.set_parameter("speed", 30.0).is_ok());
    assert_eq!(lazy.parameter("speed"), Some(30.0));
    assert_eq!(MOCK_CONSTRUCT_COUNT.load(Ordering::SeqCst), 2);
    assert_eq!(MOCK_DROP_COUNT.load(Ordering::SeqCst), 2);

    // 11. Second render reactivates with new inner receiving preserved parameter
    lazy.render(&mut frame, &ctx);
    assert_eq!(
        MOCK_CONSTRUCT_COUNT.load(Ordering::SeqCst),
        3,
        "Inner should be reallocated on next render"
    );
    assert_eq!(MOCK_DROP_COUNT.load(Ordering::SeqCst), 2);
    assert!(lazy.is_active());
    assert_eq!(frame.get(0, 0), Rgb::new(30, 0, 0));

    // 12. Final drop releases inner
    drop(lazy);
    assert_eq!(
        MOCK_DROP_COUNT.load(Ordering::SeqCst),
        3,
        "Final drop should drop inner instance"
    );
}

#[test]
fn test_render_on_demand_and_drop_before_new_allocation_invariant() {
    let mut effects = builtin_effects();
    assert_eq!(effects.len(), 21);

    let mut frame = Frame::default();
    let audio = AudioSnapshot::default();
    let ctx = RenderContext {
        now_ms: 100,
        delta_seconds: 0.016,
        audio: &audio,
        color: Rgb::WHITE,
        text: "test",
        scroll_speed: 1.0,
    };

    // Simulate engine lifecycle:
    // When switching effects, engine deactivates previous effect before rendering the new one ("drop-before-new").
    let mut active_idx: Option<usize> = None;

    for switch_target in [0, 5, 10, 15, 20, 10, 0] {
        // Drop-before-new invariant: deactivate previous effect first
        if let Some(prev) = active_idx {
            effects[prev].deactivate();
        }

        // Render target effect on demand
        effects[switch_target].render(&mut frame, &ctx);
        active_idx = Some(switch_target);
    }

    // Clean up
    if let Some(prev) = active_idx {
        effects[prev].deactivate();
    }
}

#[test]
fn test_builtin_effects_memory_savings_and_allocation_bytes() {
    // A separate single-test process excludes allocations from sibling tests.
    if std::env::var_os("MATRIX_MEMORY_TEST_CHILD").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "test_builtin_effects_memory_savings_and_allocation_bytes",
                "--nocapture",
            ])
            .env("MATRIX_MEMORY_TEST_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        print!("{}", String::from_utf8_lossy(&output.stdout));
        return;
    }
    let baseline = CURRENT_ALLOCATED.load(Ordering::SeqCst);

    // 1. Create all 21 builtin effects
    let mut effects = builtin_effects();
    let loaded_all_wrappers = CURRENT_ALLOCATED.load(Ordering::SeqCst);
    let wrappers_overhead = loaded_all_wrappers.saturating_sub(baseline);

    // 2. Render all 21 effects without deactivating (worst case retained RAM if eagerly loaded)
    let mut frame = Frame::default();
    let audio = AudioSnapshot::default();
    let ctx = RenderContext {
        now_ms: 100,
        delta_seconds: 0.016,
        audio: &audio,
        color: Rgb::WHITE,
        text: "test",
        scroll_speed: 1.0,
    };

    for effect in &mut effects {
        effect.render(&mut frame, &ctx);
    }

    let all_active_allocated = CURRENT_ALLOCATED.load(Ordering::SeqCst);
    let all_active_heap = all_active_allocated.saturating_sub(loaded_all_wrappers);
    assert!(
        all_active_heap > 0,
        "Active effects should have allocated runtime instances"
    );

    // 3. Deactivate all effects
    for effect in &mut effects {
        effect.deactivate();
    }

    let after_deactivate = CURRENT_ALLOCATED.load(Ordering::SeqCst);
    let memory_released = all_active_allocated.saturating_sub(after_deactivate);
    println!("Wrapper metadata heap: {wrappers_overhead} bytes; all runtime heap: {all_active_heap} bytes; released: {memory_released} bytes");

    assert!(
        memory_released >= all_active_heap,
        "Deactivation must release all runtime effect allocations! (released: {}, expected at least: {})",
        memory_released,
        all_active_heap
    );
}

#[test]
fn test_frames_effect_buffer_release_on_deactivate() {
    struct TestSource {
        count: usize,
    }
    impl FrameSource for TestSource {
        fn frame_count(&self) -> usize {
            self.count
        }
        fn read_frame_into(&mut self, index: usize, frame: &mut Frame) -> Result<(), String> {
            frame.clear(Rgb::new((index * 10) as u8, 0, 0));
            Ok(())
        }
    }

    let source = Box::new(TestSource { count: 5 });
    let mut effect = FramesEffect::from_source(source, 50);

    // Working buffer is released initially
    assert!(
        !effect.has_frame_buffer(),
        "FramesEffect must not allocate frame buffer initially"
    );

    let mut frame = Frame::default();
    let audio = AudioSnapshot::default();
    let ctx = RenderContext {
        now_ms: 0,
        delta_seconds: 0.0,
        audio: &audio,
        color: Rgb::BLACK,
        text: "",
        scroll_speed: 0.0,
    };

    // First render allocates frame working buffer
    effect.render(&mut frame, &ctx);
    assert!(
        effect.has_frame_buffer(),
        "FramesEffect must allocate frame buffer on render"
    );
    assert_eq!(frame.get(0, 0), Rgb::new(0, 0, 0));

    // Next frame
    let ctx2 = RenderContext {
        now_ms: 60,
        delta_seconds: 0.06,
        audio: &audio,
        color: Rgb::BLACK,
        text: "",
        scroll_speed: 0.0,
    };
    effect.render(&mut frame, &ctx2);
    assert_eq!(frame.get(0, 0), Rgb::new(10, 0, 0));

    // Deactivate releases frame working buffer while preserving source
    effect.deactivate();
    assert!(
        !effect.has_frame_buffer(),
        "FramesEffect must release frame buffer on deactivate"
    );

    // Subsequent render re-allocates buffer and continues playback
    let ctx3 = RenderContext {
        now_ms: 120,
        delta_seconds: 0.06,
        audio: &audio,
        color: Rgb::BLACK,
        text: "",
        scroll_speed: 0.0,
    };
    effect.render(&mut frame, &ctx3);
    assert!(
        effect.has_frame_buffer(),
        "FramesEffect must reallocate frame buffer on render after deactivation"
    );

    // Also test frame_animation helper
    let source2 = Box::new(TestSource { count: 3 });
    let mut boxed_anim = frame_animation(source2, 50);
    boxed_anim.render(&mut frame, &ctx);
    boxed_anim.deactivate();
}
