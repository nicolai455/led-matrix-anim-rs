use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Psychedelic 90s", "Cyber Wave", "Solar Flame", "Rainbow"];

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
        id: "zoom_freq",
        label: "Zoom cadence",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 40.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "scale",
        label: "Texture density",
        kind: ParameterKind::Number,
        min: 5.0,
        max: 25.0,
        step: 1.0,
        default: 12.0,
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

pub struct RotozoomEffect {
    values: [f32; 5],
    angle: f32,
    zoom_phase: f32,
    color_cycle: f32,
    beat_bump: f32,
}

impl Default for RotozoomEffect {
    fn default() -> Self {
        Self {
            values: [35.0, 40.0, 12.0, 3.0, 1.0],
            angle: 0.0,
            zoom_phase: 0.0,
            color_cycle: 0.0,
            beat_bump: 0.0,
        }
    }
}

impl RotozoomEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn sample_color(pal: usize, color_cycle: f32, pattern: f32) -> Rgb {
        let p = clamp01(pattern);
        match pal {
            0 => {
                // Psychedelic 90s (Amiga Demoscene Neon)
                let r = to_byte(255.0 * (0.5 + 0.5 * (p * 2.0 * PI).sin()));
                let g = to_byte(255.0 * (0.5 + 0.5 * (p * 2.0 * PI + 2.0).sin()));
                let b = to_byte(255.0 * (0.5 + 0.5 * (p * 2.0 * PI + 4.0).sin()));
                Rgb::new(r, g, b)
            }
            1 => {
                // Cyber Wave (Deep Violet -> Bright Cyan -> Hot Pink)
                let r = to_byte(255.0 * (0.2 + 0.8 * (p * PI).sin()));
                let g = to_byte(240.0 * p.powi(2));
                let b = to_byte(255.0 * (1.0 - 0.5 * p));
                Rgb::new(r, g, b)
            }
            2 => {
                // Solar Flame
                let r = to_byte(255.0 * p.sqrt());
                let g = to_byte(240.0 * p.powi(2) * 0.85);
                let b = to_byte(40.0 * p.powi(4));
                Rgb::new(r, g, b)
            }
            _ => {
                // Rainbow
                let h = (color_cycle + p) % 1.0;
                hsv_to_rgb(h, 0.95, 1.0)
            }
        }
    }
}

impl Effect for RotozoomEffect {
    fn id(&self) -> &'static str {
        "rotozoom"
    }

    fn name(&self) -> &'static str {
        "Plasma Rotozoom"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "zoom_freq" => Some(self.values[1]),
            "scale" => Some(self.values[2]),
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
            "zoom_freq" => self.values[1] = value,
            "scale" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.angle = 0.0;
        self.zoom_phase = 0.0;
        self.color_cycle = 0.0;
        self.beat_bump = 0.0;
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
            self.beat_bump = 1.0;
        } else {
            self.beat_bump = (self.beat_bump - dt * 3.0).max(0.0);
        }

        let rot_speed = (self.values[0] / 30.0) * (1.0 + self.beat_bump * 0.8);
        self.angle = (self.angle + dt * rot_speed * 1.5) % (2.0 * PI);

        let zoom_spd = (self.values[1] / 30.0) * (1.0 + bass * 0.5);
        self.zoom_phase = (self.zoom_phase + dt * zoom_spd * 2.0) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.16) % 1.0;

        let pal = self.values[3] as usize;
        let texture_density = self.values[2] * 0.1;

        // Smooth zoom factor: oscillates between close and far + beat plunge
        let zoom = (1.5 + (self.zoom_phase).sin() * 0.9) * (1.0 + bass * 0.4 + self.beat_bump * 0.5);

        let cos_a = self.angle.cos() * zoom * texture_density;
        let sin_a = self.angle.sin() * zoom * texture_density;

        let cx = 15.5f32;
        let cy = 15.5f32;

        for y in 0..HEIGHT {
            let dy = y as f32 - cy;
            for x in 0..WIDTH {
                let dx = x as f32 - cx;

                // Affine rotation & scaling
                let u = dx * cos_a - dy * sin_a;
                let v = dx * sin_a + dy * cos_a;

                // Demoscene plasma texture function
                let dist = (u * u + v * v).sqrt();
                let w1 = (u).sin();
                let w2 = (v).cos();
                let w3 = ((u + v) * 0.707).sin();
                let w4 = (dist * 1.2 - self.color_cycle * 2.0 * PI).sin();
                let high_ripple = if audio_active { (u * 3.0).sin() * (v * 3.0).cos() * treble * 0.3 } else { 0.0 };

                let val = 0.5 + 0.125 * (w1 + w2 + w3 + w4 + mid * 0.4 + high_ripple);
                let col = Self::sample_color(pal, self.color_cycle, val);
                frame.set(x, y, col);
            }
        }
    }
}
