use crate::util::{alloc_boxed_2d, clamp01, to_byte, XorShift32};
use crate::{
    AudioTunnelEffect, BlackHoleEffect, DnaSpiralEffect, DomainWarpEffect, FireEffect,
    FireworksEffect, GalaxySpiralEffect, GameOfLifeEffect, KaleidoscopeEffect, LissajousEffect,
    MatrixRainEffect, MetaballsEffect, PlasmaEffect, PolarLightsEffect, SpinningCubeEffect,
    SpinningDonutEffect, StarfieldEffect, TextEffectsEffect,
};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const TRANSITIONS: &[&str] = &["Cross Fade", "Wipe Horizontal", "Wipe Vertical", "Dissolve"];
const PLAYLISTS: &[&str] = &["Top Audio", "Cosmic Voyage", "Demoscene", "All Cool 2D"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "duration",
        label: "Scene duration",
        kind: ParameterKind::Number,
        min: 5.0,
        max: 60.0,
        step: 1.0,
        default: 20.0,
        options: &[],
        unit: "s",
    },
    ParameterDescriptor {
        id: "transition_time",
        label: "Transition time",
        kind: ParameterKind::Number,
        min: 0.5,
        max: 5.0,
        step: 0.5,
        default: 2.0,
        options: &[],
        unit: "s",
    },
    ParameterDescriptor {
        id: "transition",
        label: "Transition style",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 3.0,
        step: 1.0,
        default: 0.0,
        options: TRANSITIONS,
        unit: "",
    },
    ParameterDescriptor {
        id: "playlist",
        label: "Playlist",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 3.0,
        step: 1.0,
        default: 0.0,
        options: PLAYLISTS,
        unit: "",
    },
];

type EffectFactory = fn() -> Box<dyn Effect>;

pub struct SceneSequencerEffect {
    values: [f32; 4],
    current_effect: Option<Box<dyn Effect>>,
    next_effect: Option<Box<dyn Effect>>,
    frame_a: Box<Frame>,
    frame_b: Box<Frame>,
    dissolve_noise: Box<[[u8; WIDTH]; HEIGHT]>,
    elapsed_in_scene: f32,
    playlist_idx: usize,
    transitioning: bool,
    transition_progress: f32,
}

impl Default for SceneSequencerEffect {
    fn default() -> Self {
        let mut rng = XorShift32::new(54321);
        let mut noise = alloc_boxed_2d(0u8);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                noise[y][x] = (rng.next_f32() * 255.0) as u8;
            }
        }

        Self {
            values: [20.0, 2.0, 0.0, 0.0],
            current_effect: None,
            next_effect: None,
            frame_a: Box::new(Frame::default()),
            frame_b: Box::new(Frame::default()),
            dissolve_noise: noise,
            elapsed_in_scene: 0.0,
            playlist_idx: 0,
            transitioning: false,
            transition_progress: 0.0,
        }
    }
}

impl SceneSequencerEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_playlist(&self) -> &'static [EffectFactory] {
        let pl_idx = self.values[3] as usize;
        match pl_idx {
            0 => &[
                // Top Audio
                || Box::new(MetaballsEffect::new()),
                || Box::new(KaleidoscopeEffect::new()),
                || Box::new(AudioTunnelEffect::new()),
                || Box::new(FireworksEffect::new()),
                || Box::new(GalaxySpiralEffect::new()),
                || Box::new(DnaSpiralEffect::new()),
                || Box::new(LissajousEffect::new()),
                || Box::new(PolarLightsEffect::new()),
                || Box::new(DomainWarpEffect::new()),
                || Box::new(GameOfLifeEffect::new()),
                || Box::new(TextEffectsEffect::new()),
            ],
            1 => &[
                // Cosmic Voyage
                || Box::new(GalaxySpiralEffect::new()),
                || Box::new(StarfieldEffect::new()),
                || Box::new(BlackHoleEffect::new()),
                || Box::new(AudioTunnelEffect::new()),
                || Box::new(PolarLightsEffect::new()),
            ],
            2 => &[
                // Demoscene
                || Box::new(PlasmaEffect::new()),
                || Box::new(SpinningDonutEffect::new()),
                || Box::new(SpinningCubeEffect::new()),
                || Box::new(FireEffect::new()),
                || Box::new(MatrixRainEffect::new()),
                || Box::new(LissajousEffect::new()),
            ],
            _ => &[
                // All Cool 2D
                || Box::new(MetaballsEffect::new()),
                || Box::new(KaleidoscopeEffect::new()),
                || Box::new(FireworksEffect::new()),
                || Box::new(PolarLightsEffect::new()),
                || Box::new(DomainWarpEffect::new()),
                || Box::new(GameOfLifeEffect::new()),
                || Box::new(AudioTunnelEffect::new()),
                || Box::new(GalaxySpiralEffect::new()),
            ],
        }
    }

    fn ensure_active(&mut self) {
        if self.current_effect.is_none() {
            let pl = self.get_playlist();
            if !pl.is_empty() {
                self.playlist_idx %= pl.len();
                self.current_effect = Some(pl[self.playlist_idx]());
                if let Some(ref mut eff) = self.current_effect {
                    eff.reset();
                }
            }
        }
    }
}

