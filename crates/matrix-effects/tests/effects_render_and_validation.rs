use matrix_core::{
    AudioSnapshot, Effect, Frame, FrameSource, LfxStream, LfxWriter, RenderContext, Rgb, PIXELS,
};
use matrix_effects::builtin_effects;

#[test]
fn test_all_effects_parameter_validation_and_bounds() {
    let mut effects = builtin_effects();

    for effect in &mut effects {
        let eid = effect.id();
        let ename = effect.name();
        println!(
            "EFFECT {:20} {:20} size = {}",
            eid,
            ename,
            std::mem::size_of_val(&**effect)
        );
        assert!(!eid.is_empty(), "Effect ID must not be empty");
        assert!(!ename.is_empty(), "Effect name must not be empty");

        let params = effect.parameters().to_vec();

        if params.is_empty() {
            // For effects without parameters, setting any parameter should return Err
            assert!(
                effect.set_parameter("dummy", 0.0).is_err(),
                "Effect '{}' with no params accepted parameter set",
                eid
            );
            assert_eq!(effect.parameter("dummy"), None);
        } else {
            for param in &params {
                let pid = param.id;

                // Should match default value initially
                let initial = effect
                    .parameter(pid)
                    .unwrap_or_else(|| panic!("Effect '{}' missing parameter '{}'", eid, pid));
                assert!(
                    (initial - param.default).abs() < 1e-4,
                    "Effect '{}' param '{}' initial value mismatch: got {}, expected {}",
                    eid,
                    pid,
                    initial,
                    param.default
                );

                // Set min value
                assert!(
                    effect.set_parameter(pid, param.min).is_ok(),
                    "Effect '{}' param '{}' rejected min value {}",
                    eid,
                    pid,
                    param.min
                );
                assert!(
                    (effect.parameter(pid).unwrap() - param.min).abs() < 1e-4,
                    "Effect '{}' param '{}' value not updated to min",
                    eid,
                    pid
                );

                // Set max value
                assert!(
                    effect.set_parameter(pid, param.max).is_ok(),
                    "Effect '{}' param '{}' rejected max value {}",
                    eid,
                    pid,
                    param.max
                );
                assert!(
                    (effect.parameter(pid).unwrap() - param.max).abs() < 1e-4,
                    "Effect '{}' param '{}' value not updated to max",
                    eid,
                    pid
                );

                // Out of range below min
                assert!(
                    effect.set_parameter(pid, param.min - 1.0).is_err(),
                    "Effect '{}' param '{}' accepted value below min: {}",
                    eid,
                    pid,
                    param.min - 1.0
                );

                // Out of range above max
                assert!(
                    effect.set_parameter(pid, param.max + 1.0).is_err(),
                    "Effect '{}' param '{}' accepted value above max: {}",
                    eid,
                    pid,
                    param.max + 1.0
                );

                // Non-finite value should fail
                assert!(effect.set_parameter(pid, f32::NAN).is_err());
                assert!(effect.set_parameter(pid, f32::INFINITY).is_err());

                // Reset back to default
                assert!(effect.set_parameter(pid, param.default).is_ok());
            }

            // Setting unknown parameter should fail
            assert!(effect.set_parameter("non_existent_param", 10.0).is_err());
            assert_eq!(effect.parameter("non_existent_param"), None);
        }
    }
}

#[test]
fn test_all_effects_render_at_varying_dt_and_audio() {
    let mut effects = builtin_effects();

    let audio_silent = AudioSnapshot::default();

    let audio_active = AudioSnapshot {
        available: true,
        level: 0.8,
        peak: 0.95,
        beat: true,
        bands: [0.75; 16],
        waveform: [0.5; 128],
    };

    let dts = [0.0f32, 0.001, 0.016, 0.033, 0.05, 0.1, 0.5, 1.0];

    for effect in &mut effects {
        let eid = effect.id();
        effect.reset();

        let mut frame = Frame::default();
        let mut now_ms = 1000u64;

        for &dt in &dts {
            now_ms += (dt * 1000.0) as u64;

            // Render without audio
            let ctx_silent = RenderContext {
                now_ms,
                delta_seconds: dt,
                audio: &audio_silent,
                color: Rgb::new(200, 100, 50),
                text: "TEST",
                scroll_speed: 25.0,
            };
            effect.render(&mut frame, &ctx_silent);
            assert_eq!(frame.pixels.len(), PIXELS, "Effect '{}' frame len", eid);

            // Render with active audio
            now_ms += 16;
            let ctx_active = RenderContext {
                now_ms,
                delta_seconds: 0.016,
                audio: &audio_active,
                color: Rgb::WHITE,
                text: "HELLO WORLD",
                scroll_speed: 40.0,
            };
            effect.render(&mut frame, &ctx_active);
            assert_eq!(
                frame.pixels.len(),
                PIXELS,
                "Effect '{}' active audio frame len",
                eid
            );
        }

        // Test reset method
        effect.reset();
    }
}

