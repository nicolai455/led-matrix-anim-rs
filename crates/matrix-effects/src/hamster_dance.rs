use crate::util::{audio_bass, audio_mid, audio_treble, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Classic Web98", "Cotton Candy", "Cyber Rave", "Rainbow Choir"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "Dance speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 50.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "wave_amp",
        label: "La-Ola wave height",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 6.0,
        step: 0.5,
        default: 3.5,
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

pub struct HamsterDanceEffect {
    values: [f32; 4],
    timer: f32,
    wave_boost: f32,
    color_cycle: f32,
}

impl Default for HamsterDanceEffect {
    fn default() -> Self {
        Self {
            values: [50.0, 3.5, 0.0, 1.0],
            timer: 0.0,
            wave_boost: 0.0,
            color_cycle: 0.0,
        }
    }
}

impl HamsterDanceEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn choir_colors(pal: usize, color_cycle: f32) -> (Rgb, Rgb, Rgb, Rgb) {
        // (HamsterFur, BellyWhite, EarPink, Background)
        match pal {
            0 => (
                Rgb::new(210, 150, 80),  // Golden Hamster
                Rgb::new(250, 245, 235), // Cream white
                Rgb::new(255, 160, 180), // Pink ears
                Rgb::new(25, 30, 60),    // 90s Web Navy
            ),
            1 => (
                Rgb::new(240, 140, 190), // Pastel Pink
                Rgb::new(255, 255, 255),
                Rgb::new(120, 230, 255),
                Rgb::new(35, 15, 35),
            ),
            2 => (
                Rgb::new(0, 230, 180),   // Cyber Mint
                Rgb::new(240, 255, 255),
                Rgb::new(255, 40, 160),
                Rgb::new(15, 10, 28),
            ),
            _ => (
                hsv_to_rgb(color_cycle, 0.85, 1.0),
                Rgb::WHITE,
                hsv_to_rgb((color_cycle + 0.5) % 1.0, 0.9, 1.0),
                Rgb::new(18, 14, 26),
            ),
        }
    }

    fn draw_hamster(frame: &mut Frame, cx: i32, cy: i32, fur_c: Rgb, belly_c: Rgb, ear_c: Rgb, hands_up: bool) {
        // Round 7x7 Hamster body
        for dy in -3..=3i32 {
            let py = cy + dy;
            if py < 0 || py >= HEIGHT as i32 {
                continue;
            }
            let half_w = match dy.abs() {
                3 => 1,
                2 => 2,
                _ => 3,
            };

            for dx in -half_w..=half_w {
                let px = cx + dx;
                if px >= 0 && px < WIDTH as i32 {
                    // White belly patch on lower center
                    let is_belly = dy >= 0 && dx.abs() <= 1;
                    let c = if is_belly { belly_c } else { fur_c };
                    frame.set(px as usize, py as usize, c);
                }
            }
        }

        // Tiny round pink ears at (cx - 2, cy - 4) and (cx + 2, cy - 4)
        if cy - 4 >= 0 && cy - 4 < HEIGHT as i32 {
            if cx - 2 >= 0 && cx - 2 < WIDTH as i32 { frame.set((cx - 2) as usize, (cy - 4) as usize, ear_c); }
            if cx + 2 < WIDTH as i32 { frame.set((cx + 2) as usize, (cy - 4) as usize, ear_c); }
        }

        // Tiny Black Sunglasses / Eyes at (cx - 1, cy - 1) and (cx + 1, cy - 1)
        if cy > 0 && cy - 1 < HEIGHT as i32 {
            if cx > 0 && cx - 1 < WIDTH as i32 { frame.set((cx - 1) as usize, (cy - 1) as usize, Rgb::BLACK); }
            if cx >= 0 && cx < WIDTH as i32 { frame.set(cx as usize, (cy - 1) as usize, Rgb::BLACK); } // Bridge
            if cx + 1 < WIDTH as i32 { frame.set((cx + 1) as usize, (cy - 1) as usize, Rgb::BLACK); }
        }

        // Cute pink nose at (cx, cy)
        if cy >= 0 && cy < HEIGHT as i32 && cx >= 0 && cx < WIDTH as i32 {
            frame.set(cx as usize, cy as usize, ear_c);
        }

        // Animated Paws (Up for wave / cheering, Down for clap)
        let paw_y = if hands_up { cy - 2 } else { cy + 1 };
        if paw_y >= 0 && paw_y < HEIGHT as i32 {
            if cx - 4 >= 0 && cx - 4 < WIDTH as i32 { frame.set((cx - 4) as usize, paw_y as usize, fur_c); }
            if cx + 4 < WIDTH as i32 { frame.set((cx + 4) as usize, paw_y as usize, fur_c); }
        }
    }
}

