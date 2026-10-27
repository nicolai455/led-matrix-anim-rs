use crate::util::{audio_bass, audio_mid, audio_treble, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Rubber Ducky", "Mallard Drake", "Cyber Neon", "Rainbow Disco"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "Skate speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 45.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "spin_power",
        label: "Pirouette spin",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 5.0,
        step: 0.5,
        default: 3.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Plumage",
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

pub struct PartyDuckEffect {
    values: [f32; 4],
    timer: f32,
    spin_angle: f32,
    spin_boost: f32,
    quack_open: f32,
    color_cycle: f32,
}

impl Default for PartyDuckEffect {
    fn default() -> Self {
        Self {
            values: [45.0, 3.0, 0.0, 1.0],
            timer: 0.0,
            spin_angle: 0.0,
            spin_boost: 0.0,
            quack_open: 0.0,
            color_cycle: 0.0,
        }
    }
}

impl PartyDuckEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn duck_colors(pal: usize, color_cycle: f32) -> (Rgb, Rgb, Rgb, Rgb) {
        // (BodyYellow, BeakOrange, CapColor, SkateColor)
        match pal {
            0 => (
                Rgb::new(255, 220, 20),  // Bright Yellow
                Rgb::new(255, 110, 10),  // Orange Beak
                Rgb::new(40, 100, 230),  // Blue Cap
                Rgb::new(230, 30, 40),   // Red Skates
            ),
            1 => (
                Rgb::new(20, 160, 90),   // Mallard Emerald
                Rgb::new(245, 180, 20),
                Rgb::new(120, 70, 30),
                Rgb::new(20, 100, 220),
            ),
            2 => (
                Rgb::new(0, 240, 255),   // Cyber Cyan
                Rgb::new(255, 20, 160),  // Hot Magenta
                Rgb::new(140, 40, 255),
                Rgb::new(0, 255, 140),
            ),
            _ => (
                hsv_to_rgb(color_cycle, 0.9, 1.0),
                Rgb::new(255, 130, 20),
                hsv_to_rgb((color_cycle + 0.3) % 1.0, 0.9, 1.0),
                hsv_to_rgb((color_cycle + 0.6) % 1.0, 0.9, 1.0),
            ),
        }
    }
}