#[test]
fn test_frames_effect_streaming_and_error_handling() {
    let mut buffer = Vec::new();
    let frame_count = 3u16;
    let mut writer = LfxWriter::new(&mut buffer, frame_count).expect("create LfxWriter");

    // Write 3 distinctive frames
    for i in 0..frame_count {
        let mut f = Frame::default();
        let color = match i {
            0 => Rgb::new(255, 0, 0),
            1 => Rgb::new(0, 255, 0),
            _ => Rgb::new(0, 0, 255),
        };
        f.clear(color);
        writer.write_frame(&f).expect("write frame");
    }

    let cursor = std::io::Cursor::new(buffer);
    let source = Box::new(LfxStream::new(cursor).expect("parse lfx"));
    let mut frames_effect = matrix_effects::FramesEffect::from_source(source, 50);

    let mut output = Frame::default();
    let audio = AudioSnapshot::default();

    // Initial render at now_ms = 0
    let ctx0 = RenderContext {
        now_ms: 0,
        delta_seconds: 0.0,
        audio: &audio,
        color: Rgb::WHITE,
        text: "",
        scroll_speed: 0.0,
    };
    frames_effect.render(&mut output, &ctx0);
    assert_eq!(output.get(0, 0), Rgb::new(255, 0, 0));

    // Render after 20ms (not yet 50ms delay) -> should stay frame 0
    let ctx1 = RenderContext {
        now_ms: 20,
        delta_seconds: 0.02,
        audio: &audio,
        color: Rgb::WHITE,
        text: "",
        scroll_speed: 0.0,
    };
    frames_effect.render(&mut output, &ctx1);
    assert_eq!(output.get(0, 0), Rgb::new(255, 0, 0));

    // Render after 60ms total -> should advance to frame 1 (Green)
    let ctx2 = RenderContext {
        now_ms: 60,
        delta_seconds: 0.04,
        audio: &audio,
        color: Rgb::WHITE,
        text: "",
        scroll_speed: 0.0,
    };
    frames_effect.render(&mut output, &ctx2);
    assert_eq!(output.get(0, 0), Rgb::new(0, 255, 0));

    // Jump 110ms ahead -> should advance to frame 2 then loop
    let ctx3 = RenderContext {
        now_ms: 170,
        delta_seconds: 0.11,
        audio: &audio,
        color: Rgb::WHITE,
        text: "",
        scroll_speed: 0.0,
    };
    frames_effect.render(&mut output, &ctx3);
    // 170 - 50 = 120ms / 50 = 2 frames advanced: 1 -> 2 -> 0
    assert_eq!(output.get(0, 0), Rgb::new(255, 0, 0));

    // Error handling: if source has 0 frames or reading fails, clear to black
    struct FailingSource;
    impl FrameSource for FailingSource {
        fn frame_count(&self) -> usize {
            2
        }
        fn read_frame_into(&mut self, _index: usize, _frame: &mut Frame) -> Result<(), String> {
            Err("I/O error reading flash".to_string())
        }
    }

    let mut failing_effect = matrix_effects::FramesEffect::from_source(Box::new(FailingSource), 50);
    failing_effect.render(&mut output, &ctx0);
    assert_eq!(output.get(0, 0), Rgb::BLACK);
}

