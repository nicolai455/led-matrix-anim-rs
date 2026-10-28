use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Neon Rainbow", "Red & Blue Dual", "Gold & Purple", "Toxic Acid"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Spin speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 40.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "blades",
        label: "Pinwheel blades",
        kind: ParameterKind::Number,
        min: 4.0,
        max: 12.0,
        step: 2.0,
        default: 6.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "twist",
        label: "Spiral twist",
        kind: ParameterKind::Number,
        min: 0.0,
        max: 10.0,
        step: 1.0,
        default: 4.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Color palette",
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

pub struct DiscoPinwheelEffect {
    values: [f32; 5],
    spin_cw: f32,
    spin_ccw: f32,
    color_cycle: f32,
    strobe_pulse: f32,
    phase_jump: f32,
}

impl Default for DiscoPinwheelEffect {
    fn default() -> Self {
        Self {
            values: [40.0, 6.0, 4.0, 0.0, 1.0],
            spin_cw: 0.0,
            spin_ccw: 0.0,
            color_cycle: 0.0,
            strobe_pulse: 0.0,
            phase_jump: 0.0,
        }
    }
}

impl DiscoPinwheelEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn blade_color(pal: usize, angle_norm: f32, color_cycle: f32, brightness: f32) -> Rgb {
        let b = clamp01(brightness);
        match pal {
            0 => {
                // Neon Rainbow
                let h = (color_cycle + angle_norm) % 1.0;
                hsv_to_rgb(h, 0.95, b)
            }
            1 => {
                // Red & Blue Dual
                let is_a = angle_norm < 0.5;
                if is_a {
                    Rgb::new(to_byte(255.0 * b), to_byte(20.0 * b), to_byte(30.0 * b))
                } else {
                    Rgb::new(to_byte(10.0 * b), to_byte(120.0 * b), to_byte(255.0 * b))
                }
            }
            2 => {
                // Gold & Purple
                let is_gold = angle_norm < 0.5;
                if is_gold {
                    Rgb::new(to_byte(255.0 * b), to_byte(200.0 * b), to_byte(20.0 * b))
                } else {
                    Rgb::new(to_byte(180.0 * b), to_byte(20.0 * b), to_byte(240.0 * b))
                }
            }
            _ => {
                // Toxic Acid (Lime & Hot Pink)
                let is_lime = angle_norm < 0.5;
                if is_lime {
                    Rgb::new(to_byte(50.0 * b), to_byte(255.0 * b), to_byte(60.0 * b))
                } else {
                    Rgb::new(to_byte(255.0 * b), to_byte(10.0 * b), to_byte(160.0 * b))
                }
            }
        }
    }
}

impl Effect for DiscoPinwheelEffect {
    fn id(&self) -> &'static str {
        "disco_pinwheel"
    }

    fn name(&self) -> &'static str {
        "Disco Pinwheel"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "blades" => Some(self.values[1]),
            "twist" => Some(self.values[2]),
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
            "blades" => self.values[1] = value,
            "twist" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.spin_cw = 0.0;
        self.spin_ccw = 0.0;
        self.color_cycle = 0.0;
        self.strobe_pulse = 0.0;
        self.phase_jump = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::BLACK);

        // Integrate the bounded interval once, then render the final state.
        let dt = if context.delta_seconds.is_finite() {
            context.delta_seconds.clamp(0.0, 0.5)
        } else {
            0.0
        };
        let audio_active = self.values[4] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.strobe_pulse = 1.0;
            self.phase_jump = (self.phase_jump + PI * 0.25) % (2.0 * PI);
        } else {
            self.strobe_pulse = (self.strobe_pulse - dt * 4.0).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.9);
        self.spin_cw = (self.spin_cw + dt * speed * 2.2) % (2.0 * PI);
        self.spin_ccw = (self.spin_ccw - dt * speed * 2.6) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let num_blades = self.values[1];
        let twist = self.values[2] * 0.18;
        let pal = self.values[3] as usize;

        let cx = 15.5f32;
        let cy = 15.5f32;
        let split_r = 8.5f32 + bass * 3.0; // Boundary between inner and outer counter-rotating wheels

        for y in 0..HEIGHT {
            let dy = y as f32 - cy;
            for x in 0..WIDTH {
                let dx = x as f32 - cx;
                let r = (dx * dx + dy * dy).sqrt();
                let ang = dy.atan2(dx); // -PI..PI

                let is_inner = r < split_r;
                let base_spin = if is_inner {
                    self.spin_ccw + self.phase_jump
                } else {
                    self.spin_cw
                };

                // Spiral logarithmic twist
                let twisted_ang = ang + base_spin + r * twist;
                // Sector normalization: 0..1 per blade pair
                let blade_phase = ((twisted_ang * num_blades * 0.5) % (2.0 * PI) + (2.0 * PI)) % (2.0 * PI);
                let blade_norm = blade_phase / (2.0 * PI);

                // Blade duty cycle (half filled, half empty with anti-aliasing edge)
                let blade_edge = (blade_norm - 0.5).abs();
                let in_blade = blade_edge < (0.24 + bass * 0.08);

                if in_blade {
                    let edge_falloff = 1.0 - (blade_edge / 0.25).clamp(0.0, 1.0);
                    let rim_boost = if (r - split_r).abs() < 1.0 { 0.4 } else { 0.0 };

                    let b = (edge_falloff * 0.85 + rim_boost + self.strobe_pulse * 0.35 + mid * 0.2).clamp(0.0, 1.0);
                    let col = Self::blade_color(pal, blade_norm, self.color_cycle, b);
                    frame.set(x, y, col);
                } else if (r - split_r).abs() < 1.0 {
                    // Luminous divider ring between counter-rotating discs
                    let ring_b = to_byte(180.0 + self.strobe_pulse * 75.0);
                    frame.set(x, y, Rgb::new(ring_b, ring_b, ring_b));
                }
            }
        }

        // Center jewel cap at (15..16, 15..16)
        let jewel_c = if beat { Rgb::WHITE } else { Rgb::new(255, 230, 80) };
        frame.set(15, 15, jewel_c);
        frame.set(16, 15, jewel_c);
        frame.set(15, 16, jewel_c);
        frame.set(16, 16, jewel_c);

        // Treble sparkles on perimeter
        if audio_active && treble > 0.4 {
            for i in 0..4 {
                let ang = (i as f32 * 0.5 * PI) + self.spin_cw;
                let px = (cx + ang.cos() * 14.5).round() as i32;
                let py = (cy + ang.sin() * 14.5).round() as i32;
                if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                    frame.set(px as usize, py as usize, Rgb::WHITE);
                }
            }
        }
    }
}
