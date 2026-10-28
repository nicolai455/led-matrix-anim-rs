use crate::util::{audio_bass, audio_mid, hsv_to_rgb, to_byte, XorShift32};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Classic Nyan", "Pastel Rainbow", "Cyber Nyan", "Neon Gold"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "Flight speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 40.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "wave_amp",
        label: "Wave amplitude",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 8.0,
        step: 1.0,
        default: 3.0,
        options: &[],
        unit: "px",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Rainbow style",
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

#[derive(Clone, Copy, Debug)]
struct Star {
    x: f32,
    y: f32,
    speed: f32,
    phase: f32,
}

pub struct NyanCatEffect {
    values: [f32; 4],
    flight_time: f32,
    bounce_y: f32,
    stars: [Star; 16],
    color_cycle: f32,
}

impl Default for NyanCatEffect {
    fn default() -> Self {
        let mut rng = XorShift32::new(424242);
        let mut stars = [Star {
            x: 0.0,
            y: 0.0,
            speed: 1.0,
            phase: 0.0,
        }; 16];

        for star in &mut stars {
            star.x = rng.next_f32() * WIDTH as f32;
            star.y = rng.next_f32() * HEIGHT as f32;
            star.speed = 10.0 + rng.next_f32() * 20.0;
            star.phase = rng.next_f32() * 2.0 * PI;
        }

        Self {
            values: [40.0, 3.0, 0.0, 1.0],
            flight_time: 0.0,
            bounce_y: 0.0,
            stars,
            color_cycle: 0.0,
        }
    }
}

impl NyanCatEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn rainbow_stripe_color(pal: usize, stripe_idx: usize, color_cycle: f32) -> Rgb {
        match pal {
            0 => {
                // Classic Nyan (Red, Orange, Yellow, Green, Blue, Violet)
                match stripe_idx {
                    0 => Rgb::new(255, 0, 0),
                    1 => Rgb::new(255, 153, 0),
                    2 => Rgb::new(255, 255, 0),
                    3 => Rgb::new(51, 255, 0),
                    4 => Rgb::new(0, 153, 255),
                    _ => Rgb::new(102, 51, 255),
                }
            }
            1 => {
                // Pastel Rainbow
                let h = (stripe_idx as f32 / 6.0) % 1.0;
                hsv_to_rgb(h, 0.65, 1.0)
            }
            2 => {
                // Cyber Nyan (Neon Cyan, Electric Magenta, Bright Lime)
                match stripe_idx % 3 {
                    0 => Rgb::new(0, 240, 255),
                    1 => Rgb::new(255, 20, 180),
                    _ => Rgb::new(50, 255, 100),
                }
            }
            _ => {
                // Neon Gold / Rainbow Cycling
                let h = (color_cycle + stripe_idx as f32 * 0.12) % 1.0;
                hsv_to_rgb(h, 0.95, 1.0)
            }
        }
    }
}

