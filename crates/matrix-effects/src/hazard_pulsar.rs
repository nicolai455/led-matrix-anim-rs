use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Toxic Yellow", "Radiation Red", "Cyber Cyan", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Spin speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 35.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "radius",
        label: "Symbol size",
        kind: ParameterKind::Number,
        min: 6.0,
        max: 14.0,
        step: 1.0,
        default: 11.0,
        options: &[],
        unit: "px",
    },
    ParameterDescriptor {
        id: "shield_ring",
        label: "Shield aura",
        kind: ParameterKind::Boolean,
        min: 0.0,
        max: 1.0,
        step: 1.0,
        default: 1.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Color theme",
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

pub struct HazardPulsarEffect {
    values: [f32; 5],
    spin_angle: f32,
    color_cycle: f32,
    alarm_flash: f32,
}

impl Default for HazardPulsarEffect {
    fn default() -> Self {
        Self {
            values: [35.0, 11.0, 1.0, 0.0, 1.0],
            spin_angle: 0.0,
            color_cycle: 0.0,
            alarm_flash: 0.0,
        }
    }
}

impl HazardPulsarEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn symbol_color(pal: usize, color_cycle: f32, brightness: f32, r_norm: f32) -> Rgb {
        let b = clamp01(brightness);
        match pal {
            0 => {
                // Toxic Yellow (High contrast caution yellow & amber)
                let r = to_byte(255.0 * b);
                let g = to_byte(210.0 * b);
                let blue = to_byte(15.0 * b);
                Rgb::new(r, g, blue)
            }
            1 => {
                // Radiation Red
                let r = to_byte(255.0 * b);
                let g = to_byte(25.0 * b.powi(2));
                let blue = to_byte(40.0 * b.powi(2));
                Rgb::new(r, g, blue)
            }
            2 => {
                // Cyber Cyan
                let r = to_byte(20.0 * b);
                let g = to_byte(235.0 * b);
                let blue = to_byte(255.0 * b);
                Rgb::new(r, g, blue)
            }
            _ => {
                // Rainbow
                let h = (color_cycle + r_norm * 0.4) % 1.0;
                hsv_to_rgb(h, 0.95, b)
            }
        }
    }
}

impl Effect for HazardPulsarEffect {
    fn id(&self) -> &'static str {
        "hazard_pulsar"
    }

    fn name(&self) -> &'static str {
        "Hazard Pulsar"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "radius" => Some(self.values[1]),
            "shield_ring" => Some(self.values[2]),
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
            "radius" => self.values[1] = value,
            "shield_ring" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.spin_angle = 0.0;
        self.color_cycle = 0.0;
        self.alarm_flash = 0.0;
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
            self.alarm_flash = 1.0;
        } else {
            self.alarm_flash = (self.alarm_flash - dt * 3.5).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.8);
        self.spin_angle = (self.spin_angle + dt * speed * 2.0) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.14) % 1.0;

        let pal = self.values[3] as usize;
        let max_r = self.values[1] * (1.0 + bass * 0.25);
        let show_shield = self.values[2] > 0.5;

        let cx = 15.5f32;
        let cy = 15.5f32;

        let two_pi_over_3 = 2.0 * PI / 3.0;

        let max_r_sq = max_r * max_r;
        let shield_r = max_r + 2.5 + mid * 2.0;
        let shield_min = (shield_r - 1.2).max(0.0);
        let shield_max = shield_r + 1.2;
        let shield_min_sq = shield_min * shield_min;
        let shield_max_sq = shield_max * shield_max;

        for y in 0..HEIGHT {
            let dy = y as f32 - cy;
            for x in 0..WIDTH {
                let dx = x as f32 - cx;
                let d2 = dx * dx + dy * dy;

                let theta = (dy.atan2(dx) - self.spin_angle).rem_euclid(2.0 * PI);

                // Center core dot (r < 2.0 -> d2 < 4.0)
                let in_center_dot = d2 < 4.0;

                // Trifoil blade symmetry (3 blades spaced by 120 degrees)
                let blade_angle = (theta % two_pi_over_3) - (two_pi_over_3 * 0.5);
                let in_blade_pie = blade_angle.abs() < 0.55;
                let in_blade_radial = (12.96..=max_r_sq).contains(&d2);

                // Inner circle cutout slot in each blade (r: 5.0..=7.2 -> d2: 25.0..=51.84)
                let in_slot_radial = (25.0..=51.84).contains(&d2);
                let in_slot_pie = blade_angle.abs() < 0.25;
                let in_cutout = in_slot_radial && in_slot_pie;

                let is_hazard_body = in_center_dot || (in_blade_pie && in_blade_radial && !in_cutout);

                // Shield aura ring at periphery
                let is_shield = show_shield && (shield_min_sq..=shield_max_sq).contains(&d2);

                if is_hazard_body {
                    let r = d2.sqrt();
                    let r_norm = r / max_r;
                    let bright = (0.85 + 0.15 * (r_norm * 4.0).sin() + self.alarm_flash * 0.4).min(1.0);
                    let mut col = Self::symbol_color(pal, self.color_cycle, bright, r_norm);
                    if self.alarm_flash > 0.4 {
                        let f_b = to_byte(self.alarm_flash * 140.0);
                        col.r = col.r.saturating_add(f_b);
                        col.g = col.g.saturating_add(f_b);
                        col.b = col.b.saturating_add(f_b);
                    }
                    frame.set(x, y, col);
                } else if is_shield {
                    let shield_bright = to_byte((160.0 + treble * 95.0) * (0.6 + 0.4 * (theta * 6.0).sin()));
                    let col = Rgb::new(shield_bright, to_byte(shield_bright as f32 * 0.7), to_byte(shield_bright as f32 * 0.2));
                    frame.set(x, y, col);
                }
            }
        }
    }
}
