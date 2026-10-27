use crate::util::{audio_bass, audio_mid, audio_treble, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Ripe Yellow", "Neon Rave", "Disco Gold", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "Dance tempo",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 45.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "bounce",
        label: "Hip sway bounce",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 6.0,
        step: 1.0,
        default: 3.5,
        options: &[],
        unit: "px",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Banana style",
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

pub struct DancingBananaEffect {
    values: [f32; 4],
    step_time: f32,
    color_cycle: f32,
    spin_boost: f32,
}

impl Default for DancingBananaEffect {
    fn default() -> Self {
        Self {
            values: [45.0, 3.5, 0.0, 1.0],
            step_time: 0.0,
            color_cycle: 0.0,
            spin_boost: 0.0,
        }
    }
}

impl DancingBananaEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn banana_colors(pal: usize, color_cycle: f32) -> (Rgb, Rgb, Rgb) {
        // Returns (peel_c, tip_c, glove_c)
        match pal {
            0 => {
                // Ripe Yellow
                (Rgb::new(255, 230, 20), Rgb::new(60, 35, 10), Rgb::WHITE)
            }
            1 => {
                // Neon Rave
                (Rgb::new(0, 255, 160), Rgb::new(255, 0, 140), Rgb::new(220, 240, 255))
            }
            2 => {
                // Disco Gold
                (Rgb::new(255, 195, 30), Rgb::new(140, 90, 10), Rgb::new(255, 255, 200))
            }
            _ => {
                // Rainbow
                let c1 = hsv_to_rgb(color_cycle, 0.95, 1.0);
                let c2 = hsv_to_rgb((color_cycle + 0.4) % 1.0, 0.9, 0.7);
                (c1, c2, Rgb::WHITE)
            }
        }
    }
}