#[test]
fn test_parameter_changes_affect_render() {
    let audio_active = AudioSnapshot {
        available: true,
        level: 0.8,
        peak: 0.95,
        beat: true,
        bands: [0.75; 16],
        waveform: [0.5; 128],
    };

    // 1. SolidEffect: default white vs context color
    let mut solid = matrix_effects::SolidEffect::new();
    let mut f1 = Frame::default();
    let ctx_white = RenderContext {
        now_ms: 100,
        delta_seconds: 0.016,
        audio: &audio_active,
        color: Rgb::new(255, 0, 0),
        text: "",
        scroll_speed: 0.0,
    };
    solid.render(&mut f1, &ctx_white);
    assert_eq!(f1.get(0, 0), Rgb::new(255, 0, 0));

    // 2. TestPatternEffect: quadrants verification
    let mut pattern = matrix_effects::TestPatternEffect::new();
    let mut f_pat = Frame::default();
    pattern.render(&mut f_pat, &ctx_white);
    // Top-left (red), top-right (green), bottom-left (blue), bottom-right (yellow)
    assert_eq!(
        f_pat.get(2, 12),
        Rgb::new(255, 0, 0),
        "Top-left quadrant red"
    );
    assert_eq!(
        f_pat.get(24, 8),
        Rgb::new(0, 255, 0),
        "Top-right quadrant green"
    );
    assert_eq!(
        f_pat.get(8, 24),
        Rgb::new(0, 0, 255),
        "Bottom-left quadrant blue"
    );
    assert_eq!(
        f_pat.get(24, 24),
        Rgb::new(255, 255, 0),
        "Bottom-right quadrant yellow"
    );
    // Center diagonal arrows
    assert_eq!(f_pat.get(1, 1), Rgb::WHITE, "Corner arrow diagonal");

    // 3. TextStaticEffect: renders non-black pixels matching text
    let mut text_static = matrix_effects::TextStaticEffect::new();
    let mut f_text = Frame::default();
    let ctx_text = RenderContext {
        now_ms: 100,
        delta_seconds: 0.016,
        audio: &audio_active,
        color: Rgb::new(0, 255, 0),
        text: "HI",
        scroll_speed: 0.0,
    };
    text_static.render(&mut f_text, &ctx_text);
    let non_black_text = f_text.pixels.iter().filter(|&&p| p != Rgb::BLACK).count();
    assert!(non_black_text > 0, "TextStatic should render characters");

    // 4. TextScrollEffect: scrolls horizontally over time
    let mut text_scroll = matrix_effects::TextScrollEffect::new();
    let mut f_scroll1 = Frame::default();
    let mut f_scroll2 = Frame::default();
    let ctx_s1 = RenderContext {
        now_ms: 100,
        delta_seconds: 0.0,
        audio: &audio_active,
        color: Rgb::WHITE,
        text: "SCROLLING TEST",
        scroll_speed: 40.0,
    };
    text_scroll.render(&mut f_scroll1, &ctx_s1);
    let ctx_s2 = RenderContext {
        now_ms: 300,
        delta_seconds: 0.2,
        audio: &audio_active,
        color: Rgb::WHITE,
        text: "SCROLLING TEST",
        scroll_speed: 40.0,
    };
    text_scroll.render(&mut f_scroll2, &ctx_s2);
    assert_ne!(
        f_scroll1.pixels, f_scroll2.pixels,
        "TextScroll should advance over time"
    );

    // 5. Parameterized effects: changing palette / color mode produces different renders
    let mut effects = builtin_effects();
    for effect in &mut effects {
        let eid = effect.id();
        let params = effect.parameters().to_vec();

        // Check if effect has a "palette" or "color_mode" parameter
        if let Some(pal_param) = params
            .iter()
            .find(|p| p.id == "palette" || p.id == "color_mode")
        {
            if pal_param.max >= 1.0 {
                effect.reset();
                effect.set_parameter(pal_param.id, pal_param.min).unwrap();
                let mut frame_p0 = Frame::default();
                for step in 1..=10 {
                    let ctx = RenderContext {
                        now_ms: step * 30,
                        delta_seconds: 0.03,
                        audio: &audio_active,
                        color: Rgb::WHITE,
                        text: "TEST",
                        scroll_speed: 30.0,
                    };
                    effect.render(&mut frame_p0, &ctx);
                }

                effect.reset();
                effect
                    .set_parameter(pal_param.id, pal_param.min + 1.0)
                    .unwrap();
                let mut frame_p1 = Frame::default();
                for step in 1..=10 {
                    let ctx = RenderContext {
                        now_ms: step * 30,
                        delta_seconds: 0.03,
                        audio: &audio_active,
                        color: Rgb::WHITE,
                        text: "TEST",
                        scroll_speed: 30.0,
                    };
                    effect.render(&mut frame_p1, &ctx);
                }

                assert_ne!(
                    frame_p0.pixels,
                    frame_p1.pixels,
                    "Effect '{}' parameter '{}' did not change render between {} and {}",
                    eid,
                    pal_param.id,
                    pal_param.min,
                    pal_param.min + 1.0
                );
            }
        }

        // Check scale / size / count parameters
        if let Some(size_param) = params.iter().find(|p| {
            p.id == "scale"
                || p.id == "size"
                || p.id == "particle_count"
                || p.id == "star_count"
                || p.id == "amplitude"
        }) {
            effect.reset();
            effect.set_parameter(size_param.id, size_param.min).unwrap();
            let mut frame_min = Frame::default();
            for step in 1..=5 {
                let ctx = RenderContext {
                    now_ms: step * 30,
                    delta_seconds: 0.03,
                    audio: &audio_active,
                    color: Rgb::WHITE,
                    text: "TEST",
                    scroll_speed: 30.0,
                };
                effect.render(&mut frame_min, &ctx);
            }

            effect.reset();
            effect.set_parameter(size_param.id, size_param.max).unwrap();
            let mut frame_max = Frame::default();
            for step in 1..=5 {
                let ctx = RenderContext {
                    now_ms: step * 30,
                    delta_seconds: 0.03,
                    audio: &audio_active,
                    color: Rgb::WHITE,
                    text: "TEST",
                    scroll_speed: 30.0,
                };
                effect.render(&mut frame_max, &ctx);
            }

            assert_ne!(
                frame_min.pixels, frame_max.pixels,
                "Effect '{}' parameter '{}' did not change render between min and max",
                eid, size_param.id
            );
        }
    }
}

