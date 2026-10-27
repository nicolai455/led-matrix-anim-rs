use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Woodstock 1969", "Red & Blue Oil", "Solar Flare", "Acid Liquid"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Swirl speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 40.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "viscosity",
        label: "Fluid viscosity",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 10.0,
        step: 1.0,
        default: 5.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "blob_density",
        label: "Oil droplet scale",
        kind: ParameterKind::Number,
        min: 2.0,
        max: 8.0,
        step: 1.0,
        default: 4.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Dye colors",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 3.0,
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
        default: 1.0,
        options: &[],
        unit: "",
    },
];

pub struct LiquidOilEffect {
    values: [f32; 5],
    time: f32,
    wheel_rot: f32,
    strobe_pulse: f32,
    color_cycle: f32,
}

impl Default for LiquidOilEffect {
    fn default() -> Self {
        Self {
            values: [40.0, 5.0, 4.0, 0.0, 1.0],
            time: 0.0,
            wheel_rot: 0.0,
            strobe_pulse: 0.0,
            color_cycle: 0.0,
        }
    }
}

impl LiquidOilEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn oil_color(pal: usize, field_val: f32, color_cycle: f32, brightness: f32) -> Rgb {
        let b = clamp01(brightness);
        let t = clamp01(field_val * 0.5 + 0.5);

        match pal {
            0 => {
                // Woodstock 1969: Deep Violet / Hot Magenta / Bright Gold
                if t < 0.35 {
                    let sub = t / 0.35;
                    Rgb::new(to_byte(40.0 * b + sub * 180.0 * b), to_byte(10.0 * b), to_byte(180.0 * b))
                } else if t < 0.7 {
                    let sub = (t - 0.35) / 0.35;
                    Rgb::new(to_byte(220.0 * b), to_byte(sub * 160.0 * b), to_byte((1.0 - sub) * 160.0 * b))
                } else {
                    let sub = (t - 0.7) / 0.3;
                    Rgb::new(to_byte(255.0 * b), to_byte((160.0 + sub * 90.0) * b), to_byte(sub * 60.0 * b))
                }
            }
            1 => {
                // Red & Blue Oil: immiscible crimson and ultramarine droplets
                if t < 0.5 {
                    let sub = t / 0.5;
                    Rgb::new(to_byte(255.0 * b * (0.6 + 0.4 * sub)), to_byte(15.0 * b), to_byte(30.0 * b))
                } else {
                    let sub = (t - 0.5) / 0.5;
                    Rgb::new(to_byte(15.0 * b), to_byte(80.0 * b * sub), to_byte(255.0 * b * (0.6 + 0.4 * sub)))
                }
            }
            2 => {
                // Solar Flare: Deep Ruby -> Blazing Amber -> Sun White
                if t < 0.6 {
                    let sub = t / 0.6;
                    Rgb::new(to_byte(255.0 * b), to_byte(sub * 180.0 * b), to_byte(10.0 * b))
                } else {
                    let sub = (t - 0.6) / 0.4;
                    Rgb::new(to_byte(255.0 * b), to_byte(255.0 * b), to_byte(sub * 240.0 * b))
                }
            }
            _ => {
                // Acid Liquid: Rainbow fluid spectrum
                let h = (color_cycle + t * 0.8) % 1.0;
                hsv_to_rgb(h, 0.95, b)
            }
        }
    }
}

impl Effect for LiquidOilEffect {
    fn id(&self) -> &'static str {
        "liquid_oil"
    }

    fn name(&self) -> &'static str {
        "Liquid Disco Oil"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "viscosity" => Some(self.values[1]),
            "blob_density" => Some(self.values[2]),
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
            "viscosity" => self.values[1] = value,
            "blob_density" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.time = 0.0;
        self.wheel_rot = 0.0;
        self.strobe_pulse = 0.0;
        self.color_cycle = 0.0;
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
            self.strobe_pulse = 1.0;
        } else {
            self.strobe_pulse = (self.strobe_pulse - dt * 3.5).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.8);
        self.time += dt * speed * 1.5;
        self.wheel_rot = (self.wheel_rot + dt * speed * 0.5) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let pal = self.values[3] as usize;
        let visc = self.values[1] * 0.12;
        let scale = self.values[2] * 0.08;

        let cx = 15.5f32;
        let cy = 15.5f32;
        let cos_w = self.wheel_rot.cos();
        let sin_w = self.wheel_rot.sin();

        // 32x32 Liquid Oil Projection Simulation
        for y in 0..HEIGHT {
            let dy = y as f32 - cy;
            for x in 0..WIDTH {
                let dx = x as f32 - cx;

                // Circular overhead projector glass aperture
                let r2 = dx * dx + dy * dy;
                if r2 > 240.0 {
                    continue; // Outside projector rim
                }

                // Rotated coordinates inside the oil wheel disc
                let rx = dx * cos_w - dy * sin_w;
                let ry = dx * sin_w + dy * cos_w;

                // Convection currents & immiscible fluid harmonic wave superposition
                let u = rx * scale;
                let v = ry * scale;

                let wave1 = (u * 1.2 + self.time * 1.8).sin();
                let wave2 = (v * 1.4 - self.time * 1.4).cos();
                let wave3 = ((u + v) * 0.8 + (self.time * 2.2).sin() * visc).sin();
                let wave4 = ((u * 0.7 - v * 0.9) * 1.3 - self.time).cos();

                // Audio turbulence deformation
                let audio_turb = if audio_active {
                    ((rx * 0.3 + ry * 0.3).sin()) * (bass * 0.6 + treble * 0.4)
                } else {
                    0.0
                };

                let field = (wave1 + wave2 + wave3 + wave4) * 0.25 + audio_turb;

                // Surface tension meniscus / bubble edge sharpening
                let meniscus = (field * 3.0).fract().abs();
                let is_meniscus = (meniscus - 0.5).abs() > 0.42;

                let edge_glow = if is_meniscus { 0.35 + mid * 0.25 } else { 0.0 };

                let d_edge = (15.5 - r2.sqrt()).max(0.0) / 15.5;
                let bright = (d_edge.powf(0.4) * (0.8 + edge_glow + self.strobe_pulse * 0.3)).clamp(0.0, 1.0);

                let col = Self::oil_color(pal, field, self.color_cycle, bright);
                frame.set(x, y, col);
            }
        }

        // Projector glass bezel ring (outer border)
        let bezel_b = to_byte(120.0 + self.strobe_pulse * 100.0);
        let bezel_c = Rgb::new(bezel_b, bezel_b, bezel_b);
        for y in 0..HEIGHT {
            let dy = y as f32 - cy;
            for x in 0..WIDTH {
                let dx = x as f32 - cx;
                let r2 = dx * dx + dy * dy;
                // Bezel ring at radius ~15.5 (14.6^2 ≈ 213, 16.4^2 ≈ 269)
                if (213.0..=269.0).contains(&r2) {
                    frame.set(x, y, bezel_c);
                }
            }
        }

        // Treble sparkles in fluid
        if audio_active && treble > 0.4 {
            for i in 0..4 {
                let sx = (i * 9 + 4) % WIDTH;
                let sy = (i * 7 + 3) % HEIGHT;
                frame.set(sx, sy, Rgb::WHITE);
            }
        }
    }
}
