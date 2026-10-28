use crate::util::{audio_bass, audio_mid, audio_treble, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Classic Yellow", "Evil Purple", "Cyber Neon", "Rainbow Minion"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "Dance speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 45.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "two_eyes",
        label: "Two eyes (Bob)",
        kind: ParameterKind::Boolean,
        min: 0.0,
        max: 1.0,
        step: 1.0,
        default: 0.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Minion style",
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

pub struct PartyMinionEffect {
    values: [f32; 4],
    timer: f32,
    cheer_boost: f32,
    color_cycle: f32,
}

impl Default for PartyMinionEffect {
    fn default() -> Self {
        Self {
            values: [45.0, 0.0, 0.0, 1.0],
            timer: 0.0,
            cheer_boost: 0.0,
            color_cycle: 0.0,
        }
    }
}

impl PartyMinionEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn minion_colors(pal: usize, color_cycle: f32) -> (Rgb, Rgb, Rgb, Rgb) {
        // (Skin, Overalls, GoggleRim, GloveBoot)
        match pal {
            0 => (
                Rgb::new(255, 220, 25),  // Minion Yellow
                Rgb::new(30, 85, 195),   // Denim Blue
                Rgb::new(175, 185, 200), // Silver Goggles
                Rgb::new(25, 25, 30),    // Black leather
            ),
            1 => (
                Rgb::new(140, 40, 200),  // Evil Minion Purple
                Rgb::new(35, 30, 45),
                Rgb::new(140, 140, 150),
                Rgb::new(15, 15, 20),
            ),
            2 => (
                Rgb::new(0, 255, 180),   // Cyber Neon
                Rgb::new(255, 20, 150),
                Rgb::new(240, 250, 255),
                Rgb::new(20, 10, 40),
            ),
            _ => (
                hsv_to_rgb(color_cycle, 0.9, 1.0),
                hsv_to_rgb((color_cycle + 0.4) % 1.0, 0.9, 0.9),
                Rgb::new(200, 210, 230),
                Rgb::new(20, 20, 25),
            ),
        }
    }
}