#[test]
fn test_silent_and_audio_reactive_behavior() {
    let audio_silent = AudioSnapshot {
        available: false,
        level: 0.0,
        peak: 0.0,
        beat: false,
        bands: [0.0; 16],
        waveform: [0.0; 128],
    };

    let audio_active = AudioSnapshot {
        available: true,
        level: 0.9,
        peak: 1.0,
        beat: true,
        bands: [0.85; 16],
        waveform: [0.7; 128],
    };

    let mut effects = builtin_effects();
    for effect in &mut effects {
        let eid = effect.id();
        let params = effect.parameters().to_vec();

        let audio_param = params
            .iter()
            .find(|p| p.id == "audio_reactive" || p.id == "audio_enabled");

        if let Some(ap) = audio_param {
            // Enable audio reactivity
            effect.reset();
            effect.set_parameter(ap.id, 1.0).unwrap();
            if eid == "stroboscope" {
                // In Beat Sync mode, audio directly drives flashes (vs 1Hz standby)
                effect.set_parameter("mode", 1.0).unwrap();
            }

            // Render under silent conditions
            let mut frame_silent = Frame::default();
            for step in 1..=10 {
                let ctx = RenderContext {
                    now_ms: step * 30,
                    delta_seconds: 0.03,
                    audio: &audio_silent,
                    color: Rgb::WHITE,
                    text: "TEST",
                    scroll_speed: 30.0,
                };
                effect.render(&mut frame_silent, &ctx);
            }

            // Render under active audio conditions
            effect.reset();
            effect.set_parameter(ap.id, 1.0).unwrap();
            let mut frame_active = Frame::default();
            for step in 1..=10 {
                let ctx = RenderContext {
                    now_ms: step * 30,
                    delta_seconds: 0.03,
                    audio: &audio_active,
                    color: Rgb::WHITE,
                    text: "TEST",
                    scroll_speed: 30.0,
                };
                effect.render(&mut frame_active, &ctx);
            }

            assert_ne!(
                frame_silent.pixels, frame_active.pixels,
                "Audio-reactive effect '{}' should produce different output with active audio vs silence",
                eid
            );
        }
    }
}

