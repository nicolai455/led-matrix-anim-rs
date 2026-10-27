use crate::util::{audio_bass, audio_mid, audio_treble, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Classic Shiba", "Black & Tan", "Cyber Doge", "Rainbow Doge"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Bob tempo",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 40.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "sunglasses",
        label: "MLG sunglasses",
        kind: ParameterKind::Boolean,
        min: 0.0,
        max: 1.0,
        step: 1.0,
        default: 1.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "meme_words",
        label: "Floating meme text",
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
        label: "Fur palette",
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

pub struct DogeWowEffect {
    values: [f32; 5],
    timer: f32,
    glasses_y: f32,
    glasses_landed: bool,
    color_cycle: f32,
    word_index: usize,
    word_timer: f32,
    flare_pulse: f32,
}

impl Default for DogeWowEffect {
    fn default() -> Self {
        Self {
            values: [40.0, 1.0, 1.0, 0.0, 1.0],
            timer: 0.0,
            glasses_y: -8.0,
            glasses_landed: false,
            color_cycle: 0.0,
            word_index: 0,
            word_timer: 0.0,
            flare_pulse: 0.0,
        }
    }
}

impl DogeWowEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn shiba_colors(pal: usize, color_cycle: f32) -> (Rgb, Rgb, Rgb) {
        // (Fur, WhiteMuzzle, EarInner)
        match pal {
            0 => (
                Rgb::new(235, 175, 80),  // Warm Golden Tan
                Rgb::new(255, 250, 240), // Cream White
                Rgb::new(210, 140, 60),
            ),
            1 => (
                Rgb::new(45, 40, 45),    // Black & Tan Shiba
                Rgb::new(240, 230, 215),
                Rgb::new(190, 140, 70),
            ),
            2 => (
                Rgb::new(0, 230, 255),   // Cyber Cyan
                Rgb::new(245, 250, 255),
                Rgb::new(255, 20, 140),
            ),
            _ => (
                hsv_to_rgb(color_cycle, 0.8, 1.0),
                Rgb::WHITE,
                hsv_to_rgb((color_cycle + 0.3) % 1.0, 0.9, 0.9),
            ),
        }
    }

    fn draw_3x5_char(frame: &mut Frame, ch: char, x: i32, y: i32, col: Rgb) {
        // 3x5 compact glyph bitmap
        let glyph: u16 = match ch {
            'w' | 'W' => 0b101_101_101_111_101,
            'o' | 'O' => 0b111_101_101_101_111,
            's' | 'S' => 0b111_100_111_001_111,
            'm' | 'M' => 0b101_111_101_101_101,
            'u' | 'U' => 0b101_101_101_101_111,
            'c' | 'C' => 0b111_100_100_100_111,
            'h' | 'H' => 0b101_101_111_101_101,
            '!' => 0b010_010_010_000_010,
            _ => 0b000_000_000_000_000,
        };

        for row in 0..5i32 {
            for col_idx in 0..3i32 {
                let bit_idx = (4 - row) * 3 + (2 - col_idx);
                if (glyph & (1 << bit_idx)) != 0 {
                    let px = x + col_idx;
                    let py = y + row;
                    if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                        frame.set(px as usize, py as usize, col);
                    }
                }
            }
        }
    }

    fn draw_word(frame: &mut Frame, text: &str, start_x: i32, start_y: i32, col: Rgb) {
        let mut cur_x = start_x;
        for c in text.chars() {
            Self::draw_3x5_char(frame, c, cur_x, start_y, col);
            cur_x += 4;
        }
    }
}

