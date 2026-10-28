use crate::util::{audio_bass, audio_mid, audio_treble, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const PALETTES: &[&str] = &["Daylight Retro", "Midnight Neon", "Sunset Vapor", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "Scroll speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 40.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "flap_power",
        label: "Flap jump power",
        kind: ParameterKind::Number,
        min: 2.0,
        max: 8.0,
        step: 0.5,
        default: 4.5,
        options: &[],
        unit: "px",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Theme",
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

pub struct FlappyBirdEffect {
    values: [f32; 4],
    bird_y: f32,
    bird_vy: f32,
    pipe_x: f32,
    pipe_gap_y: f32,
    wing_phase: f32,
    ground_scroll: f32,
    color_cycle: f32,
}

impl Default for FlappyBirdEffect {
    fn default() -> Self {
        Self {
            values: [40.0, 4.5, 0.0, 1.0],
            bird_y: 14.0,
            bird_vy: 0.0,
            pipe_x: 28.0,
            pipe_gap_y: 13.0,
            wing_phase: 0.0,
            ground_scroll: 0.0,
            color_cycle: 0.0,
        }
    }
}

impl FlappyBirdEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn theme_colors(pal: usize, color_cycle: f32) -> (Rgb, Rgb, Rgb, Rgb) {
        // (Sky, Pipe, BirdBody, Ground)
        match pal {
            0 => (
                Rgb::new(75, 190, 230),  // Daylight Cyan
                Rgb::new(80, 200, 40),   // Classic Mario Green
                Rgb::new(255, 215, 20),  // Yellow Bird
                Rgb::new(220, 190, 110), // Sand Ground
            ),
            1 => (
                Rgb::new(12, 16, 35),    // Midnight Navy
                Rgb::new(0, 240, 220),   // Neon Cyan Pipe
                Rgb::new(255, 40, 160),  // Magenta Bird
                Rgb::new(30, 45, 80),
            ),
            2 => (
                Rgb::new(180, 80, 120),  // Sunset Vapor
                Rgb::new(90, 40, 140),   // Violet Pipe
                Rgb::new(255, 170, 50),  // Gold Bird
                Rgb::new(100, 40, 70),
            ),
            _ => (
                Rgb::new(15, 15, 25),
                hsv_to_rgb(color_cycle, 0.9, 1.0),
                hsv_to_rgb((color_cycle + 0.4) % 1.0, 0.9, 1.0),
                Rgb::new(50, 40, 60),
            ),
        }
    }
}

impl Effect for FlappyBirdEffect {
    fn id(&self) -> &'static str {
        "flappy_bird"
    }

    fn name(&self) -> &'static str {
        "Flappy Bird Loop"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "flap_power" => Some(self.values[1]),
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
            "flap_power" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.bird_y = 14.0;
        self.bird_vy = 0.0;
        self.pipe_x = 28.0;
        self.pipe_gap_y = 13.0;
        self.wing_phase = 0.0;
        self.ground_scroll = 0.0;
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

        let pal = self.values[2] as usize;
        let (sky_c, pipe_c, bird_c, ground_c) = Self::theme_colors(pal, self.color_cycle);

        frame.clear(sky_c);

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.7);
        self.ground_scroll = (self.ground_scroll + dt * speed * 18.0) % 32.0;
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        // Pipe scrolling logic
        self.pipe_x -= dt * speed * 14.0;
        if self.pipe_x < -7.0 {
            self.pipe_x = 34.0;
            // Shift pipe gap height between 8 and 18
            self.pipe_gap_y = 9.0 + ((self.ground_scroll * 1.7).sin() * 0.5 + 0.5) * 8.0;
        }

        // Flap & Gravity logic:
        let gravity = 32.0;
        self.bird_vy += gravity * dt;

        // Jump triggers: audio beat or proximity to pipe gap
        let flap_threshold = self.pipe_gap_y + 2.0;
        let should_flap = (beat && self.bird_y > 4.0)
            || (self.bird_y > flap_threshold && self.bird_vy > 0.0)
            || self.bird_y > 23.0;

        if should_flap {
            let flap_impulse = self.values[1] * (if beat { 4.5 } else { 3.8 });
            self.bird_vy = -flap_impulse;
            self.wing_phase = 1.0;
        }

        self.bird_y += self.bird_vy * dt;
        self.bird_y = self.bird_y.clamp(2.0, 24.5);
        if self.bird_y >= 24.5 {
            self.bird_vy = 0.0;
        }

        self.wing_phase = (self.wing_phase - dt * 6.0).max(0.0);

        // 1. Background Clouds (scrolling slowly)
        let cloud_c = Rgb::new(245, 250, 255);
        let cloud_x1 = ((self.ground_scroll * 0.3) % 40.0) as i32;
        let cloud_x2 = (((self.ground_scroll * 0.3) + 20.0) % 40.0) as i32;

        for &(cx, cy) in &[(35 - cloud_x1, 5), (35 - cloud_x2, 8)] {
            for cdx in -2..=2i32 {
                let px = cx + cdx;
                if px >= 0 && px < WIDTH as i32 && cy < HEIGHT as i32 {
                    frame.set(px as usize, cy as usize, cloud_c);
                }
            }
        }

        // 2. Pipes (Top pipe and Bottom pipe)
        let pipe_w = 6i32;
        let gap_h = 10.0f32; // Gap size 10 pixels
        let pipe_left = self.pipe_x.round() as i32;

        let pipe_rim_c = Rgb::new(
            to_byte(pipe_c.r as f32 * 1.2),
            to_byte(pipe_c.g as f32 * 1.2),
            to_byte(pipe_c.b as f32 * 1.2),
        );
        let pipe_shade_c = Rgb::new(
            to_byte(pipe_c.r as f32 * 0.7),
            to_byte(pipe_c.g as f32 * 0.7),
            to_byte(pipe_c.b as f32 * 0.7),
        );

        let gap_top = (self.pipe_gap_y - gap_h * 0.5).round() as i32;
        let gap_bot = (self.pipe_gap_y + gap_h * 0.5).round() as i32;

        for dx in 0..pipe_w {
            let px = pipe_left + dx;
            if px < 0 || px >= WIDTH as i32 {
                continue;
            }

            let col = if dx == 1 {
                pipe_rim_c // Highlight stripe
            } else if dx == pipe_w - 1 {
                pipe_shade_c // Shadow
            } else {
                pipe_c
            };

            // Top Pipe (from y = 0 down to gap_top)
            for py in 0..gap_top {
                if py >= 0 && py < HEIGHT as i32 {
                    frame.set(px as usize, py as usize, col);
                }
            }
            // Bottom Pipe (from gap_bot down to ground y = 25)
            for py in gap_bot..26 {
                if py >= 0 && py < HEIGHT as i32 {
                    frame.set(px as usize, py as usize, col);
                }
            }
        }

        // Pipe Flange Collars (lip at opening)
        if pipe_left - 1 < WIDTH as i32 && pipe_left + pipe_w >= 0 {
            for dx in -1..=(pipe_w) {
                let px = pipe_left + dx;
                if px >= 0 && px < WIDTH as i32 {
                    if gap_top > 0 && gap_top - 1 < HEIGHT as i32 {
                        frame.set(px as usize, (gap_top - 1) as usize, pipe_shade_c);
                    }
                    if gap_bot >= 0 && gap_bot < HEIGHT as i32 {
                        frame.set(px as usize, gap_bot as usize, pipe_shade_c);
                    }
                }
            }
        }

        // 3. Scrolling Ground & Grass (y: 26..31)
        let grass_c = Rgb::new(120, 210, 40);
        let ground_grid_c = Rgb::new(170, 140, 80);

        for x in 0..WIDTH {
            frame.set(x, 26, grass_c);
            let checker = ((x as f32 + self.ground_scroll).floor() as usize) % 4 < 2;
            let c_bot = if checker { ground_c } else { ground_grid_c };
            for y in 27..HEIGHT {
                frame.set(x, y, c_bot);
            }
        }

        // 4. Flappy Bird (x = 10, y = bird_y)
        let bx = 10i32;
        let by = self.bird_y.round() as i32;

        // Bird Body (Width 6, Height 5)
        for dy in -2..=2i32 {
            for dx in -3..=2i32 {
                let px = bx + dx;
                let py = by + dy;
                if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                    // Elliptic round body
                    if (dx * dx * 4 + dy * dy * 9) <= 36 {
                        frame.set(px as usize, py as usize, bird_c);
                    }
                }
            }
        }

        // Big White Eye & Pupil at (bx + 1, by - 1)
        if bx + 1 < WIDTH as i32 && by > 0 && by - 1 < HEIGHT as i32 {
            frame.set((bx + 1) as usize, (by - 1) as usize, Rgb::WHITE);
            frame.set((bx + 2) as usize, (by - 1) as usize, Rgb::WHITE);
            frame.set((bx + 2) as usize, (by - 1) as usize, Rgb::BLACK); // Pupil looking forward
        }

        // Orange Beak at (bx + 3, by) and (bx + 4, by)
        let beak_c = Rgb::new(255, 120, 20);
        if by >= 0 && by < HEIGHT as i32 {
            if bx + 3 < WIDTH as i32 { frame.set((bx + 3) as usize, by as usize, beak_c); }
            if bx + 4 < WIDTH as i32 { frame.set((bx + 4) as usize, by as usize, beak_c); }
            if by + 1 < HEIGHT as i32 && bx + 3 < WIDTH as i32 {
                frame.set((bx + 3) as usize, (by + 1) as usize, beak_c);
            }
        }

        // Flapping Wing at (bx - 1, by)
        let wing_c = Rgb::WHITE;
        let wing_offset_y = if self.wing_phase > 0.3 { -1 } else { 1 };
        if bx > 0 && by + wing_offset_y >= 0 && (by + wing_offset_y) < HEIGHT as i32 {
            frame.set((bx - 1) as usize, (by + wing_offset_y) as usize, wing_c);
            frame.set(bx as usize, (by + wing_offset_y) as usize, wing_c);
        }

        // 5. Coin score sparkle on treble
        if audio_active && treble > 0.4 {
            let spark_x = (bx + 6) as usize;
            let spark_y = (by - 3).max(0) as usize;
            if spark_x < WIDTH && spark_y < HEIGHT {
                frame.set(spark_x, spark_y, Rgb::new(255, 240, 80));
            }
        }

        if audio_active && mid > 0.35 {
            frame.set(2, 2, Rgb::new(255, 255, 180));
        }
    }
}
