use matrix_core::{AudioSnapshot, Frame, RenderContext, Rgb};
use matrix_effects::builtin_effects;

const IDS: &[&str] = &[
    "police_strobe",
    "moving_heads",
    "laser_scanner",
    "disco_pinwheel",
    "octopus_scanner",
    "chaser_cannons",
    "mirror_cavern",
    "laser_harp",
    "derby_flower",
    "liquid_oil",
];

fn context<'a>(dt: f32, audio: &'a AudioSnapshot) -> RenderContext<'a> {
    RenderContext {
        now_ms: 0,
        delta_seconds: dt,
        audio,
        color: Rgb::WHITE,
        text: "",
        scroll_speed: 1.0,
    }
}

#[test]
fn lava_lamp_maximum_audio_parameters_do_not_panic() {
    let mut effect = builtin_effects()
        .into_iter()
        .find(|e| e.id() == "lava_lamp")
        .unwrap();
    for p in effect.parameters().to_vec() {
        effect.set_parameter(p.id, p.max).unwrap();
    }
    let mut audio = AudioSnapshot::default();
    audio.available = true;
    audio.level = 1.0;
    audio.peak = 1.0;
    audio.beat = true;
    audio.bands = [1.0; 16];
    audio.waveform = [1.0; 128];
    let mut frame = Frame::default();
    for _ in 0..35 {
        effect.render(&mut frame, &context(0.1, &audio));
    }
}

#[test]
fn disco_zero_delta_does_not_advance_silent_animation() {
    let audio = AudioSnapshot::default();
    for id in IDS {
        let mut effect = builtin_effects()
            .into_iter()
            .find(|e| e.id() == *id)
            .unwrap();
        let mut first = Frame::default();
        let mut next = Frame::default();
        effect.render(&mut first, &context(0.0, &audio));
        for _ in 0..20 {
            effect.render(&mut next, &context(0.0, &audio));
        }
        assert!(first.pixels == next.pixels, "{id} advances at zero delta");
    }
}

