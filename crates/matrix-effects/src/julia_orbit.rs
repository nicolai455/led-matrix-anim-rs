use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Electric Violet", "Fire Fractal", "Ice Crystal", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Orbit speed",
        kind: ParameterKind::Number,
        min: 5.0,
        max: 100.0,
        step: 1.0,
        default: 30.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "zoom",
        label: "Magnification",
        kind: ParameterKind::Number,
        min: 8.0,
        max: 30.0,
        step: 1.0,
        default: 14.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "max_iter",
        label: "Detail iterations",
        kind: ParameterKind::Number,
        min: 8.0,
        max: 24.0,
        step: 1.0,
        default: 16.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Fractal color",
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

pub struct JuliaOrbitEffect {
    values: [f32; 5],
    orbit_phase: f32,
    color_cycle: f32,
    pulse: f32,
}

impl Default for JuliaOrbitEffect {
    fn default() -> Self {
        Self {
            values: [30.0, 14.0, 16.0, 3.0, 1.0],
            orbit_phase: 0.0,
            color_cycle: 0.0,
            pulse: 0.0,
        }
    }
}

impl JuliaOrbitEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn fractal_color(pal: usize, color_cycle: f32, iter_norm: f32, escaped: bool) -> Rgb {
        if !escaped {
            return Rgb::new(10, 5, 18); // Dark interior core
        }
        let t = clamp01(iter_norm);

        match pal {
            0 => {
                // Electric Violet
                let r = to_byte(255.0 * (0.4 + 0.6 * (t * 2.0 * PI).sin()));
                let g = to_byte(255.0 * (0.2 + 0.3 * (t * 2.0 * PI + 1.2).sin()));
                let b = to_byte(255.0 * (0.6 + 0.4 * (t * 2.0 * PI + 2.4).cos()));
                Rgb::new(r, g, b)
            }
            1 => {
                // Fire Fractal
                let r = to_byte(255.0 * t.sqrt());
                let g = to_byte(255.0 * t.powi(2) * 0.85);
                let b = to_byte(40.0 * t.powi(4));
                Rgb::new(r, g, b)
            }
            2 => {
                // Ice Crystal
                let r = to_byte(30.0 + 190.0 * t.powi(3));
                let g = to_byte(120.0 + 135.0 * t);
                let b = to_byte(255.0 * (0.5 + 0.5 * t));
                Rgb::new(r, g, b)
            }
            _ => {
                // Rainbow
                let h = (color_cycle + t * 0.85) % 1.0;
                hsv_to_rgb(h, 0.95, (0.3 + 0.7 * t).min(1.0))
            }
        }
    }
}

impl Effect for JuliaOrbitEffect {
    fn id(&self) -> &'static str {
        "julia_orbit"
    }

    fn name(&self) -> &'static str {
        "Fractal Julia Orbit"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "zoom" => Some(self.values[1]),
            "max_iter" => Some(self.values[2]),
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
            "zoom" => self.values[1] = value,
            "max_iter" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.orbit_phase = 0.0;
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
            self.pulse = (self.pulse - dt * 2.8).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.7);
        self.orbit_phase = (self.orbit_phase + dt * speed * 0.8) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let pal = self.values[3] as usize;
        let max_iters = (self.values[2].round() as usize).clamp(8, 24);
        let base_zoom = (self.values[1] / 10.0) * (1.0 + bass * 0.4 + self.pulse * 0.3);

        // Constant c orbits through the boundary of the Mandelbrot set
        // c = -0.7 + 0.27 * cos(t), 0.27 * sin(t)
        let cx = -0.7 + 0.26 * (self.orbit_phase).cos() + if audio_active { mid * 0.08 } else { 0.0 };
        let cy = 0.27 * (self.orbit_phase).sin() + if audio_active { treble * 0.08 } else { 0.0 };

        let scale = 2.4 / (base_zoom * 16.0);

        for y in 0..HEIGHT {
            let zy0 = (y as f32 - 15.5) * scale;
            for x in 0..WIDTH {
                let zx0 = (x as f32 - 15.5) * scale;

                let mut zx = zx0;
                let mut zy = zy0;
                let mut iter = 0;
                let mut escaped = false;

                while iter < max_iters {
                    let zx2 = zx * zx;
                    let zy2 = zy * zy;
                    if zx2 + zy2 > 4.0 {
                        escaped = true;
                        break;
                    }
                    let new_zx = zx2 - zy2 + cx;
                    let new_zy = 2.0 * zx * zy + cy;
                    zx = new_zx;
                    zy = new_zy;
                    iter += 1;
                }

                let iter_norm = iter as f32 / max_iters as f32;
                let col = Self::fractal_color(pal, self.color_cycle, iter_norm, escaped);
                frame.set(x, y, col);
            }
        }
    }
}
