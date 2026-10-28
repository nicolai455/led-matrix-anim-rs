use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Stellar Fire", "Hypernova Blue", "Antimatter Violet", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Expansion speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 40.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "filaments",
        label: "Plasma filaments",
        kind: ParameterKind::Number,
        min: 4.0,
        max: 16.0,
        step: 1.0,
        default: 8.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "shock_thickness",
        label: "Shockwave width",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 5.0,
        step: 0.5,
        default: 2.5,
        options: &[],
        unit: "px",
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

pub struct SupernovaEffect {
    values: [f32; 5],
    blast_radius: f32,
    blast_age: f32,
    color_cycle: f32,
    core_intensity: f32,
}

impl Default for SupernovaEffect {
    fn default() -> Self {
        Self {
            values: [40.0, 8.0, 2.5, 3.0, 1.0],
            blast_radius: 0.0,
            blast_age: 10.0,
            color_cycle: 0.0,
            core_intensity: 1.0,
        }
    }
}

impl SupernovaEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn plasma_color(pal: usize, color_cycle: f32, brightness: f32, r_norm: f32) -> Rgb {
        let b = clamp01(brightness);
        match pal {
            0 => {
                // Stellar Fire (Ruby / Incandescent Gold / White)
                if b > 0.85 {
                    let w = (b - 0.85) / 0.15;
                    Rgb::new(255, 255, to_byte(180.0 + 75.0 * w))
                } else if b > 0.4 {
                    let t = (b - 0.4) / 0.45;
                    Rgb::new(255, to_byte(140.0 * t), 15)
                } else {
                    let r = to_byte(255.0 * (b / 0.4));
                    Rgb::new(r, 0, 0)
                }
            }
            1 => {
                // Hypernova Blue (Electric Cyan / Ultra-Blue / White core)
                if b > 0.85 {
                    let w = (b - 0.85) / 0.15;
                    Rgb::new(to_byte(180.0 + 75.0 * w), 255, 255)
                } else {
                    let r = to_byte(20.0 * b);
                    let g = to_byte(210.0 * b);
                    let blue = to_byte(255.0 * b);
                    Rgb::new(r, g, blue)
                }
            }
            2 => {
                // Antimatter Violet (Deep Violet / Hot Pink / Neon White)
                if b > 0.8 {
                    let w = (b - 0.8) / 0.2;
                    Rgb::new(255, to_byte(120.0 + 135.0 * w), 255)
                } else {
                    let r = to_byte(255.0 * b);
                    let g = to_byte(15.0 * b);
                    let blue = to_byte(230.0 * b);
                    Rgb::new(r, g, blue)
                }
            }
            _ => {
                // Rainbow Plasma
                let h = (color_cycle + r_norm * 0.5 + b * 0.25) % 1.0;
                hsv_to_rgb(h, 0.95, b)
            }
        }
    }
}

impl Effect for SupernovaEffect {
    fn id(&self) -> &'static str {
        "supernova"
    }

    fn name(&self) -> &'static str {
        "Supernova Shockwave"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "filaments" => Some(self.values[1]),
            "shock_thickness" => Some(self.values[2]),
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
            "filaments" => self.values[1] = value,
            "shock_thickness" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.blast_radius = 0.0;
        self.blast_age = 10.0;
        self.color_cycle = 0.0;
        self.core_intensity = 1.0;
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

        let expansion_speed = (self.values[0] / 30.0) * (20.0 + bass * 18.0);
        self.color_cycle = (self.color_cycle + dt * 0.16) % 1.0;

        // Trigger blast on audio beat or timer
        if beat || self.blast_age > 3.0 {
            self.blast_radius = 0.5;
            self.blast_age = 0.0;
            self.core_intensity = 1.0;
        } else {
            self.blast_age += dt;
            self.blast_radius += dt * expansion_speed;
            self.core_intensity = (self.core_intensity - dt * 1.5).max(0.1);
        }

        let pal = self.values[3] as usize;
        let filaments = self.values[1];
        let thickness = self.values[2] * (1.0 + bass * 0.4);

        let cx = 15.5f32;
        let cy = 15.5f32;
        let max_screen_r = 22.0f32;

        let blast_r = self.blast_radius;

        for y in 0..HEIGHT {
            let dy = y as f32 - cy;
            for x in 0..WIDTH {
                let dx = x as f32 - cx;
                let r = (dx * dx + dy * dy).sqrt();
                let theta = dy.atan2(dx);

                let r_norm = (r / max_screen_r).min(1.0);

                // 1. Central remnant star / pulsar glow
                let core_r = 2.5 + bass * 1.5;
                let core_val = if r <= core_r {
                    (1.0 - (r / core_r)) * self.core_intensity * 1.5
                } else {
                    0.0
                };

                // 2. Expanding Main Shockwave Front with Rayleigh-Taylor Filaments
                let filament_wobble = (filaments * theta + self.blast_age * 4.0).sin() * 1.5
                    + (filaments * 2.0 * theta - self.blast_age * 6.0).cos() * 0.8;

                let current_front_r = blast_r + filament_wobble;
                let dist_to_front = (r - current_front_r).abs();

                let shock_val = if dist_to_front <= thickness && blast_r < 32.0 {
                    let falloff = 1.0 - (dist_to_front / thickness);
                    let age_fade = (1.0 - (self.blast_age / 2.5)).max(0.0);
                    falloff.powf(1.4) * age_fade * (1.0 + mid * 0.4)
                } else {
                    0.0
                };

                // 3. Trailing ejecta debris field inside shockwave
                let inside_blast = r < current_front_r && blast_r < 30.0;
                let debris_val = if inside_blast {
                    let d_fade = (r / current_front_r.max(1.0)).powi(2);
                    let noise = (theta * 8.0 + r * 0.5).sin() * 0.15;
                    (d_fade * 0.4 + noise) * (1.0 - self.blast_age / 2.8).max(0.0)
                } else {
                    0.0
                };

                let total_brightness = (core_val + shock_val + debris_val).clamp(0.0, 1.0);

                if total_brightness > 0.03 {
                    let col = Self::plasma_color(pal, self.color_cycle, total_brightness, r_norm);
                    frame.set(x, y, col);
                }
            }
        }

        // Relativistic synchrotron ray streamers from treble
        if audio_active && treble > 0.45 && self.blast_age < 1.0 {
            for s in 0..4 {
                let ang = (s as f32 * 0.5) * PI + self.color_cycle * 2.0 * PI;
                let rx = (cx + ang.cos() * (blast_r + 4.0)).round() as i32;
                let ry = (cy + ang.sin() * (blast_r + 4.0)).round() as i32;
                if rx >= 0 && rx < WIDTH as i32 && ry >= 0 && ry < HEIGHT as i32 {
                    frame.set(rx as usize, ry as usize, Rgb::WHITE);
                }
            }
        }
    }
}