impl Effect for HamsterDanceEffect {
    fn id(&self) -> &'static str {
        "hamster_dance"
    }

    fn name(&self) -> &'static str {
        "Hamster Dance"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

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
        self.timer = 0.0;
        self.wave_boost = 0.0;
        self.color_cycle = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[3] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.wave_boost = 1.0;
        } else {
            self.wave_boost = (self.wave_boost - dt * 3.5).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.9);
        self.timer = (self.timer + dt * speed * 2.5) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.16) % 1.0;

        let pal = self.values[2] as usize;
        let (fur_c, belly_c, ear_c, bg_c) = Self::choir_colors(pal, self.color_cycle);

        frame.clear(bg_c);

        // 1. Retro 90s Cheerful Polka-Dot Wallpaper
        for y in (2..HEIGHT).step_by(6) {
            for x in (2..WIDTH).step_by(6) {
                let dot_c = Rgb::new(
                    to_byte(bg_c.r as f32 * 1.5 + mid * 60.0),
                    to_byte(bg_c.g as f32 * 1.5 + bass * 60.0),
                    to_byte(bg_c.b as f32 * 1.5 + treble * 60.0),
                );
                frame.set(x, y, dot_c);
            }
        }

        // 2. Hamster Choir Rows (2 Rows of 3 Hamsters = 6 Hamsters dancing in wave sync)
        let base_amp = self.values[1] + bass * 2.5 + self.wave_boost * 1.5;

        // Top Row (3 hamsters at y ~ 10)
        let top_xs = [6i32, 16i32, 26i32];
        for (i, &hx) in top_xs.iter().enumerate() {
            // Wave offset per hamster column
            let phase = self.timer + (i as f32) * 1.2;
            let bounce_y = (phase.sin()) * base_amp;
            let hy = (10.0 - bounce_y).round() as i32;
            let hands_up = phase.sin() > 0.0;
            Self::draw_hamster(frame, hx, hy, fur_c, belly_c, ear_c, hands_up);
        }

        // Bottom Row (3 hamsters at y ~ 22)
        let bot_xs = [6i32, 16i32, 26i32];
        for (i, &hx) in bot_xs.iter().enumerate() {
            // Reverse or staggered wave
            let phase = self.timer + (i as f32) * 1.2 + PI * 0.6;
            let bounce_y = (phase.sin()) * base_amp;
            let hy = (22.0 - bounce_y).round() as i32;
            let hands_up = phase.sin() > 0.0;
            Self::draw_hamster(frame, hx, hy, fur_c, belly_c, ear_c, hands_up);
        }

        // 3. Disco Floor Lighting at bottom row (y = 30..=31)
        for x in 0..WIDTH {
            #[allow(clippy::manual_is_multiple_of)]
            let tile_on = (x + (self.timer * 8.0) as usize) % 3 == 0;
            let tile_c = if tile_on {
                Rgb::new(to_byte(230.0 + mid * 25.0), to_byte(180.0 + bass * 75.0), 40)
            } else {
                Rgb::new(30, 20, 45)
            };
            frame.set(x, 30, tile_c);
            frame.set(x, 31, Rgb::new(15, 10, 25));
        }

        // Musical sparkles on treble
        if audio_active && treble > 0.4 {
            for i in 0..4 {
                let sx = (i * 8 + 3) % WIDTH;
                let sy = (i * 7 + 1) % 8;
                frame.set(sx, sy, Rgb::new(255, 255, 140));
            }
        }
    }
}
