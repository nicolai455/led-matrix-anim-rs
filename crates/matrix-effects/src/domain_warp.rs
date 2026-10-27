use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Bioliquid", "Magma Marble", "Deep Sea", "Rainbow"];

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
        id: "scale",
        label: "Scale",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 30.0,
        step: 1.0,
        default: 12.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "warp_depth",
        label: "Warp depth",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 50.0,
        options: &[],
        unit: "%",
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

pub struct DomainWarpEffect {
    values: [f32; 5],
    phase: f32,
    color_cycle: f32,
    pulse: f32,
}

impl Default for DomainWarpEffect {
    fn default() -> Self {
        Self {
            values: [30.0, 12.0, 50.0, 3.0, 1.0],
            phase: 0.0,
            color_cycle: 0.0,
            pulse: 0.0,
        }
    }
}

impl DomainWarpEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn sample_color(pal: usize, color_cycle: f32, v: f32, q_mag: f32) -> Rgb {
        let val = clamp01(v);
        match pal {
            0 => {
                // Bioliquid (Fluorescent Lime / Cyan / Indigo)
                let r = to_byte(255.0 * (0.1 + 0.9 * val.powi(2)));
                let g = to_byte(255.0 * (0.3 + 0.7 * (val * PI).sin()));
                let b = to_byte(255.0 * (0.6 + 0.4 * q_mag));
                Rgb::new(r, g, b)
            }
            1 => {
                // Magma Marble (Dark basalt, lava orange, gold highlights)
                let r = to_byte(255.0 * val.sqrt());
                let g = to_byte(255.0 * val.powi(2) * 0.85);
                let b = to_byte(255.0 * val.powi(4) * 0.4);
                Rgb::new(r, g, b)
            }
            2 => {
                // Deep Sea (Bioluminescent jellyfish in deep oceanic abyss)
                let r = to_byte(255.0 * val.powi(3) * 0.4);
                let g = to_byte(255.0 * (0.2 * val + 0.7 * val.powi(2)));
                let b = to_byte(255.0 * (0.4 + 0.6 * val));
                Rgb::new(r, g, b)
            }
            _ => {
                // Rainbow Fade
                let h = (color_cycle + val * 0.6 + q_mag * 0.25) % 1.0;
                hsv_to_rgb(h, 0.95, (val * 0.8 + 0.2).min(1.0))
            }
        }
    }
}

impl Effect for DomainWarpEffect {
    fn id(&self) -> &'static str {
        "domain_warp"
    }

    fn name(&self) -> &'static str {
        "Domain Warp"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "scale" => Some(self.values[1]),
            "warp_depth" => Some(self.values[2]),
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
            "scale" => self.values[1] = value,
            "warp_depth" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.color_cycle = 0.0;
        self.pulse = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[4] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.pulse = 1.0;
        } else {
            self.pulse = (self.pulse - dt * 2.2).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + self.pulse * 0.7);
        self.phase = (self.phase + dt * speed * 1.5) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.14) % 1.0;

        let pal = self.values[3] as usize;
        let scale = self.values[1] * 0.1;
        let warp_factor = (self.values[2] / 50.0) * (1.0 + bass * 0.6);

        let t = self.phase;

        for y in 0..HEIGHT {
            let py = (y as f32 - 15.5) * scale;
            for x in 0..WIDTH {
                let px = (x as f32 - 15.5) * scale;

                // Domain warping layer 1: q = (sin(p.x + t), cos(p.y + t))
                let qx = (px + t).sin() + (py * 0.7 - t * 0.5).cos() * 0.5;
                let qy = (py + t * 0.8).cos() + (px * 0.8 + t * 0.6).sin() * 0.5;

                // Domain warping layer 2: r = f(p + warp * q)
                let rx = ((px + warp_factor * qx) * 1.5 + t * 1.2).sin();
                let ry = ((py + warp_factor * qy) * 1.5 - t * 1.1).cos();

                // Audio ripple injection on high frequencies
                let ripple = if audio_active {
                    ((px * 6.0).sin() * (py * 6.0).cos()) * treble * 0.3
                } else {
                    0.0
                };

                // Final composite pattern
                let final_val = 0.5 + 0.5 * ((px + warp_factor * rx + t * 0.4).cos()
                    * (py + warp_factor * ry - t * 0.4).sin()
                    + (qx * qy * 0.5)
                    + (mid * 0.2)
                    + ripple);

                let q_mag = (qx * qx + qy * qy).sqrt().min(1.0);
                let col = Self::sample_color(pal, self.color_cycle, final_val, q_mag);
                frame.set(x, y, col);
            }
        }
    }
}
