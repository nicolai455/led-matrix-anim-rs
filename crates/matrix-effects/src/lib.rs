pub mod arcade_invaders;
pub mod audio_mesh;
pub mod audio_pulse;
pub mod audio_tunnel;
pub mod audio_waveform;
pub mod black_hole;
pub mod bouncing_speakers;
pub mod cyber_rain;
pub mod cyber_tunnel;
pub mod dancing_banana;
pub mod disco_ball;
pub mod disco_floor;
pub mod dna_spiral;
pub mod domain_warp;
pub mod equalizer_city;
pub mod fire;
pub mod fire_tornado;
pub mod fireworks;
pub mod frames;
pub mod freq_bars;
pub mod galaxy_spiral;
pub mod game_of_life;
pub mod hazard_pulsar;
pub mod hypno_spirals;
pub mod julia_orbit;
pub mod kaleidoscope;
pub mod laser_show;
pub mod lava_lamp;
pub mod lazy;
pub mod lissajous;
pub mod matrix_rain;
pub mod metaballs;
pub mod neon_heart;
pub mod nyan_cat;
pub mod outrun_drive;
pub mod pacman_chase;
pub mod party_dancers;
pub mod plasma;
pub mod polar_lights;
pub mod pulse_ball;
pub mod radial_spectrum;
pub mod rainbow;
pub mod retro_breakout;
pub mod ripple;
pub mod rotating_fish;
pub mod rotozoom;
pub mod scene_sequencer;
pub mod solid;
pub mod spectrogram;
pub mod spectrum_orb;
pub mod spinning_cube;
pub mod spinning_donut;
pub mod spinning_vinyl;
pub mod starfield;
pub mod strobe_blinder;
pub mod stroboscope;
pub mod supernova;
pub mod test_pattern;
pub mod text;
pub mod text_effects;
pub mod ufo_abduction;
pub mod util;
pub mod water_caustics;
pub mod yin_yang;

pub use arcade_invaders::ArcadeInvadersEffect;
pub use audio_mesh::AudioMeshEffect;
pub use audio_pulse::AudioPulseEffect;
pub use audio_tunnel::AudioTunnelEffect;
pub use audio_waveform::AudioWaveformEffect;
pub use black_hole::BlackHoleEffect;
pub use bouncing_speakers::BouncingSpeakersEffect;
pub use cyber_rain::CyberRainEffect;
pub use cyber_tunnel::CyberTunnelEffect;
pub use dancing_banana::DancingBananaEffect;
pub use disco_ball::DiscoBallEffect;
pub use disco_floor::DiscoFloorEffect;
pub use dna_spiral::DnaSpiralEffect;
pub use domain_warp::DomainWarpEffect;
pub use equalizer_city::EqualizerCityEffect;
pub use fire::FireEffect;
pub use fire_tornado::FireTornadoEffect;
pub use fireworks::FireworksEffect;
pub use frames::{frame_animation, FramesEffect};
pub use freq_bars::FreqBarsEffect;
pub use galaxy_spiral::GalaxySpiralEffect;
pub use game_of_life::GameOfLifeEffect;
pub use hazard_pulsar::HazardPulsarEffect;
pub use hypno_spirals::HypnoSpiralsEffect;
pub use julia_orbit::JuliaOrbitEffect;
pub use kaleidoscope::KaleidoscopeEffect;
pub use laser_show::LaserShowEffect;
pub use lava_lamp::LavaLampEffect;
pub use lazy::{EffectFactory, LazyEffect};
pub use lissajous::LissajousEffect;
pub use matrix_rain::MatrixRainEffect;
pub use metaballs::MetaballsEffect;
pub use neon_heart::NeonHeartEffect;
pub use nyan_cat::NyanCatEffect;
pub use outrun_drive::OutrunDriveEffect;
pub use pacman_chase::PacmanChaseEffect;
pub use party_dancers::PartyDancersEffect;
pub use plasma::PlasmaEffect;
pub use polar_lights::PolarLightsEffect;
pub use pulse_ball::PulseBallEffect;
pub use radial_spectrum::RadialSpectrumEffect;
pub use rainbow::RainbowEffect;
pub use retro_breakout::RetroBreakoutEffect;
pub use ripple::RippleEffect;
pub use rotating_fish::RotatingFishEffect;
pub use rotozoom::RotozoomEffect;
pub use scene_sequencer::SceneSequencerEffect;
pub use solid::SolidEffect;
pub use spectrogram::SpectrogramEffect;
pub use spectrum_orb::SpectrumOrbEffect;
pub use spinning_cube::SpinningCubeEffect;
pub use spinning_donut::SpinningDonutEffect;
pub use spinning_vinyl::SpinningVinylEffect;
pub use starfield::StarfieldEffect;
pub use strobe_blinder::StrobeBlinderEffect;
pub use stroboscope::StroboscopeEffect;
pub use supernova::SupernovaEffect;
pub use test_pattern::TestPatternEffect;
pub use text::{
    draw_centered_text, draw_char, draw_text, text_width, TextScrollEffect, TextStaticEffect,
};
pub use text_effects::TextEffectsEffect;
pub use ufo_abduction::UfoAbductionEffect;
pub use water_caustics::WaterCausticsEffect;
pub use yin_yang::YinYangEffect;

use matrix_core::Effect;

/// Returns instances of all 22 built-in effects matching the legacy catalog,
/// wrapped in `LazyEffect` to defer runtime allocation until first render.
pub fn builtin_effects() -> Vec<Box<dyn Effect>> {
    let mut effects: Vec<Box<dyn Effect>> = Vec::with_capacity(22);
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
        Box::new(RotatingFishEffect::new())
    })));
    log::info!("Registered RotatingFish");
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
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(FireworksEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(DnaSpiralEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(LissajousEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(GameOfLifeEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(PolarLightsEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(DomainWarpEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(GalaxySpiralEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(TextEffectsEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(SceneSequencerEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(DiscoBallEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(LaserShowEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(DiscoFloorEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(NyanCatEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(PartyDancersEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(PacmanChaseEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(EqualizerCityEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(StrobeBlinderEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(SpinningVinylEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(BouncingSpeakersEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(NeonHeartEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(LavaLampEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(ArcadeInvadersEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(AudioMeshEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(FireTornadoEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(HypnoSpiralsEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(OutrunDriveEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(SpectrumOrbEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(CyberRainEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(JuliaOrbitEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(RotozoomEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(HazardPulsarEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(SupernovaEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(WaterCausticsEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(RetroBreakoutEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(CyberTunnelEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(YinYangEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(UfoAbductionEffect::new())
    })));
    effects.push(Box::new(LazyEffect::new(|| {
        Box::new(DancingBananaEffect::new())
    })));
    log::info!("Registered all effects!");
    effects
}