impl Effect for NyanCatEffect {
    fn id(&self) -> &'static str {
        "nyan_cat"
    }

    fn name(&self) -> &'static str {
        "Nyan Cat Space Loop"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "wave_amp" => Some(self.values[1]),
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
            "wave_amp" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.flight_time = 0.0;
        self.bounce_y = 0.0;
        self.color_cycle = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        // Deep space navy background: #003366
        frame.clear(Rgb::new(0, 35, 75));

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[3] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.bounce_y = -3.5;
        } else {
            self.bounce_y = (self.bounce_y + dt * 10.0).min(0.0);
        }

        let speed_scale = self.values[0] / 40.0;
        self.flight_time += dt * speed_scale * 10.0;
        self.color_cycle = (self.color_cycle + dt * 0.2) % 1.0;

        // 1. Render Background Scrolling Stars
        for star in &mut self.stars {
            star.x -= star.speed * speed_scale * dt;
            if star.x < 0.0 {
                star.x += WIDTH as f32;
            }
            star.phase = (star.phase + dt * 4.0) % (2.0 * PI);

            let xi = star.x.round() as i32;
            let yi = star.y.round() as i32;

            if xi >= 0 && xi < WIDTH as i32 && yi >= 0 && yi < HEIGHT as i32 {
                let twinkle = 0.5 + 0.5 * star.phase.sin();
                let b = to_byte(255.0 * twinkle);
                frame.set(xi as usize, yi as usize, Rgb::new(b, b, b));

                // 4-point cross sparkle for bright stars
                if twinkle > 0.85 {
                    let dim = Rgb::new(b / 3, b / 3, b / 3);
                    if xi > 0 { frame.set((xi - 1) as usize, yi as usize, dim); }
                    if xi + 1 < WIDTH as i32 { frame.set((xi + 1) as usize, yi as usize, dim); }
                    if yi > 0 { frame.set(xi as usize, (yi - 1) as usize, dim); }
                    if yi + 1 < HEIGHT as i32 { frame.set(xi as usize, (yi + 1) as usize, dim); }
                }
            }
        }

        let pal = self.values[2] as usize;
        let base_amp = self.values[1] + bass * 3.0 + mid * 1.5;

        // Cat Position (Right-Center of matrix)
        let cat_base_x = 16i32;
        let cat_bob = (self.flight_time * 0.8).sin() * 1.5 + self.bounce_y;
        let cat_y = (13.0 + cat_bob).round() as i32;

        // 2. Render Undulating Rainbow Trail (Left edge x=0 up to cat_base_x)
        for x in 0..=(cat_base_x as usize) {
            let xf = x as f32;
            let wave_offset = ((xf * 0.45 - self.flight_time * 0.8).sin() * base_amp).round() as i32;
            let rainbow_top = cat_y + 1 + wave_offset;

            for stripe in 0..6 {
                let ry = rainbow_top + stripe as i32;
                if ry >= 0 && ry < HEIGHT as i32 {
                    let col = Self::rainbow_stripe_color(pal, stripe, self.color_cycle);
                    frame.set(x, ry as usize, col);
                }
            }
        }

        // 3. Render 32x32 Pixel Art Nyan Cat
        // Leg & tail running animation frame (2 frames alternating at high cadence)
        let run_frame = (self.flight_time * 1.2).floor() as usize % 2;

        // Colors
        let c_border = Rgb::new(0, 0, 0);
        let c_crust = Rgb::new(235, 175, 100);
        let c_frosting = Rgb::new(255, 155, 210);
        let c_sprinkle = Rgb::new(255, 50, 100);
        let c_cat_grey = Rgb::new(160, 160, 160);
        let c_cheek = Rgb::new(255, 100, 150);
        let c_white = Rgb::WHITE;

        // Pop-Tart Body (Width 11, Height 9) at (cat_base_x, cat_y)
        for by in 0..9i32 {
            for bx in 0..11i32 {
                let px = cat_base_x + bx;
                let py = cat_y + by;

                if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                    let is_border = bx == 0 || bx == 10 || by == 0 || by == 8;
                    let is_crust = bx == 1 || bx == 9 || by == 1 || by == 7;
                    let is_sprinkle = (bx * 3 + by * 5) % 4 == 0;

                    let col = if is_border {
                        c_border
                    } else if is_crust {
                        c_crust
                    } else if is_sprinkle {
                        c_sprinkle
                    } else {
                        c_frosting
                    };
                    frame.set(px as usize, py as usize, col);
                }
            }
        }

        // Cat Head at (cat_base_x + 7, cat_y + 1)
        let hx = cat_base_x + 7;
        let hy = cat_y + 1;

        // Head shape
        for dy in 0..7i32 {
            for dx in 0..7i32 {
                let px = hx + dx;
                let py = hy + dy;
                if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                    // Ears at top (dy == 0: dx == 1, 5)
                    let is_ear = dy == 0 && (dx == 1 || dx == 5);
                    let is_head = (1..=5).contains(&dy) && (1..=5).contains(&dx);
                    let is_snout = dy == 6 && (2..=4).contains(&dx);

                    if is_ear || is_head || is_snout {
                        // Features
                        let is_eye = (dy == 3) && (dx == 2 || dx == 5);
                        let is_eye_shine = (dy == 2) && (dx == 2 || dx == 5);
                        let is_cheek = (dy == 4) && (dx == 1 || dx == 5);
                        let is_nose = (dy == 3) && (dx == 3);

                        let col = if is_eye_shine {
                            c_white
                        } else if is_eye || is_nose {
                            c_border
                        } else if is_cheek {
                            c_cheek
                        } else {
                            c_cat_grey
                        };
                        frame.set(px as usize, py as usize, col);
                    }
                }
            }
        }

        // Legs (Front & Back)
        let leg_y = cat_y + 8;
        if run_frame == 0 {
            // Frame A: legs forward
            frame.set((cat_base_x + 2) as usize, leg_y as usize, c_cat_grey);
            frame.set((cat_base_x + 8) as usize, leg_y as usize, c_cat_grey);
        } else {
            // Frame B: legs back
            frame.set((cat_base_x + 3) as usize, leg_y as usize, c_cat_grey);
            frame.set((cat_base_x + 9) as usize, leg_y as usize, c_cat_grey);
        }

        // Tail (Animated behind Pop-Tart at cat_base_x - 2)
        let tail_dy = if run_frame == 0 { 2 } else { 4 };
        let tx = cat_base_x - 1;
        let ty = cat_y + tail_dy;
        if tx >= 0 && tx < WIDTH as i32 && ty >= 0 && ty < HEIGHT as i32 {
            frame.set(tx as usize, ty as usize, c_cat_grey);
        }
    }
}
