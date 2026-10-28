use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Andromeda", "Cosmic Fire", "Quantum Teal", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Speed",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 100.0,
        step: 1.0,
        default: 30.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "arms",
        label: "Spiral arms",
        kind: ParameterKind::Number,
        min: 2.0,
        max: 5.0,
        step: 1.0,
        default: 3.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "tightness",
        label: "Arm twist",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 8.0,
        step: 1.0,
        default: 4.0,
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

pub struct GalaxySpiralEffect {
    values: [f32; 5],
    phase: f32,
    color_cycle: f32,
    core_flare: f32,
}

impl Default for GalaxySpiralEffect {
    fn default() -> Self {
        Self {
            values: [30.0, 3.0, 4.0, 3.0, 1.0],
            phase: 0.0,
            color_cycle: 0.0,
            core_flare: 0.0,
        }
    }
}

impl GalaxySpiralEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn sample_color(pal: usize, color_cycle: f32, brightness: f32, r_norm: f32, arm_angle: f32) -> Rgb {
        let b = clamp01(brightness);
        match pal {
            0 => {
                // Andromeda (Deep space violet / magenta / electric blue / white core)
                let core_white = (1.0 - r_norm * 2.5).max(0.0) * b;
                let r = to_byte(255.0 * ((0.4 * (1.0 - r_norm) + 0.6 * (arm_angle * 0.5).sin().abs()) * b + core_white));
                let g = to_byte(255.0 * ((0.1 * (1.0 - r_norm) + 0.3 * (1.0 - r_norm).powi(2)) * b + core_white));
                let blue = to_byte(255.0 * ((0.8 * r_norm + 0.5 * (1.0 - r_norm)) * b + core_white));
                Rgb::new(r, g, blue)
            }
            1 => {
                // Cosmic Fire (Deep ember / orange / gold core)
                let core_white = (1.0 - r_norm * 3.0).max(0.0) * b;
                let r = to_byte(255.0 * (b.sqrt() + core_white));
                let g = to_byte(255.0 * (b.powi(2) * 0.8 + core_white));
                let blue = to_byte(255.0 * (b.powi(4) * 0.4 + core_white));
                Rgb::new(r, g, blue)
            }
            2 => {
                // Quantum Teal (Dark cyan / neon teal / crystalline core)
                let core_white = (1.0 - r_norm * 2.8).max(0.0) * b;
                let r = to_byte(255.0 * (0.1 * b + core_white));
                let g = to_byte(255.0 * (0.85 * b + core_white));
                let blue = to_byte(255.0 * (0.95 * b + core_white));
                Rgb::new(r, g, blue)
            }
            _ => {
                // Rainbow Fade
                let core_white = (1.0 - r_norm * 3.0).max(0.0) * b;
                let h = (color_cycle + r_norm * 0.5 + arm_angle / (2.0 * PI)) % 1.0;
                let mut base = hsv_to_rgb(h, 0.95, b);
                let w_byte = to_byte(255.0 * core_white);
                base.r = base.r.saturating_add(w_byte);
                base.g = base.g.saturating_add(w_byte);
                base.b = base.b.saturating_add(w_byte);
                base
            }
        }
    }
}

impl Effect for GalaxySpiralEffect {
    fn id(&self) -> &'static str {
        "galaxy_spiral"
    }

    fn name(&self) -> &'static str {
        "Galaxy Spiral"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "arms" => Some(self.values[1]),
            "tightness" => Some(self.values[2]),
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
            "tightness" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.color_cycle = 0.0;
        self.core_flare = 0.0;
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
            self.core_flare = 1.0;
        } else {
            self.core_flare = (self.core_flare - dt * 2.5).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + self.core_flare * 0.5);
        self.phase = (self.phase + dt * speed * 1.4) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.12) % 1.0;

        let pal = self.values[3] as usize;
        let arms = self.values[1];
        let tightness = self.values[2];
        let max_r = 16.0f32;

        let center_x = 15.5f32;
        let center_y = 15.5f32;

        for y in 0..HEIGHT {
            let dy = y as f32 - center_y;
            for x in 0..WIDTH {
                let dx = x as f32 - center_x;
                let r = (dx * dx + dy * dy).sqrt();
                let theta = dy.atan2(dx);

                let r_norm = (r / max_r).min(1.0);

                // Galactic core glow
                let core_r = 2.5 * (1.0 + bass * 0.4 + self.core_flare * 0.6);
                let core_brightness = ((-r / core_r).exp() * 1.2).min(1.0);

                // Logarithmic spiral arms
                // Angle along spiral arm: theta - tightness * ln(r + 1.0)
                let spiral_angle = theta - tightness * (r * 0.35 + 1.0).ln() - self.phase;
                // Periodic arm intensity
                let arm_val = ((arms * spiral_angle).cos() * 0.5 + 0.5).powf(3.0);

                // Radial envelope: fade out at galaxy edge
                let radial_envelope = (1.0 - r_norm).max(0.0).powf(1.2);

                // Shimmer from treble / mid
                let star_sparkle = if audio_active && r > 3.0 {
                    let noise = (spiral_angle * 12.0).sin() * (r * 4.0).cos();
                    if noise > 0.7 { treble * 0.35 } else { 0.0 }
                } else {
                    0.0
                };

                let arm_brightness = arm_val * radial_envelope * (1.0 + mid * 0.4) + star_sparkle;
                let total_brightness = (core_brightness + arm_brightness).clamp(0.0, 1.0);

                if total_brightness > 0.04 {
                    let col = Self::sample_color(pal, self.color_cycle, total_brightness, r_norm, spiral_angle);
                    frame.set(x, y, col);
                }
            }
        }
    }
}
