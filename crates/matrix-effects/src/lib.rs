pub mod audio_pulse;
pub mod audio_tunnel;
pub mod audio_waveform;
pub mod black_hole;
pub mod fire;
pub mod frames;
pub mod freq_bars;
pub mod kaleidoscope;
pub mod lazy;
pub mod matrix_rain;
pub mod metaballs;
pub mod plasma;
pub mod pulse_ball;
pub mod radial_spectrum;
pub mod rainbow;
pub mod ripple;
pub mod solid;
pub mod spectrogram;
pub mod spinning_cube;
pub mod spinning_donut;
pub mod starfield;
pub mod stroboscope;
pub mod test_pattern;
pub mod text;
pub mod util;

pub use audio_pulse::AudioPulseEffect;
pub use audio_tunnel::AudioTunnelEffect;
pub use audio_waveform::AudioWaveformEffect;
pub use black_hole::BlackHoleEffect;
pub use fire::FireEffect;
pub use frames::{frame_animation, FramesEffect};
pub use freq_bars::FreqBarsEffect;
pub use kaleidoscope::KaleidoscopeEffect;
pub use lazy::{EffectFactory, LazyEffect};
pub use matrix_rain::MatrixRainEffect;
pub use metaballs::MetaballsEffect;
pub use plasma::PlasmaEffect;
pub use pulse_ball::PulseBallEffect;
pub use radial_spectrum::RadialSpectrumEffect;
pub use rainbow::RainbowEffect;
pub use ripple::RippleEffect;
pub use solid::SolidEffect;
pub use spectrogram::SpectrogramEffect;
pub use spinning_cube::SpinningCubeEffect;
pub use spinning_donut::SpinningDonutEffect;
pub use starfield::StarfieldEffect;
pub use stroboscope::StroboscopeEffect;
pub use test_pattern::TestPatternEffect;
pub use text::{
    draw_centered_text, draw_char, draw_text, text_width, TextScrollEffect, TextStaticEffect,
};

use matrix_core::Effect;

/// Returns instances of all 21 built-in effects matching the legacy catalog,
/// wrapped in `LazyEffect` to defer runtime allocation until first render.
pub fn builtin_effects() -> Vec<Box<dyn Effect>> {
    let mut effects: Vec<Box<dyn Effect>> = Vec::with_capacity(21);
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(TestPatternEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| Box::new(RainbowEffect::new()))));
    effects.push(Box::new(LazyEffect::new(|| Box::new(SolidEffect::new()))));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(TextStaticEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(TextScrollEffect::new())
    })));
    log::info!("Registered static effects");
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(BlackHoleEffect::new())
    })));
    log::info!("Registered BlackHole");
    effects.push(Box::new(LazyEffect::new(|| Box::new(FireEffect::new()))));
    log::info!("Registered Fire");
    effects.push(Box::new(LazyEffect::new(|| Box::new(PlasmaEffect::new()))));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(StarfieldEffect::new())
    })));
    log::info!("Registered Starfield");
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(MatrixRainEffect::new())
    })));
    log::info!("Registered MatrixRain");
    effects.push(Box::new(LazyEffect::new(|| Box::new(RippleEffect::new()))));
    effects.push(Box::new(LazyEffect::new(
        || Box::new(FreqBarsEffect::new()),
    )));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(SpectrogramEffect::new())
    })));
    log::info!("Registered Spectrogram");
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(AudioPulseEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(PulseBallEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(SpinningDonutEffect::new())
    })));
    log::info!("Registered SpinningDonut");
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(SpinningCubeEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(AudioWaveformEffect::new())
    })));
    log::info!("Registered AudioWaveform");
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(RadialSpectrumEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(StroboscopeEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| Box::new(FramesEffect::new()))));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(MetaballsEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(KaleidoscopeEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(AudioTunnelEffect::new())
    })));
    log::info!("Registered effects!");
    effects
}
