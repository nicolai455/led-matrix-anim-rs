use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const PALETTES: &[&str] = &["Classic Oat", "Tuxedo Cat", "Neon Cyber", "Rainbow Pop"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "Pop rate",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 40.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "shockwave",
        label: "Pop shockwave",
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
        label: "Cat style",
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

pub struct PopcatEffect {
    values: [f32; 4],
    timer: f32,
    is_popping: bool,
    pop_decay: f32,
    shockwave_radius: f32,
    pop_count: u32,
    color_cycle: f32,
}

impl Default for PopcatEffect {
    fn default() -> Self {
        Self {
            values: [40.0, 1.0, 0.0, 1.0],
            timer: 0.0,
            is_popping: false,
            pop_decay: 0.0,
            shockwave_radius: 0.0,
            pop_count: 0,
            color_cycle: 0.0,
        }
    }
}

impl PopcatEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn cat_colors(pal: usize, color_cycle: f32) -> (Rgb, Rgb, Rgb) {
        // (Fur, InnerEar/Mouth, Outline)
        match pal {
            0 => (
                Rgb::new(245, 235, 220), // Oat cream white
                Rgb::new(240, 130, 150), // Pink mouth/ears
                Rgb::new(50, 45, 40),
            ),
            1 => (
                Rgb::new(35, 35, 45),    // Tuxedo dark
                Rgb::new(255, 150, 180),
                Rgb::new(15, 15, 20),
            ),
            2 => (
                Rgb::new(20, 240, 200),  // Neon Cyan
                Rgb::new(255, 30, 140),  // Neon Pink
                Rgb::new(10, 20, 40),
            ),
            _ => (
                hsv_to_rgb(color_cycle, 0.75, 1.0),
                Rgb::new(255, 255, 255),
                Rgb::new(25, 20, 35),
            ),
        }
    }
}