impl Effect for SceneSequencerEffect {
    fn id(&self) -> &'static str {
        "scene_sequencer"
    }

    fn name(&self) -> &'static str {
        "Scene Sequencer"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "duration" => Some(self.values[0]),
            "transition_time" => Some(self.values[1]),
            "transition" => Some(self.values[2]),
            "playlist" => Some(self.values[3]),
            _ => None,
        }
    }

    fn set_parameter(&mut self, id: &str, value: f32) -> Result<(), String> {
        let desc = self
            .parameters()
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("effect '{}' has no parameter '{}'", self.id(), id))?;
        desc.validate_value(value)?;
        match id {
            "duration" => self.values[0] = value,
            "transition_time" => self.values[1] = value,
            "transition" => self.values[2] = value,
            "playlist" => {
                if (self.values[3] - value).abs() > 0.01 {
                    self.values[3] = value;
                    self.current_effect = None;
                    self.next_effect = None;
                    self.playlist_idx = 0;
                    self.elapsed_in_scene = 0.0;
                    self.transitioning = false;
                }
            }
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.elapsed_in_scene = 0.0;
        self.transitioning = false;
        self.transition_progress = 0.0;
        if let Some(ref mut eff) = self.current_effect {
            eff.reset();
        }
        if let Some(ref mut eff) = self.next_effect {
            eff.deactivate();
        }
        self.next_effect = None;
    }

    fn deactivate(&mut self) {
        if let Some(ref mut eff) = self.current_effect {
            eff.deactivate();
        }
        if let Some(ref mut eff) = self.next_effect {
            eff.deactivate();
        }
        self.current_effect = None;
        self.next_effect = None;
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let dt = context.delta_seconds.clamp(0.001, 0.1);
        self.ensure_active();

        let scene_duration = self.values[0];
        let transition_time = self.values[1].min(scene_duration * 0.5);
        let trans_style = self.values[2] as usize;

        if !self.transitioning {
            self.elapsed_in_scene += dt;
            if self.elapsed_in_scene >= scene_duration {
                // Begin transition to next effect in playlist
                let pl = self.get_playlist();
                if pl.len() > 1 {
                    let next_idx = (self.playlist_idx + 1) % pl.len();
                    self.next_effect = Some(pl[next_idx]());
                    if let Some(ref mut next) = self.next_effect {
                        next.reset();
                    }
                    self.transitioning = true;
                    self.transition_progress = 0.0;
                } else {
                    self.elapsed_in_scene = 0.0;
                }
            }
        }

        if self.transitioning {
            self.transition_progress += dt / transition_time.max(0.1);
            let progress = clamp01(self.transition_progress);

            // Render current effect into frame_a
            if let Some(ref mut curr) = self.current_effect {
                curr.render(&mut self.frame_a, context);
            }
            // Render next effect into frame_b
            if let Some(ref mut next) = self.next_effect {
                next.render(&mut self.frame_b, context);
            }

            // Blend based on transition style
            match trans_style {
                0 => {
                    // Cross Fade (smooth linear interpolation)
                    let p_b = progress;
                    let p_a = 1.0 - progress;
                    for y in 0..HEIGHT {
                        for x in 0..WIDTH {
                            let pa = self.frame_a.get(x, y);
                            let pb = self.frame_b.get(x, y);
                            let r = to_byte(pa.r as f32 * p_a + pb.r as f32 * p_b);
                            let g = to_byte(pa.g as f32 * p_a + pb.g as f32 * p_b);
                            let b = to_byte(pa.b as f32 * p_a + pb.b as f32 * p_b);
                            frame.set(x, y, Rgb::new(r, g, b));
                        }
                    }
                }
                1 => {
                    // Wipe Horizontal
                    let split_x = (progress * (WIDTH as f32 + 2.0) - 1.0).round() as i32;
                    for y in 0..HEIGHT {
                        for x in 0..WIDTH {
                            let xi = x as i32;
                            if xi < split_x {
                                frame.set(x, y, self.frame_b.get(x, y));
                            } else if xi == split_x {
                                frame.set(x, y, Rgb::WHITE);
                            } else {
                                frame.set(x, y, self.frame_a.get(x, y));
                            }
                        }
                    }
                }
                2 => {
                    // Wipe Vertical
                    let split_y = (progress * (HEIGHT as f32 + 2.0) - 1.0).round() as i32;
                    for y in 0..HEIGHT {
                        let yi = y as i32;
                        for x in 0..WIDTH {
                            if yi < split_y {
                                frame.set(x, y, self.frame_b.get(x, y));
                            } else if yi == split_y {
                                frame.set(x, y, Rgb::WHITE);
                            } else {
                                frame.set(x, y, self.frame_a.get(x, y));
                            }
                        }
                    }
                }
                _ => {
                    // Dissolve
                    let thresh = (progress * 255.0) as u8;
                    for y in 0..HEIGHT {
                        for x in 0..WIDTH {
                            let n = self.dissolve_noise[y][x];
                            if n <= thresh {
                                frame.set(x, y, self.frame_b.get(x, y));
                            } else {
                                frame.set(x, y, self.frame_a.get(x, y));
                            }
                        }
                    }
                }
            }

            if self.transition_progress >= 1.0 {
                // Transition finished: make next the current, drop old
                if let Some(mut old) = self.current_effect.take() {
                    old.deactivate();
                }
                self.current_effect = self.next_effect.take();
                let pl = self.get_playlist();
                self.playlist_idx = (self.playlist_idx + 1) % pl.len();
                self.transitioning = false;
                self.elapsed_in_scene = 0.0;
            }
        } else {
            // Render active effect directly
            if let Some(ref mut curr) = self.current_effect {
                curr.render(frame, context);
            }
        }
    }
}
