use matrix_core::{AudioSnapshot, Frame, RenderContext, Rgb};
use matrix_effects::builtin_effects;

fn hostile_audio(kind: &str) -> AudioSnapshot {
    let mut audio = AudioSnapshot {
        available: true,
        beat: true,
        ..Default::default()
    };
    let bad = if kind == "nan" { f32::NAN } else { f32::INFINITY };
    audio.bands = [bad; 16];
    audio.waveform = [bad; 128];
    audio.level = bad;
    audio.peak = bad;
    audio.bass = bad;
    audio.onset = bad;
    audio.bpm = bad;
    audio
}

#[test]
fn probe_extreme_inputs() {
    let mut failures = Vec::new();
    for effect in builtin_effects().iter_mut() {
        let id = effect.id();
        eprintln!("PROBE {id}");
        for kind in ["nan", "inf"] {
            let audio = hostile_audio(kind);
            for delta in [1e9f32, 0.5] {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    for frame_idx in 0..40u64 {
                        let now_ms = if frame_idx % 2 == 0 {
                            frame_idx * (u64::MAX / 2 / 40)
                        } else {
                            u64::MAX / 2
                        };
                        let context = RenderContext {
                            now_ms,
                            delta_seconds: delta,
                            audio: &audio,
                            color: Rgb::new(200, 100, 50),
                            text: "HELLO WORLD",
                            scroll_speed: 30.0,
                        };
                        let mut frame = Frame::new();
                        effect.render(&mut frame, &context);
                    }
                }));
                if let Err(payload) = result {
                    let msg = payload
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                        .unwrap_or_else(|| "?".into());
                    failures.push(format!("{id} kind={kind} delta={delta}: {msg}"));
                    break;
                }
            }
        }
    }
    println!("FAILURES:\n{}", failures.join("\n"));
    assert!(failures.is_empty());
}