impl Effect for PartyMinionEffect {
    fn id(&self) -> &'static str {
        "party_minion"
    }

    fn name(&self) -> &'static str {
        "Party Minion"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "two_eyes" => Some(self.values[1]),
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
            "two_eyes" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.timer = 0.0;
        self.cheer_boost = 0.0;
        self.color_cycle = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::new(15, 12, 28)); // Dance hall background

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[3] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.cheer_boost = 1.0;
        } else {
            self.cheer_boost = (self.cheer_boost - dt * 3.0).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.8);
        self.timer = (self.timer + dt * speed * 2.2) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let pal = self.values[2] as usize;
        let two_eyed = self.values[1] > 0.5;
        let (skin_c, overall_c, goggle_c, black_c) = Self::minion_colors(pal, self.color_cycle);

        // 1. Stage Lighting Floor (y: 28..31)
        for x in 0..WIDTH {
            #[allow(clippy::manual_is_multiple_of)]
            let stage_on = (x + (self.timer * 7.0) as usize) % 4 == 0;
            let c = if stage_on {
                Rgb::new(to_byte(200.0 + mid * 55.0), to_byte(30.0 + bass * 180.0), 240)
            } else {
                Rgb::new(25, 20, 40)
            };
            frame.set(x, 28, c);
            frame.set(x, 29, Rgb::new(20, 15, 30));
            frame.set(x, 30, Rgb::new(15, 10, 25));
            frame.set(x, 31, Rgb::new(10, 8, 20));
        }

        // Kinematics: Hip sway & bounce
        let sway = (self.timer).sin() * 2.8;
        let bounce = (self.timer * 2.0).sin().abs() * (1.8 + bass * 2.0) + self.cheer_boost * 2.0;

        let cx = 15.5f32 + sway;
        let cy = 15.0f32 - bounce;

        // 2. Hair Strands on top of head
        let hair_top = (cy - 10.0).round() as i32;
        if hair_top >= 0 && hair_top < HEIGHT as i32 {
            let hx = cx.round() as usize;
            if hx < WIDTH { frame.set(hx, hair_top as usize, black_c); }
            if hx > 0 { frame.set(hx - 1, (hair_top + 1) as usize, black_c); }
            if hx + 1 < WIDTH { frame.set(hx + 1, (hair_top + 1) as usize, black_c); }
        }

        // 3. Capsule Body (y: -8..+6 relative to cy, radius 6.5)
        for dy in -8..=7i32 {
            let py = (cy + dy as f32).round() as i32;
            if py < 0 || py >= HEIGHT as i32 {
                continue;
            }

            let half_w = if dy < -2 {
                // Top hemisphere dome
                let r_top = 6.2f32;
                let rel = (dy + 2) as f32;
                (r_top * r_top - rel * rel).max(0.0).sqrt()
            } else if dy > 4 {
                // Bottom hemisphere
                let r_bot = 6.2f32;
                let rel = (dy - 4) as f32;
                (r_bot * r_bot - rel * rel).max(0.0).sqrt()
            } else {
                // Straight cylindrical middle
                6.2f32
            };

            let hw = half_w.round() as i32;
            let is_overall = dy >= 2;

            for dx in -hw..=hw {
                let px = (cx + dx as f32).round() as i32;
                if px >= 0 && px < WIDTH as i32 {
                    let mut col = if is_overall { overall_c } else { skin_c };

                    // Overall bib in front (dy: 0..=1, |dx| <= 4)
                    if (dy == 0 || dy == 1) && dx.abs() <= 4 {
                        col = overall_c;
                    }
                    // Overall suspender straps at dy: -2..=0, dx = -4, +4
                    if (-2..=0).contains(&dy) && (dx.abs() == 4 || dx.abs() == 3) {
                        col = overall_c;
                    }

                    frame.set(px as usize, py as usize, col);
                }
            }
        }

        // 4. Black Goggle Strap (horizontal band across head at dy = -3)
        let strap_y = (cy - 3.0).round() as i32;
        if strap_y >= 0 && strap_y < HEIGHT as i32 {
            for dx in -6..=6i32 {
                let px = (cx + dx as f32).round() as i32;
                if px >= 0 && px < WIDTH as i32 {
                    frame.set(px as usize, strap_y as usize, black_c);
                }
            }
        }

        // 5. Silver Goggles & Eyes
        if !two_eyed {
            // Big Single Goggle in Center (Stuart)
            let gy = (cy - 3.0).round() as i32;
            let gx = cx.round() as i32;

            for dy in -3..=3i32 {
                let py = gy + dy;
                if py < 0 || py >= HEIGHT as i32 {
                    continue;
                }
                for dx in -3..=3i32 {
                    let px = gx + dx;
                    if px >= 0 && px < WIDTH as i32 {
                        let d2 = dx * dx + dy * dy;
                        if d2 <= 9 {
                            let c = if d2 >= 6 {
                                goggle_c // Metallic silver rim
                            } else if dx == 0 && dy == 0 {
                                black_c  // Pupil
                            } else if dx == 1 && dy == 0 {
                                Rgb::new(130, 70, 20) // Brown iris
                            } else {
                                Rgb::WHITE // Sclera eye white
                            };
                            frame.set(px as usize, py as usize, c);
                        }
                    }
                }
            }
        } else {
            // Two Goggles (Bob)
            for &g_off in &[-2.5f32, 2.5f32] {
                let gx = (cx + g_off).round() as i32;
                let gy = (cy - 3.0).round() as i32;

                for dy in -2..=2i32 {
                    let py = gy + dy;
                    if py < 0 || py >= HEIGHT as i32 {
                        continue;
                    }
                    for dx in -2..=2i32 {
                        let px = gx + dx;
                        if px >= 0 && px < WIDTH as i32 {
                            let d2 = dx * dx + dy * dy;
                            if d2 <= 5 {
                                let c = if d2 >= 4 {
                                    goggle_c
                                } else if dx == 0 && dy == 0 {
                                    black_c
                                } else {
                                    Rgb::WHITE
                                };
                                frame.set(px as usize, py as usize, c);
                            }
                        }
                    }
                }
            }
        }

        // 6. Happy Smile Mouth (dy = 0)
        let mouth_y = cy.round() as i32;
        let mouth_x = cx.round() as i32;
        if mouth_y >= 0 && mouth_y < HEIGHT as i32 {
            if mouth_x > 0 { frame.set((mouth_x - 1) as usize, mouth_y as usize, black_c); }
            frame.set(mouth_x as usize, (mouth_y + 1) as usize, black_c);
            if mouth_x + 1 < WIDTH as i32 { frame.set((mouth_x + 1) as usize, mouth_y as usize, black_c); }
            // Tongue if cheering
            if self.cheer_boost > 0.2 {
                frame.set(mouth_x as usize, mouth_y as usize, Rgb::new(255, 60, 80));
            }
        }

        // 7. Arms & Black Gloves:
        // If cheering or beat: Hands raised high in "Bananaaa!" cheer
        // Else: Cha-cha swinging side to side
        let hands_up = self.cheer_boost > 0.1 || beat;
        let l_glove_x = (cx - 7.5).round() as i32;
        let l_glove_y = (cy + if hands_up { -7.0 } else { 2.0 + sway * 0.8 }).round() as i32;

        let r_glove_x = (cx + 7.5).round() as i32;
        let r_glove_y = (cy + if hands_up { -7.0 } else { 2.0 - sway * 0.8 }).round() as i32;

        for &(gx, gy) in &[(l_glove_x, l_glove_y), (r_glove_x, r_glove_y)] {
            for dy in 0..=1 {
                for dx in 0..=1 {
                    let px = gx + dx;
                    let py = gy + dy;
                    if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                        frame.set(px as usize, py as usize, black_c);
                    }
                }
            }
        }

        // 8. Feet / Boots at (cx - 3, cy + 9) and (cx + 2, cy + 9)
        let foot_y = (cy + 9.0).round() as i32;
        let l_foot_x = (cx - 3.5 + sway * 0.6).round() as i32;
        let r_foot_x = (cx + 2.0 + sway * 0.6).round() as i32;

        for &fx in &[l_foot_x, r_foot_x] {
            if fx >= 0 && fx + 1 < WIDTH as i32 && foot_y >= 0 && foot_y < HEIGHT as i32 {
                frame.set(fx as usize, foot_y as usize, black_c);
                frame.set((fx + 1) as usize, foot_y as usize, black_c);
            }
        }

        // Sparkle party disco particles on treble
        if audio_active && treble > 0.4 {
            for i in 0..4 {
                let sx = (i * 9 + 2) % WIDTH;
                let sy = (i * 6 + 1) % 10;
                frame.set(sx, sy, Rgb::new(255, 255, 120));
            }
        }
    }
}
