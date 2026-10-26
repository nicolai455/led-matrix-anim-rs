use crate::util::{clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Rainbow", "Ocean", "Lava"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "Speed",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 100.0,
        step: 1.0,
        default: 30.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "scale",
        label: "Scale",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 50.0,
        step: 1.0,
        default: 15.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Palette",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 2.0,
        step: 1.0,
        default: 0.0,
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
        default: 0.0,
        options: &[],
        unit: "",
    },
];

pub struct PlasmaEffect {
    values: [f32; 4],
    phase: f32,
    palette_shift: f32,
}

impl Default for PlasmaEffect {
    fn default() -> Self {
        Self {
            values: [30.0, 15.0, 0.0, 0.0],
            phase: 0.0,
            palette_shift: 0.0,
        }
    }
}

impl PlasmaEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn color_from_palette(&self, mut t: f32) -> Rgb {
        t = t - t.floor();
        let pal = self.values[2] as usize;
        let two_pi = 2.0 * PI;
        if pal == 1 {
            // Ocean
            let r = to_byte(255.0 * (0.2 + 0.3 * (t * two_pi).sin()));
            let g = to_byte(255.0 * (0.5 + 0.5 * (t * two_pi + 1.2).sin()));
            let b = to_byte(255.0 * (0.7 + 0.3 * (t * two_pi + 0.5).sin()));
            Rgb::new(r, g, b)
        } else if pal == 2 {
            // Lava
            let r = to_byte(255.0 * (0.8 + 0.2 * (t * two_pi).sin()));
            let g = to_byte(255.0 * (0.4 + 0.4 * (t * two_pi + 0.5).sin()));
            let b = to_byte(255.0 * (0.1 + 0.1 * (t * two_pi + 1.0).sin()));
            Rgb::new(r, g, b)
        } else {
            // Rainbow
            hsv_to_rgb(t, 1.0, 1.0)
        }
    }
}

impl Effect for PlasmaEffect {
    fn id(&self) -> &'static str {
        "plasma"
    }

    fn name(&self) -> &'static str {
        "Plasma"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "scale" => Some(self.values[1]),
            "palette" => Some(self.values[2]),
            "audio_reactive" => Some(self.values[3]),
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
            "palette" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.palette_shift = 0.0;
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let mut speed_mul = self.values[0] / 30.0;
        let mut scale_mul = 1.0;

        if self.values[3] != 0.0 && context.audio.available {
            let level = clamp01(context.audio.level);
            speed_mul *= 1.0 + level * 2.0;
            scale_mul = 1.0 + level * 0.5;
            if context.audio.beat {
                self.palette_shift += 0.25;
            }
        }

        let dt = context.delta_seconds.clamp(0.0, 1.0);
        self.phase += 1.875 * speed_mul * dt;
        self.palette_shift += 0.3125 * speed_mul * dt;

        let scale = self.values[1] * 0.1 * scale_mul;
        let two_pi = 2.0 * PI;

        for y in 0..HEIGHT {
            let fy = y as f32 / HEIGHT as f32;
            for x in 0..WIDTH {
                let fx = x as f32 / WIDTH as f32;

                let mut v = (fx * scale * two_pi + self.phase).sin();
                v += (fy * scale * two_pi + self.phase * 1.3).sin();
                v += ((fx + fy) * scale * PI + self.phase * 0.7).sin();
                v += (((fx * fx + fy * fy).sqrt()) * scale * two_pi + self.phase * 1.1).sin();
                v = v * 0.125 + 0.5;

                frame.set(x, y, self.color_from_palette(v + self.palette_shift));
            }
        }
    }
}