impl Effect for DogeWowEffect {
    fn id(&self) -> &'static str {
        "doge_wow"
    }

    fn name(&self) -> &'static str {
        "Doge Much Wow"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "sunglasses" => Some(self.values[1]),
            "meme_words" => Some(self.values[2]),
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
            "sunglasses" => self.values[1] = value,
            "meme_words" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.timer = 0.0;
        self.glasses_y = -8.0;
        self.glasses_landed = false;
        self.color_cycle = 0.0;
        self.word_index = 0;
        self.word_timer = 0.0;
        self.flare_pulse = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        // Deep purple comic background
        frame.clear(Rgb::new(20, 15, 35));

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[4] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let _mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.flare_pulse = 1.0;
            self.word_timer = 1.5; // Trigger word change
            self.word_index = (self.word_index + 1) % 4;
        } else {
            self.flare_pulse = (self.flare_pulse - dt * 3.5).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.8);
        self.timer = (self.timer + dt * speed * 2.2) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        self.word_timer += dt;
        if self.word_timer > 2.0 {
            self.word_timer = 0.0;
            self.word_index = (self.word_index + 1) % 4;
        }

        let pal = self.values[3] as usize;
        let (fur_c, muzzle_c, ear_c) = Self::shiba_colors(pal, self.color_cycle);

        // Head bobbing & tilting kinematics
        let _head_tilt = (self.timer).sin() * 0.12;
        let head_bob = (self.timer * 2.0).cos().abs() * (1.5 + bass * 2.0);

        let cx = 15.5f32;
        let cy = 16.5f32 + head_bob;

        // 1. Pointy Shiba Ears
        // Left Ear (tipped back/left)
        let l_ear_poly = [
            (cx - 10.0, cy - 12.0),
            (cx - 9.0, cy - 11.0), (cx - 8.0, cy - 11.0),
            (cx - 11.0, cy - 10.0), (cx - 10.0, cy - 10.0), (cx - 9.0, cy - 10.0), (cx - 8.0, cy - 10.0),
            (cx - 11.0, cy - 9.0), (cx - 10.0, cy - 9.0), (cx - 9.0, cy - 9.0), (cx - 8.0, cy - 9.0),
        ];
        for (i, &(ex, ey)) in l_ear_poly.iter().enumerate() {
            let px = ex.round() as i32;
            let py = ey.round() as i32;
            if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                frame.set(px as usize, py as usize, if i == 5 { ear_c } else { fur_c });
            }
        }

        // Right Ear
        let r_ear_poly = [
            (cx + 9.0, cy - 12.0),
            (cx + 8.0, cy - 11.0), (cx + 9.0, cy - 11.0),
            (cx + 7.0, cy - 10.0), (cx + 8.0, cy - 10.0), (cx + 9.0, cy - 10.0), (cx + 10.0, cy - 10.0),
            (cx + 7.0, cy - 9.0), (cx + 8.0, cy - 9.0), (cx + 9.0, cy - 9.0), (cx + 10.0, cy - 9.0),
        ];
        for (i, &(ex, ey)) in r_ear_poly.iter().enumerate() {
            let px = ex.round() as i32;
            let py = ey.round() as i32;
            if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                frame.set(px as usize, py as usize, if i == 5 { ear_c } else { fur_c });
            }
        }

        // 2. Shiba Round Cheeks & Head (Ellipse)
        for dy in -9..=9i32 {
            let py = (cy + dy as f32).round() as i32;
            if py < 0 || py >= HEIGHT as i32 {
                continue;
            }
            let y_rel = dy as f32 / 9.5;
            let half_w = (1.0 - y_rel * y_rel).max(0.0).sqrt() * 11.0;
            let hw = half_w.round() as i32;

            for dx in -hw..=hw {
                let px = (cx + dx as f32).round() as i32;
                if px >= 0 && px < WIDTH as i32 {
                    // White muzzle & cheek urajiro patches (bottom half and outer cheeks)
                    let is_muzzle = (dy >= 0 && (dx as f32).abs() < 5.0)
                        || (dy >= 2 && (dx as f32).abs() < 9.0)
                        || (dy == -1 && (dx as f32).abs() < 3.0);

                    // Cheek fluff highlights
                    let is_eyebrow = (dy == -4 || dy == -5) && ((dx as f32).abs() >= 3.5 && (dx as f32).abs() <= 5.5);

                    let col = if is_eyebrow || is_muzzle {
                        muzzle_c
                    } else {
                        fur_c
                    };
                    frame.set(px as usize, py as usize, col);
                }
            }
        }

        // 3. Cute Black Nose & Mouth
        let nose_y = (cy + 2.0).round() as usize;
        let nose_x = cx.round() as usize;
        if nose_y < HEIGHT && nose_x < WIDTH {
            frame.set(nose_x, nose_y, Rgb::BLACK);
            frame.set(nose_x - 1, nose_y, Rgb::BLACK);
            frame.set(nose_x, nose_y + 1, Rgb::new(60, 40, 40));
        }

        // 4. Shiba Eyes (or covered by MLG sunglasses)
        let eye_level_y = cy - 2.0;
        let l_eye_x = (cx - 4.5).round() as usize;
        let r_eye_x = (cx + 4.5).round() as usize;

        // Render natural black almond eyes
        for dy in 0..=1 {
            for dx in 0..=1 {
                if (eye_level_y.round() as usize + dy) < HEIGHT {
                    frame.set(l_eye_x + dx, eye_level_y.round() as usize + dy, Rgb::BLACK);
                    frame.set(r_eye_x + dx, eye_level_y.round() as usize + dy, Rgb::BLACK);
                }
            }
        }
        // White twinkle pupil dot
        if (eye_level_y.round() as usize) < HEIGHT {
            frame.set(l_eye_x, eye_level_y.round() as usize, Rgb::WHITE);
            frame.set(r_eye_x, eye_level_y.round() as usize, Rgb::WHITE);
        }

        // 5. MLG 8-Bit Pixel Sunglasses animation:
        let show_glasses = self.values[1] > 0.5;
        if show_glasses {
            // Drop sunglasses down from sky
            let target_y = eye_level_y;
            if self.glasses_y < target_y {
                self.glasses_y += dt * 25.0;
                if self.glasses_y >= target_y {
                    self.glasses_y = target_y;
                    self.glasses_landed = true;
                }
            } else {
                self.glasses_y = target_y;
            }

            // Draw 8-bit shades (Width: 17, Height: 4)
            let gy = self.glasses_y.round() as i32;
            let gx_start = (cx - 8.5).round() as i32;

            for dy in 0..4i32 {
                let py = gy + dy;
                if py < 0 || py >= HEIGHT as i32 {
                    continue;
                }
                for dx in 0..17i32 {
                    let px = gx_start + dx;
                    if px < 0 || px >= WIDTH as i32 {
                        continue;
                    }

                    // Bridge in middle (dx == 8)
                    let is_bridge = dx == 8 && dy == 0;
                    // Lens left (dx: 1..7), Lens right (dx: 9..15)
                    let is_left_lens = (1..=7).contains(&dx);
                    let is_right_lens = (9..=15).contains(&dx);

                    if is_bridge || is_left_lens || is_right_lens {
                        // MLG white diagonal reflection stripe
                        let is_white_stripe = (dx == 2 && dy == 1)
                            || (dx == 3 && dy == 2)
                            || (dx == 4 && dy == 3)
                            || (dx == 10 && dy == 1)
                            || (dx == 11 && dy == 2)
                            || (dx == 12 && dy == 3);

                        let c = if is_white_stripe {
                            Rgb::WHITE
                        } else {
                            Rgb::new(10, 10, 15) // Matte Black
                        };
                        frame.set(px as usize, py as usize, c);
                    }
                }
            }

            // Treble / Beat lens flare sparkle!
            if self.flare_pulse > 0.1 || treble > 0.5 {
                let flare_x = (gx_start + 2) as usize;
                let flare_y = (gy + 1) as usize;
                if flare_x < WIDTH && flare_y < HEIGHT {
                    frame.set(flare_x, flare_y, Rgb::new(255, 255, 120));
                    if flare_x + 1 < WIDTH { frame.set(flare_x + 1, flare_y, Rgb::WHITE); }
                    if flare_y > 0 { frame.set(flare_x, flare_y - 1, Rgb::WHITE); }
                }
            }
        }

        // 6. Floating Meme Words (Comic Doge phrases)
        if self.values[2] > 0.5 {
            let phrases = [
                ("WOW", 18, 3, Rgb::new(255, 230, 40)),     // Golden Yellow
                ("MUCH", 2, 4, Rgb::new(255, 60, 180)),    // Neon Magenta
                ("SO", 21, 25, Rgb::new(40, 240, 255)),    // Electric Cyan
                ("SUCH", 2, 25, Rgb::new(80, 255, 120)),   // Lime Green
            ];

            let (word, wx, wy, wcol) = phrases[self.word_index];
            Self::draw_word(frame, word, wx, wy, wcol);
        }

        // Beat border flash
        if self.flare_pulse > 0.2 {
            let flash_c = to_byte(self.flare_pulse * 100.0);
            frame.set(0, 0, Rgb::new(flash_c, flash_c, flash_c));
            frame.set(31, 0, Rgb::new(flash_c, flash_c, flash_c));
            frame.set(0, 31, Rgb::new(flash_c, flash_c, flash_c));
            frame.set(31, 31, Rgb::new(flash_c, flash_c, flash_c));
        }
    }
}
