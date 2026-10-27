use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Optical Mono", "Psychedelic Purple", "Hypno Emerald", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Rotation speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 35.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "arms",
        label: "Spiral arms",
        kind: ParameterKind::Number,
        min: 2.0,
        max: 8.0,
        step: 1.0,
        default: 4.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "pitch",
        label: "Spiral tightness",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 8.0,
        step: 1.0,
        default: 3.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Palette",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 3.0,
        step: 1.0,
        default: 3.0,
        options: PALETTES,
        unit: "",
    },
    ParameterDescriptor {
        id: "audio_reactive",
        label: "Audio reactive",
        kind: ParameterKind::Boolean,
        min: 0.0,
        max: 1.0,
        step: 1.0,
        default: 1.0,
        options: &[],
        unit: "",
    },
];

pub struct HypnoSpiralsEffect {
    values: [f32; 5],
    phase1: f32,
    phase2: f32,
    color_cycle: f32,
    beat_pulse: f32,
}

impl Default for HypnoSpiralsEffect {
    fn default() -> Self {
        Self {
            values: [35.0, 4.0, 3.0, 3.0, 1.0],
            phase1: 0.0,
            phase2: 0.0,
            color_cycle: 0.0,
            beat_pulse: 0.0,
        }
    }
}

impl HypnoSpiralsEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn moire_color(pal: usize, color_cycle: f32, intensity: f32, r_norm: f32) -> Rgb {
        let b = clamp01(intensity);
        match pal {
            0 => {
                // Optical Mono (Pure high-contrast monochrome with fringe levels)
                let byte = to_byte(255.0 * b);
                Rgb::new(byte, byte, byte)
            }
            1 => {
                // Psychedelic Purple (Deep violet -> Neon Magenta -> Electric Cyan)
                let r = to_byte(255.0 * b);
                let g = to_byte(20.0 * (1.0 - b) + 120.0 * b.powi(2));
                let blue = to_byte(180.0 * (1.0 - b) + 255.0 * b);
                Rgb::new(r, g, blue)
            }
            2 => {
                // Hypno Emerald (Dark Jade -> Bright Lime -> White Highlights)
                let r = to_byte(20.0 + 120.0 * b.powi(3));
                let g = to_byte(255.0 * b);
                let blue = to_byte(60.0 * (1.0 - b) + 180.0 * b.powi(2));
                Rgb::new(r, g, blue)
            }
            _ => {
                // Rainbow Fade
                let h = (color_cycle + r_norm * 0.5 + b * 0.3) % 1.0;
                hsv_to_rgb(h, 0.95, b)
            }
        }
    }
}

impl Effect for HypnoSpiralsEffect {
    fn id(&self) -> &'static str {
        "hypno_spirals"
    }

    fn name(&self) -> &'static str {
        "Hypnotic Moire Spirals"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "arms" => Some(self.values[1]),
            "pitch" => Some(self.values[2]),
            "palette" => Some(self.values[3]),
            "audio_reactive" => Some(self.values[4]),
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
            "speed" => self.values[0] = value,
            "arms" => self.values[1] = value,
            "pitch" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.phase1 = 0.0;
        self.phase2 = 0.0;
        self.color_cycle = 0.0;
        self.beat_pulse = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::BLACK);

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[4] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.beat_pulse = 1.0;
        } else {
            self.beat_pulse = (self.beat_pulse - dt * 2.8).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + self.beat_pulse * 0.7);
        // Counter-rotating dual spiral phases
        self.phase1 = (self.phase1 + dt * speed * 2.2) % (2.0 * PI);
        self.phase2 = (self.phase2 - dt * speed * 1.8) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let pal = self.values[3] as usize;
        let arms = self.values[1];
        let base_pitch = self.values[2] * (1.0 + bass * 0.4);

        let cx = 15.5f32;
        let cy = 15.5f32;
        let max_r = 16.0f32;

        for y in 0..HEIGHT {
            let dy = y as f32 - cy;
            for x in 0..WIDTH {
                let dx = x as f32 - cx;
                let r = (dx * dx + dy * dy).sqrt();
                let theta = dy.atan2(dx);

                let r_norm = (r / max_r).min(1.0);

                // Spiral 1 (Clockwise)
                let s1 = (arms * theta + base_pitch * r - self.phase1).sin();
                // Spiral 2 (Counter-clockwise)
                let s2 = (arms * theta - base_pitch * r - self.phase2).sin();

                // Optical Moiré interference pattern multiplication
                let moire = ((s1 * s2) + 1.0) * 0.5;

                // Circular shockwave ring from beat
                let ring = if self.beat_pulse > 0.1 {
                    let target_r = (1.0 - self.beat_pulse) * 16.0;
                    if (r - target_r).abs() < 1.5 { 0.4 * self.beat_pulse } else { 0.0 }
                } else {
                    0.0
                };

                let high_sparkle = if audio_active { treble * ((r * 8.0).sin() * 0.2).max(0.0) } else { 0.0 };

                let total_intensity = (moire.powf(1.8) + ring + high_sparkle + mid * 0.1).clamp(0.0, 1.0);

                let col = Self::moire_color(pal, self.color_cycle, total_intensity, r_norm);
                frame.set(x, y, col);
            }
        }
    }
}