impl Effect for DancingBananaEffect {
    fn id(&self) -> &'static str {
        "dancing_banana"
    }

    fn name(&self) -> &'static str {
        "Dancing Banana"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "bounce" => Some(self.values[1]),
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
            "bounce" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.step_time = 0.0;
        self.color_cycle = 0.0;
        self.spin_boost = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::new(10, 10, 20)); // Deep stage background

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[3] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.spin_boost = 1.0;
        } else {
            self.spin_boost = (self.spin_boost - dt * 3.0).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.8 + self.spin_boost * 0.7);
        self.step_time += dt * speed * 2.2;
        self.color_cycle = (self.color_cycle + dt * 0.16) % 1.0;

        let pal = self.values[2] as usize;
        let (peel_c, tip_c, glove_c) = Self::banana_colors(pal, self.color_cycle);

        // Stage lights at bottom (y = 29..=31)
        for x in 0..WIDTH {
            #[allow(clippy::manual_is_multiple_of)]
            let stage_on = (x + (self.step_time * 6.0) as usize) % 4 == 0;
            let stage_c = if stage_on {
                Rgb::new(to_byte(180.0 + mid * 75.0), to_byte(30.0 + bass * 150.0), 240)
            } else {
                Rgb::new(20, 15, 30)
            };
            frame.set(x, 30, stage_c);
            frame.set(x, 31, Rgb::new(15, 10, 25));
        }

        // Banana animation kinematics:
        // Lean angle sways sinusoidally left and right
        let sway = (self.step_time * 2.0 * PI).sin();
        let bounce_amp = self.values[1] + bass * 2.5;
        let bounce_y = (self.step_time * 4.0 * PI).sin().abs() * bounce_amp;

        let cx = 15.5 + sway * 3.5;
        let cy = 16.0 - bounce_y;

        // Render curved banana crescent body (y from -11 to +10 relative to cy)
        for dy in -11..=10i32 {
            let py = (cy + dy as f32).round() as i32;
            if py < 0 || py >= HEIGHT as i32 {
                continue;
            }

            let y_norm = dy as f32 / 11.0; // -1.0 (top tip) to +1.0 (bottom tip)
            // Crescent bend: banana curves laterally
            let bend = (1.0 - y_norm * y_norm) * 3.2 * sway.signum();
            let half_w = (1.0 - y_norm.abs().powf(1.8)) * 3.8;

            let is_tip = dy <= -9 || dy >= 8;

            let bx_center = cx + bend;
            let x_start = (bx_center - half_w).round() as i32;
            let x_end = (bx_center + half_w).round() as i32;

            for px in x_start..=x_end {
                if px >= 0 && px < WIDTH as i32 {
                    let col = if is_tip {
                        tip_c
                    } else {
                        // 3D round cylindrical shading
                        let x_rel = (px as f32 - bx_center) / half_w.max(0.5);
                        let shade = (1.0 - x_rel.abs().powi(2) * 0.5).clamp(0.4, 1.0);
                        Rgb::new(
                            to_byte(peel_c.r as f32 * shade),
                            to_byte(peel_c.g as f32 * shade),
                            to_byte(peel_c.b as f32 * shade),
                        )
                    };
                    frame.set(px as usize, py as usize, col);
                }
            }
        }

        // Goofy Eyes and Mouth at (cx, cy - 2)
        let eye_y = (cy - 3.0).round() as i32;
        let eye_x1 = (cx - 1.5).round() as i32;
        let eye_x2 = (cx + 1.5).round() as i32;

        if eye_y >= 0 && eye_y < HEIGHT as i32 {
            if eye_x1 >= 0 && eye_x1 < WIDTH as i32 { frame.set(eye_x1 as usize, eye_y as usize, Rgb::BLACK); }
            if eye_x2 >= 0 && eye_x2 < WIDTH as i32 { frame.set(eye_x2 as usize, eye_y as usize, Rgb::BLACK); }
        }

        let mouth_y = (cy - 1.0).round() as i32;
        let mouth_x = cx.round() as i32;
        if mouth_y >= 0 && mouth_y < HEIGHT as i32 && mouth_x >= 0 && mouth_x < WIDTH as i32 {
            let m_c = if beat { Rgb::new(255, 50, 60) } else { Rgb::BLACK };
            frame.set(mouth_x as usize, mouth_y as usize, m_c);
        }

        // Cartoon White Gloved Arms (alternating up & down in dance posture)
        let arm_l_up = sway > 0.0;
        let l_hand_x = (cx - 6.0).round() as i32;
        let l_hand_y = (cy + if arm_l_up { -6.0 } else { 3.0 }).round() as i32;

        let r_hand_x = (cx + 6.0).round() as i32;
        let r_hand_y = (cy + if !arm_l_up { -6.0 } else { 3.0 }).round() as i32;

        for &(hx, hy) in &[(l_hand_x, l_hand_y), (r_hand_x, r_hand_y)] {
            for dy in -1..=1i32 {
                for dx in -1..=1i32 {
                    let px = hx + dx;
                    let py = hy + dy;
                    if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                        frame.set(px as usize, py as usize, glove_c);
                    }
                }
            }
        }

        // Dancing Feet at (cx - 3, cy + 11) and (cx + 3, cy + 11)
        let foot_y = 28i32;
        let l_foot_x = (cx - 3.0 + if sway < 0.0 { -2.0 } else { 0.0 }).round() as i32;
        let r_foot_x = (cx + 3.0 + if sway > 0.0 { 2.0 } else { 0.0 }).round() as i32;

        for &fx in &[l_foot_x, r_foot_x] {
            if fx >= 0 && fx + 1 < WIDTH as i32 && foot_y < HEIGHT as i32 {
                frame.set(fx as usize, foot_y as usize, tip_c);
                frame.set((fx + 1) as usize, foot_y as usize, tip_c);
            }
        }

        // Maraca sparkles on treble
        if audio_active && treble > 0.35 {
            let s_c = Rgb::new(255, 240, 100);
            if l_hand_x > 0 && l_hand_y > 0 { frame.set((l_hand_x - 1) as usize, (l_hand_y - 1) as usize, s_c); }
            if r_hand_x + 1 < WIDTH as i32 && r_hand_y > 0 { frame.set((r_hand_x + 1) as usize, (r_hand_y - 1) as usize, s_c); }
        }
    }
}