impl Effect for PartyDuckEffect {
    fn id(&self) -> &'static str {
        "party_duck"
    }

    fn name(&self) -> &'static str {
        "Party Duck"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "spin_power" => Some(self.values[1]),
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
            "spin_power" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.timer = 0.0;
        self.spin_angle = 0.0;
        self.spin_boost = 0.0;
        self.quack_open = 0.0;
        self.color_cycle = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        // Roller disco arena floor & background
        frame.clear(Rgb::new(14, 10, 28));

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[3] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.spin_boost = 1.0;
            self.quack_open = 1.0;
        } else {
            self.spin_boost = (self.spin_boost - dt * 2.5).max(0.0);
            self.quack_open = (self.quack_open - dt * 4.0).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.8);
        self.timer = (self.timer + dt * speed * 2.2) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let spin_rate = self.values[1] * (if self.spin_boost > 0.1 { 7.0 } else { 1.5 });
        self.spin_angle = (self.spin_angle + dt * spin_rate) % (2.0 * PI);

        let pal = self.values[2] as usize;
        let (body_c, beak_c, cap_c, skate_c) = Self::duck_colors(pal, self.color_cycle);

        // 1. Roller Disco Rink Grid (y: 25..31)
        for y in 25..HEIGHT {
            for x in 0..WIDTH {
                #[allow(clippy::manual_is_multiple_of)]
                let grid = (x + y * 2 + (self.timer * 6.0) as usize) % 5 == 0;
                let c = if grid {
                    Rgb::new(to_byte(120.0 + mid * 80.0), 30, to_byte(180.0 + bass * 75.0))
                } else {
                    Rgb::new(20, 15, 35)
                };
                frame.set(x, y, c);
            }
        }

        // Kinematics: Skate gliding left to right + headbanging bob
        let glide_x = (self.timer).sin() * 4.0;
        let headbob_y = (self.timer * 2.0).cos().abs() * (1.5 + bass * 2.0);

        let duck_cx = 15.5f32 + glide_x;
        let duck_cy = 14.5f32 - headbob_y;

        // Projection width factor from spin angle (turns 360 smoothly)
        let facing_x = self.spin_angle.cos(); // -1 (facing left) to +1 (facing right)
        let facing_dir = if facing_x >= 0.0 { 1.0f32 } else { -1.0f32 };
        let facing_scale = facing_x.abs().max(0.2);

        // 2. Duck Round Plump Body (cx: duck_cx, cy: duck_cy + 3)
        let body_cy = duck_cy + 3.0;
        for dy in -3..=4i32 {
            let py = (body_cy + dy as f32).round() as i32;
            if py < 0 || py >= HEIGHT as i32 {
                continue;
            }
            let y_norm = dy as f32 / 4.0;
            let half_w = (1.0 - y_norm * y_norm).max(0.0).sqrt() * 7.0 * (0.7 + 0.3 * facing_scale);
            let hw = half_w.round() as i32;

            for dx in -hw..=hw {
                let px = (duck_cx + dx as f32).round() as i32;
                if px >= 0 && px < WIDTH as i32 {
                    // Wing feathers highlight
                    let is_wing = (dx as f32 * facing_dir < 0.0) && dy.abs() <= 1;
                    let col = if is_wing {
                        Rgb::new(
                            to_byte(body_c.r as f32 * 0.85),
                            to_byte(body_c.g as f32 * 0.85),
                            to_byte(body_c.b as f32 * 0.85),
                        )
                    } else {
                        body_c
                    };
                    frame.set(px as usize, py as usize, col);
                }
            }
        }

        // 3. Duck Head (cx: duck_cx + 2.0 * facing_x, cy: duck_cy - 4)
        let head_cx = duck_cx + 2.0 * facing_x;
        let head_cy = duck_cy - 4.0;

        for dy in -4..=2i32 {
            let py = (head_cy + dy as f32).round() as i32;
            if py < 0 || py >= HEIGHT as i32 {
                continue;
            }
            let half_w = match dy {
                -4 => 2,
                -3..=0 => 4,
                _ => 3,
            };

            for dx in -half_w..=half_w {
                let px = (head_cx + dx as f32).round() as i32;
                if px >= 0 && px < WIDTH as i32 {
                    frame.set(px as usize, py as usize, body_c);
                }
            }
        }

        // 4. Cool Backwards Baseball Cap on head (at dy = -4 and -5)
        for dx in -4..=3i32 {
            let px = (head_cx + dx as f32).round() as i32;
            let py = (head_cy - 4.0).round() as i32;
            if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                frame.set(px as usize, py as usize, cap_c);
            }
        }
        // Cap backwards visor brim
        let visor_x = (head_cx - 4.5 * facing_dir).round() as i32;
        let visor_y = (head_cy - 3.0).round() as i32;
        if visor_x >= 0 && visor_x < WIDTH as i32 && visor_y >= 0 && visor_y < HEIGHT as i32 {
            frame.set(visor_x as usize, visor_y as usize, cap_c);
        }

        // 5. Duck Eye (facing direction)
        let eye_x = (head_cx + 1.8 * facing_x).round() as i32;
        let eye_y = (head_cy - 1.0).round() as i32;
        if eye_x >= 0 && eye_x < WIDTH as i32 && eye_y >= 0 && eye_y < HEIGHT as i32 {
            frame.set(eye_x as usize, eye_y as usize, Rgb::BLACK);
            // Twinkle catchlight
            if eye_y > 0 { frame.set(eye_x as usize, (eye_y - 1) as usize, Rgb::WHITE); }
        }

        // 6. Orange Bill / Beak (Quacking animated)
        let beak_start_x = (head_cx + 3.2 * facing_x).round() as i32;
        let beak_y = (head_cy + 0.5).round() as i32;
        let beak_len = (3.5 * facing_scale).round() as i32;

        for bx in 0..=beak_len {
            let px = beak_start_x + bx * (facing_dir as i32);
            if px >= 0 && px < WIDTH as i32 {
                // Top bill
                if beak_y >= 0 && beak_y < HEIGHT as i32 {
                    frame.set(px as usize, beak_y as usize, beak_c);
                }
                // Bottom bill (drops if quacking)
                let bot_y = beak_y + if self.quack_open > 0.2 { 2 } else { 1 };
                if bot_y >= 0 && bot_y < HEIGHT as i32 {
                    frame.set(px as usize, bot_y as usize, beak_c);
                }
            }
        }

        // 7. Roller Skates at feet (duck_cx - 3, duck_cx + 3 at y = 24)
        let skate_y = 23i32;
        for &off_x in &[-3.0f32, 3.0f32] {
            let sx = (duck_cx + off_x).round() as i32;
            if sx >= 0 && sx + 1 < WIDTH as i32 && skate_y < HEIGHT as i32 {
                // Boot shoe
                frame.set(sx as usize, skate_y as usize, skate_c);
                frame.set((sx + 1) as usize, skate_y as usize, skate_c);
                // Roller skate wheels
                if skate_y + 1 < HEIGHT as i32 {
                    frame.set(sx as usize, (skate_y + 1) as usize, Rgb::new(240, 240, 255));
                    frame.set((sx + 1) as usize, (skate_y + 1) as usize, Rgb::new(240, 240, 255));
                }
            }
        }

        // 8. Skate Sparkles & Light Trail on treble
        if audio_active && treble > 0.35 {
            let spark_x = (duck_cx - 4.0 * facing_dir).round() as i32;
            let spark_y = 24i32;
            if spark_x >= 0 && spark_x < WIDTH as i32 && spark_y < HEIGHT as i32 {
                frame.set(spark_x as usize, spark_y as usize, Rgb::new(255, 230, 80));
            }
        }
    }
}