#[test]
fn test_bounded_work_with_long_timestamp_gaps() {
    let audio = AudioSnapshot {
        available: true,
        level: 0.5,
        peak: 0.5,
        beat: false,
        bands: [0.5; 16],
        waveform: [0.0; 128],
    };

    let mut effects = builtin_effects();
    for effect in &mut effects {
        let eid = effect.id();
        effect.reset();
        let mut frame = Frame::default();

        // 1. Initial normal frame
        let ctx0 = RenderContext {
            now_ms: 1000,
            delta_seconds: 0.016,
            audio: &audio,
            color: Rgb::WHITE,
            text: "TEST",
            scroll_speed: 25.0,
        };
        effect.render(&mut frame, &ctx0);

        // 2. Huge jump forward: delta_seconds = 1000.0, now_ms leaps 1 hour
        let start = std::time::Instant::now();
        let ctx_huge = RenderContext {
            now_ms: 1000 + 3_600_000,
            delta_seconds: 1000.0,
            audio: &audio,
            color: Rgb::WHITE,
            text: "TEST",
            scroll_speed: 25.0,
        };
        effect.render(&mut frame, &ctx_huge);
        let elapsed = start.elapsed();

        // Must finish in well under 100ms (no spinning indefinitely)
        assert!(
            elapsed.as_millis() < 100,
            "Effect '{}' took too long on huge timestamp jump: {:?}",
            eid,
            elapsed
        );

        // 3. Clock reset backwards: now_ms jumps from 3.6M back to 500
        let ctx_back = RenderContext {
            now_ms: 500,
            delta_seconds: 0.0,
            audio: &audio,
            color: Rgb::WHITE,
            text: "TEST",
            scroll_speed: 25.0,
        };
        effect.render(&mut frame, &ctx_back);

        // Check pixels are valid, no panics
        assert_eq!(frame.pixels.len(), PIXELS);
    }
}

#[test]
fn test_frames_streaming_catchup_and_recovery() {
    let mut buffer = Vec::new();
    let frame_count = 5u16;
    let mut writer = LfxWriter::new(&mut buffer, frame_count).expect("create LfxWriter");

    let colors = [
        Rgb::new(255, 0, 0),
        Rgb::new(0, 255, 0),
        Rgb::new(0, 0, 255),
        Rgb::new(255, 255, 0),
        Rgb::new(255, 0, 255),
    ];

    for c in &colors {
        let mut f = Frame::default();
        f.pixels.fill(*c);
        writer.write_frame(&f).expect("write frame");
    }

    let cursor = std::io::Cursor::new(buffer);
    let stream = LfxStream::new(cursor).expect("create stream");
    let mut frames_effect = matrix_effects::FramesEffect::from_source(Box::new(stream), 100);

    let audio = AudioSnapshot::default();
    let mut output = Frame::default();

    // Initial frame at t=0 -> Frame 0 (Red)
    let ctx0 = RenderContext {
        now_ms: 0,
        delta_seconds: 0.0,
        audio: &audio,
        color: Rgb::WHITE,
        text: "",
        scroll_speed: 0.0,
    };
    frames_effect.render(&mut output, &ctx0);
    assert_eq!(output.get(0, 0), colors[0]);

    // Jump 1 hour ahead (3,600,000 ms):
    // 3,600,000 / 100 = 36,000 frames elapsed.
    // 36,000 % 5 = 0 -> Frame 0 (Red)
    let ctx_jump = RenderContext {
        now_ms: 3_600_000,
        delta_seconds: 100.0,
        audio: &audio,
        color: Rgb::WHITE,
        text: "",
        scroll_speed: 0.0,
    };
    let start = std::time::Instant::now();
    frames_effect.render(&mut output, &ctx_jump);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 50,
        "FramesEffect catch-up must be O(1) bounded"
    );
    assert_eq!(output.get(0, 0), colors[0]);

    // Backwards jump: t=100ms
    let ctx_back = RenderContext {
        now_ms: 100,
        delta_seconds: 0.0,
        audio: &audio,
        color: Rgb::WHITE,
        text: "",
        scroll_speed: 0.0,
    };
    frames_effect.render(&mut output, &ctx_back);
    // Does not panic or lock up

    // Test transient failure then recovery
    struct FlakySource {
        should_fail: bool,
    }
    impl FrameSource for FlakySource {
        fn frame_count(&self) -> usize {
            2
        }
        fn read_frame_into(&mut self, _index: usize, frame: &mut Frame) -> Result<(), String> {
            if self.should_fail {
                Err("simulated read failure".to_string())
            } else {
                frame.clear(Rgb::new(42, 84, 126));
                Ok(())
            }
        }
    }

    let mut flaky_effect =
        matrix_effects::FramesEffect::from_source(Box::new(FlakySource { should_fail: true }), 50);
    flaky_effect.render(&mut output, &ctx0);
    assert_eq!(output.get(0, 0), Rgb::BLACK, "Failed read yields black");

    // Replace with recovered source
    flaky_effect.set_source(Box::new(FlakySource { should_fail: false }), 50);
    flaky_effect.render(&mut output, &ctx0);
    assert_eq!(
        output.get(0, 0),
        Rgb::new(42, 84, 126),
        "Recovered read yields frame data"
    );
}