impl Effect for PopcatEffect {
    fn id(&self) -> &'static str {
        "popcat"
    }

    fn name(&self) -> &'static str {
        "Popcat"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "shockwave" => Some(self.values[1]),
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
            "shockwave" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.timer = 0.0;
        self.is_popping = false;
        self.pop_decay = 0.0;
        self.shockwave_radius = 0.0;
        self.pop_count = 0;
        self.color_cycle = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::new(18, 16, 32)); // Warm cozy room background

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[3] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.7);
        self.timer += dt * speed * 2.5;
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        // Determine POP trigger
        let should_pop = if audio_active {
            beat || bass > 0.55 || (context.audio.level > 0.6)
        } else {
            (self.timer % 2.0) < 0.8
        };

        if should_pop && !self.is_popping {
            self.pop_decay = 1.0;
            self.shockwave_radius = 2.0;
            self.pop_count = self.pop_count.wrapping_add(1);
        }
        self.is_popping = should_pop;

        if self.pop_decay > 0.0 {
            self.pop_decay = (self.pop_decay - dt * 4.0).max(0.0);
            self.shockwave_radius += dt * 38.0;
        }

        let pal = self.values[2] as usize;
        let (fur_c, mouth_c, out_c) = Self::cat_colors(pal, self.color_cycle);

        // 1. Pop Shockwave ripple if active
        if self.values[1] > 0.5 && self.shockwave_radius > 0.0 && self.shockwave_radius < 30.0 {
            let mouth_center_x = 15.5f32;
            let mouth_center_y = 21.0f32;
            let r_wave = self.shockwave_radius;

            for y in 0..HEIGHT {
                let dy = y as f32 - mouth_center_y;
                for x in 0..WIDTH {
                    let dx = x as f32 - mouth_center_x;
                    let d = (dx * dx + dy * dy).sqrt();
                    let diff = (d - r_wave).abs();
                    if diff < 1.2 {
                        let bright = clamp01((1.2 - diff) / 1.2 * (1.0 - r_wave / 30.0));
                        let ripple_c = Rgb::new(
                            to_byte(bright * 240.0),
                            to_byte(bright * 200.0),
                            to_byte(bright * 100.0),
                        );
                        let cur = frame.get(x, y);
                        frame.set(
                            x,
                            y,
                            Rgb::new(
                                cur.r.saturating_add(ripple_c.r),
                                cur.g.saturating_add(ripple_c.g),
                                cur.b.saturating_add(ripple_c.b),
                            ),
                        );
                    }
                }
            }
        }

        // Cat dimensions & Head position (recoils down-back when popping)
        let head_y_offset = if self.is_popping { 1.0f32 } else { 0.0f32 };
        let head_cx = 15.5f32;
        let head_cy = 15.5f32 + head_y_offset;

        // 2. Ears
        // Left Ear
        for dy in -9..=-4i32 {
            let py = (head_cy + dy as f32).round() as i32;
            if py >= 0 && py < HEIGHT as i32 {
                for dx in -8..=-3i32 {
                    let px = (head_cx + dx as f32).round() as i32;
                    if px >= 0 && px < WIDTH as i32 {
                        let is_inner = dx == -6 && dy == -6;
                        let c = if is_inner { mouth_c } else { fur_c };
                        frame.set(px as usize, py as usize, c);
                    }
                }
            }
        }
        // Right Ear
        for dy in -9..=-4i32 {
            let py = (head_cy + dy as f32).round() as i32;
            if py >= 0 && py < HEIGHT as i32 {
                for dx in 3..=8i32 {
                    let px = (head_cx + dx as f32).round() as i32;
                    if px >= 0 && px < WIDTH as i32 {
                        let is_inner = dx == 6 && dy == -6;
                        let c = if is_inner { mouth_c } else { fur_c };
                        frame.set(px as usize, py as usize, c);
                    }
                }
            }
        }

        // 3. Cat Head & Body (Spherical curve)
        for dy in -5..=15i32 {
            let py = (head_cy + dy as f32).round() as i32;
            if py < 0 || py >= HEIGHT as i32 {
                continue;
            }
            let half_w = if dy < 6 {
                (9.0f32 * 9.0 - (dy as f32).powi(2)).max(0.0).sqrt()
            } else {
                9.5f32 + (dy - 6) as f32 * 0.4
            };

            let hw = half_w.round() as i32;
            for dx in -hw..=hw {
                let px = (head_cx + dx as f32).round() as i32;
                if px >= 0 && px < WIDTH as i32 {
                    let is_outline = (dx as f32).abs() >= (half_w - 0.9);
                    let c = if is_outline { out_c } else { fur_c };
                    frame.set(px as usize, py as usize, c);
                }
            }
        }

        // 4. Eyes:
        // Closed / Normal: big black round pupils
        // Popping: squinted or wide staring eyes
        let eye_y = (head_cy - 1.5).round() as usize;
        let l_eye_x = 10usize;
        let r_eye_x = 21usize;

        if !self.is_popping {
            // Big open curious eyes (2x2)
            for dy in 0..2 {
                for dx in 0..2 {
                    frame.set(l_eye_x + dx, eye_y + dy, Rgb::BLACK);
                    frame.set(r_eye_x + dx, eye_y + dy, Rgb::BLACK);
                }
            }
            // Catchlight dot
            frame.set(l_eye_x, eye_y, Rgb::WHITE);
            frame.set(r_eye_x, eye_y, Rgb::WHITE);
        } else {
            // Squinted enthusiastic eyes (^ ^)
            frame.set(l_eye_x - 1, eye_y + 1, Rgb::BLACK);
            frame.set(l_eye_x, eye_y, Rgb::BLACK);
            frame.set(l_eye_x + 1, eye_y + 1, Rgb::BLACK);

            frame.set(r_eye_x - 1, eye_y + 1, Rgb::BLACK);
            frame.set(r_eye_x, eye_y, Rgb::BLACK);
            frame.set(r_eye_x + 1, eye_y + 1, Rgb::BLACK);
        }

        // 5. Nose & Mouth:
        let nose_y = (head_cy + 1.0).round() as usize;
        frame.set(15, nose_y, mouth_c);
        frame.set(16, nose_y, mouth_c);

        if !self.is_popping {
            // Closed tiny :3 mouth line
            let mouth_y = nose_y + 2;
            frame.set(14, mouth_y, out_c);
            frame.set(15, mouth_y + 1, out_c);
            frame.set(16, mouth_y + 1, out_c);
            frame.set(17, mouth_y, out_c);
        } else {
            // Huge wide "O" mouth!
            let mouth_center_x = 15.5f32;
            let mouth_center_y = (head_cy + 5.5).round();
            let mouth_rad_x = 5.0f32;
            let mouth_rad_y = 4.2f32;

            for dy in -4..=4i32 {
                let py = (mouth_center_y + dy as f32).round() as i32;
                if py >= 0 && py < HEIGHT as i32 {
                    for dx in -5..=5i32 {
                        let px = (mouth_center_x + dx as f32).round() as i32;
                        if px >= 0 && px < WIDTH as i32 {
                            let nx = dx as f32 / mouth_rad_x;
                            let ny = dy as f32 / mouth_rad_y;
                            let d2 = nx * nx + ny * ny;
                            if d2 <= 1.0 {
                                let c = if d2 > 0.75 {
                                    out_c // Dark mouth lip border
                                } else if dy >= 1 {
                                    mouth_c // Pink tongue
                                } else {
                                    Rgb::new(20, 10, 15) // Deep mouth throat
                                };
                                frame.set(px as usize, py as usize, c);
                            }
                        }
                    }
                }
            }
        }

        // 6. Audio sparkles on treble
        if audio_active && treble > 0.4 {
            for i in 0..3 {
                let sx = (i * 11 + 5) % WIDTH;
                let sy = (i * 7 + 2) % 10;
                frame.set(sx, sy, Rgb::new(255, 240, 150));
            }
        }
        if audio_active && mid > 0.3 {
            frame.set(2, 2, Rgb::new(80, 180, 255));
        }
    }
}
