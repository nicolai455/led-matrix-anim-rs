use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Fire & Ice", "Solar & Lunar", "Toxic & Plasma", "Rainbow"];

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
        id: "radius",
        label: "Disc radius",
        kind: ParameterKind::Number,
        min: 6.0,
        max: 15.0,
        step: 1.0,
        default: 13.0,
        options: &[],
        unit: "px",
    },
    ParameterDescriptor {
        id: "turbulence",
        label: "Element turbulence",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 10.0,
        step: 1.0,
        default: 5.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Elements",
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

pub struct YinYangEffect {
    values: [f32; 5],
    rotation: f32,
    color_cycle: f32,
    element_pulse: f32,
}

impl Default for YinYangEffect {
    fn default() -> Self {
        Self {
            values: [35.0, 13.0, 5.0, 3.0, 1.0],
            rotation: 0.0,
            color_cycle: 0.0,
            element_pulse: 0.0,
        }
    }
}

impl YinYangEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn element_color(pal: usize, is_yang: bool, intensity: f32, color_cycle: f32) -> Rgb {
        let b = clamp01(intensity);
        match pal {
            0 => {
                // Fire (Yang) & Ice (Yin)
                if is_yang {
                    // Fire: Red / Gold / White
                    let r = to_byte(255.0 * b.sqrt());
                    let g = to_byte(220.0 * b.powi(2) * 0.8);
                    let blue = to_byte(30.0 * b.powi(4));
                    Rgb::new(r, g, blue)
                } else {
                    // Ice: Deep cyan / Marine / Arctic white
                    let r = to_byte(30.0 * b.powi(2));
                    let g = to_byte(210.0 * b);
                    let blue = to_byte(255.0 * b);
                    Rgb::new(r, g, blue)
                }
            }
            1 => {
                // Solar & Lunar (Gold Solar vs Silver Moon)
                if is_yang {
                    Rgb::new(255, to_byte(210.0 * b), to_byte(20.0 * b))
                } else {
                    let silver = to_byte(230.0 * b);
                    Rgb::new(to_byte(silver as f32 * 0.85), silver, to_byte(silver as f32 * 1.1))
                }
            }
            2 => {
                // Toxic & Plasma (Neon Lime vs Electric Magenta)
                if is_yang {
                    Rgb::new(to_byte(50.0 * b), 255, to_byte(70.0 * b))
                } else {
                    Rgb::new(255, to_byte(20.0 * b), to_byte(200.0 * b))
                }
            }
            _ => {
                // Rainbow Dual
                let base_h = (color_cycle + if is_yang { 0.0 } else { 0.5 }) % 1.0;
                hsv_to_rgb(base_h, 0.95, b)
            }
        }
    }
}

impl Effect for YinYangEffect {
    fn id(&self) -> &'static str {
        "yin_yang"
    }

    fn name(&self) -> &'static str {
        "Fire & Ice Yin-Yang"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "radius" => Some(self.values[1]),
            "turbulence" => Some(self.values[2]),
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
            "turbulence" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.rotation = 0.0;
        self.color_cycle = 0.0;
        self.element_pulse = 0.0;
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
            self.element_pulse = 1.0;
        } else {
            self.element_pulse = (self.element_pulse - dt * 3.0).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + self.element_pulse * 0.9);
        self.rotation = (self.rotation + dt * speed * 2.0) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let pal = self.values[3] as usize;
        let main_r = self.values[1];
        let turb_scale = self.values[2] * 0.2;

        let cx = 15.5f32;
        let cy = 15.5f32;
        let r_sq = main_r * main_r;
        let half_r = main_r * 0.5;
        let half_r_sq = half_r * half_r;
        let dot_r = main_r * 0.18;
        let dot_r_sq = dot_r * dot_r;
        let corona_min_sq = (main_r - 1.4).max(0.0).powi(2);
        let corona_max_sq = (main_r + 1.4).powi(2);

        let cos_rot = self.rotation.cos();
        let sin_rot = self.rotation.sin();

        for y in 0..HEIGHT {
            let dy = y as f32 - cy;
            for x in 0..WIDTH {
                let dx = x as f32 - cx;
                let d2 = dx * dx + dy * dy;

                if d2 <= r_sq {
                    // Rotate local coordinates
                    let u = dx * cos_rot - dy * sin_rot;
                    let v = dx * sin_rot + dy * cos_rot;

                    // Wave turbulent displacement
                    let turb = if audio_active {
                        (u * 0.5 + self.rotation * 2.0).sin() * (v * 0.5).cos() * turb_scale * (bass + treble) * 0.5
                    } else {
                        0.0
                    };

                    let u_eff = u + turb;
                    let v_eff = v + turb;

                    // Upper sub-circle center: (0, -half_r) squared
                    let d_upper_sq = u_eff * u_eff + (v_eff + half_r) * (v_eff + half_r);
                    // Lower sub-circle center: (0, half_r) squared
                    let d_lower_sq = u_eff * u_eff + (v_eff - half_r) * (v_eff - half_r);

                    // Classic Yin-Yang boolean classification
                    let in_yang_base = u_eff > 0.0;
                    let in_yang = if d_upper_sq <= half_r_sq {
                        true
                    } else if d_lower_sq <= half_r_sq {
                        false
                    } else {
                        in_yang_base
                    };

                    // Circular eye dots in each half
                    let is_yang_dot = d_lower_sq <= dot_r_sq;
                    let is_yin_dot = d_upper_sq <= dot_r_sq;

                    let final_yang = if is_yang_dot {
                        true
                    } else if is_yin_dot {
                        false
                    } else {
                        in_yang
                    };

                    // Compute elemental intensity texture
                    let element_shimmer = if final_yang {
                        // Fire half: boosted by bass & upward flame noise
                        let flame_noise = (u * 1.5 + v * 2.0 - self.rotation * 4.0).sin() * 0.2;
                        (0.8 + 0.2 * flame_noise + bass * 0.4 + self.element_pulse * 0.3).clamp(0.0, 1.0)
                    } else {
                        // Ice half: crystalline fracture lines boosted by treble
                        let ice_crystal = (u * 3.0 - v * 3.0).abs() % 1.5 < 0.3;
                        let crys_boost = if ice_crystal { 0.35 + treble * 0.4 } else { 0.0 };
                        (0.65 + crys_boost + mid * 0.2).clamp(0.0, 1.0)
                    };

                    let col = Self::element_color(pal, final_yang, element_shimmer, self.color_cycle);
                    frame.set(x, y, col);
                } else if (corona_min_sq..=corona_max_sq).contains(&d2) {
                    // Elemental corona rim
                    let rim_t = (d2.sqrt() - main_r).abs() / 1.4;
                    let rim_c = to_byte((1.0 - rim_t) * (180.0 + self.element_pulse * 75.0));
                    frame.set(x, y, Rgb::new(rim_c, to_byte(rim_c as f32 * 0.8), to_byte(rim_c as f32 * 0.5)));
                }
            }
        }
    }
}
