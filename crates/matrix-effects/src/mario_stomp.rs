use crate::util::{audio_bass, audio_mid, audio_treble, hsv_to_rgb};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const PALETTES: &[&str] = &["NES Overworld", "Underground 1-2", "Bowser Castle", "Star Invincible"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "Run speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 40.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "jump_height",
        label: "Jump height",
        kind: ParameterKind::Number,
        min: 6.0,
        max: 16.0,
        step: 1.0,
        default: 11.0,
        options: &[],
        unit: "px",
    },
    ParameterDescriptor {
        id: "palette",
        label: "World theme",
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

pub struct MarioStompEffect {
    values: [f32; 4],
    mario_y: f32,
    mario_vy: f32,
    is_jumping: bool,
    run_phase: f32,
    goomba_x: f32,
    goomba_squished: f32,
    block_bump: f32,
    coin_pop_y: f32,
    color_cycle: f32,
}

impl Default for MarioStompEffect {
    fn default() -> Self {
        Self {
            values: [40.0, 11.0, 0.0, 1.0],
            mario_y: 20.0,
            mario_vy: 0.0,
            is_jumping: false,
            run_phase: 0.0,
            goomba_x: 28.0,
            goomba_squished: 0.0,
            block_bump: 0.0,
            coin_pop_y: -10.0,
            color_cycle: 0.0,
        }
    }
}

impl MarioStompEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn world_colors(pal: usize, color_cycle: f32) -> (Rgb, Rgb, Rgb, Rgb) {
        // (Sky, Ground, MarioOveralls, MarioCap)
        match pal {
            0 => (
                Rgb::new(100, 150, 255), // NES Blue Sky
                Rgb::new(200, 75, 20),   // Brick Orange
                Rgb::new(20, 50, 230),   // Blue Overalls
                Rgb::new(230, 20, 20),   // Red Shirt / Cap
            ),
            1 => (
                Rgb::new(10, 10, 15),    // Black Underground
                Rgb::new(0, 130, 140),   // Blue Bricks
                Rgb::new(0, 160, 200),
                Rgb::new(230, 150, 40),
            ),
            2 => (
                Rgb::new(30, 10, 10),    // Castle Charcoal
                Rgb::new(120, 40, 20),   // Dark Red Blocks
                Rgb::new(40, 40, 40),
                Rgb::new(255, 60, 20),
            ),
            _ => (
                Rgb::new(15, 12, 25),
                Rgb::new(60, 50, 70),
                hsv_to_rgb(color_cycle, 0.9, 1.0),
                hsv_to_rgb((color_cycle + 0.3) % 1.0, 0.95, 1.0),
            ),
        }
    }
}

impl Effect for MarioStompEffect {
    fn id(&self) -> &'static str {
        "mario_stomp"
    }

    fn name(&self) -> &'static str {
        "Mario Goomba Stomp"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "jump_height" => Some(self.values[1]),
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
            "jump_height" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.mario_y = 20.0;
        self.mario_vy = 0.0;
        self.is_jumping = false;
        self.run_phase = 0.0;
        self.goomba_x = 28.0;
        self.goomba_squished = 0.0;
        self.block_bump = 0.0;
        self.coin_pop_y = -10.0;
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
        let (sky_c, ground_c, overalls_c, cap_c) = Self::world_colors(pal, self.color_cycle);

        frame.clear(sky_c);

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.7);
        self.run_phase += dt * speed * 12.0;
        self.color_cycle = (self.color_cycle + dt * 0.2) % 1.0;

        let ground_y = 21.0f32; // Mario stands at y=21 (height 5 -> bottoms at 26)

        // Physics & Jumping
        let gravity = 55.0;
        self.mario_vy += gravity * dt;
        self.mario_y += self.mario_vy * dt;

        if self.mario_y >= ground_y {
            self.mario_y = ground_y;
            self.mario_vy = 0.0;
            self.is_jumping = false;
        }

        // Goomba moving towards Mario (from right to left)
        if self.goomba_squished > 0.0 {
            self.goomba_squished = (self.goomba_squished - dt * 2.5).max(0.0);
            if self.goomba_squished == 0.0 {
                self.goomba_x = 34.0; // Respawn Goomba on right
            }
        } else {
            self.goomba_x -= dt * speed * 11.0;
            if self.goomba_x < -6.0 {
                self.goomba_x = 34.0;
            }
        }

        // Mario Jump Trigger:
        // Jump when Goomba gets close (x ~ 14..18) or on beat!
        let goomba_dist = self.goomba_x - 8.0;
        let should_jump = (!self.is_jumping && goomba_dist > 2.0 && goomba_dist < 11.0)
            || (beat && !self.is_jumping);

        if should_jump {
            let jump_power = self.values[1] * (if beat { 2.4 } else { 2.0 });
            self.mario_vy = -jump_power;
            self.is_jumping = true;
        }

        // Stomp Collision Check (Mario landing on Goomba)
        if self.goomba_squished == 0.0 && self.is_jumping && self.mario_vy > 0.0 {
            let mario_feet = self.mario_y + 4.5;
            let goomba_top = 22.0;
            let dx = (self.goomba_x - 8.0).abs();
            if dx < 4.0 && (mario_feet - goomba_top).abs() < 2.5 {
                // STOMP SUCCESS!
                self.goomba_squished = 1.0;
                self.mario_vy = -18.0; // Stomp bounce!
                self.block_bump = 1.0;
                self.coin_pop_y = 12.0;
            }
        }

        // 1. Question Block at x: 8..=12, y: 11
        if self.block_bump > 0.0 {
            self.block_bump = (self.block_bump - dt * 4.0).max(0.0);
        }
        let block_y = 11i32 - (self.block_bump * 2.0).round() as i32;
        let q_block_c = Rgb::new(245, 170, 30);
        let q_block_border = Rgb::new(140, 80, 10);

        for dy in 0..5i32 {
            for dx in 0..5i32 {
                let px = 8 + dx;
                let py = block_y + dy;
                if px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                    let is_border = dx == 0 || dx == 4 || dy == 0 || dy == 4;
                    let is_question = (dx == 2 && dy == 2) || (dx == 2 && dy == 1) || (dx == 2 && dy == 3);
                    let c = if is_border {
                        q_block_border
                    } else if is_question {
                        Rgb::WHITE
                    } else {
                        q_block_c
                    };
                    frame.set(px as usize, py as usize, c);
                }
            }
        }

        // Coin Pop from block
        if self.coin_pop_y > 0.0 {
            self.coin_pop_y -= dt * 16.0;
            let cy = self.coin_pop_y.round() as i32;
            if cy >= 0 && cy < HEIGHT as i32 {
                frame.set(10, cy as usize, Rgb::new(255, 230, 50));
                frame.set(10, (cy + 1) as usize, Rgb::new(255, 230, 50));
            }
        }

        // 2. Brick Ground (y: 26..31)
        let ground_mortar = Rgb::new(80, 20, 10);
        for y in 26..HEIGHT {
            for x in 0..WIDTH {
                let brick_line = y == 26 || y == 29;
                let seam = if y < 29 { x % 6 == 0 } else { (x + 3) % 6 == 0 };
                let col = if brick_line || seam {
                    ground_mortar
                } else {
                    ground_c
                };
                frame.set(x, y, col);
            }
        }

        // 3. Goomba Rendering (at goomba_x, y: 22..25)
        let gx = self.goomba_x.round() as i32;
        let goomba_c = Rgb::new(175, 75, 15);
        let goomba_face = Rgb::new(245, 220, 170);

        if self.goomba_squished > 0.0 {
            // Squished flat pancake Goomba!
            for dx in -3..=3i32 {
                let px = gx + dx;
                if px >= 0 && px < WIDTH as i32 {
                    frame.set(px as usize, 25, goomba_c);
                }
            }
        } else {
            // Walking Goomba (5x4)
            for dy in 0..4i32 {
                let py = 22 + dy;
                for dx in -2..=2i32 {
                    let px = gx + dx;
                    if px >= 0 && px < WIDTH as i32 && py < HEIGHT as i32 {
                        let is_face = dy >= 2 && dx.abs() <= 1;
                        let c = if is_face { goomba_face } else { goomba_c };
                        frame.set(px as usize, py as usize, c);
                    }
                }
            }
            // Angry Eyebrows & Eyes
            if gx > 0 && gx - 1 < WIDTH as i32 { frame.set((gx - 1) as usize, 23, Rgb::BLACK); }
            if gx + 1 < WIDTH as i32 { frame.set((gx + 1) as usize, 23, Rgb::BLACK); }

            // Feet waddle
            #[allow(clippy::manual_is_multiple_of)]
            let waddle = ((self.run_phase * 1.5).floor() as usize) % 2 == 0;
            let foot_x = if waddle { gx - 1 } else { gx + 1 };
            if foot_x >= 0 && foot_x < WIDTH as i32 {
                frame.set(foot_x as usize, 25, Rgb::new(40, 20, 10));
            }
        }

        // 4. Mario (at x: 7..=9, y: mario_y)
        let mx = 8i32;
        let my = self.mario_y.round() as i32;
        let skin_c = Rgb::new(255, 200, 140);

        // Cap (y: my, x: mx - 1 ..= mx + 2)
        for dx in -1..=2i32 {
            let px = mx + dx;
            if px >= 0 && px < WIDTH as i32 && my >= 0 && my < HEIGHT as i32 {
                frame.set(px as usize, my as usize, cap_c);
            }
        }

        // Face & Mustache (y: my + 1)
        if my + 1 < HEIGHT as i32 {
            frame.set(mx as usize, (my + 1) as usize, skin_c);
            if mx + 1 < WIDTH as i32 { frame.set((mx + 1) as usize, (my + 1) as usize, skin_c); }
            if mx + 2 < WIDTH as i32 { frame.set((mx + 2) as usize, (my + 1) as usize, Rgb::BLACK); } // Mustache/nose
        }

        // Shirt & Overalls Body (y: my + 2..my + 3)
        if my + 2 < HEIGHT as i32 {
            if mx > 0 { frame.set((mx - 1) as usize, (my + 2) as usize, cap_c); } // Arm/shirt
            frame.set(mx as usize, (my + 2) as usize, overalls_c);
            if mx + 1 < WIDTH as i32 { frame.set((mx + 1) as usize, (my + 2) as usize, overalls_c); }
        }
        if my + 3 < HEIGHT as i32 {
            frame.set(mx as usize, (my + 3) as usize, overalls_c);
            if mx + 1 < WIDTH as i32 { frame.set((mx + 1) as usize, (my + 3) as usize, overalls_c); }
        }

        // Legs / Boots (y: my + 4)
        if my + 4 < HEIGHT as i32 {
            let boot_c = Rgb::new(120, 50, 15);
            if self.is_jumping {
                // Jump pose: legs tucked up or split
                if mx > 0 { frame.set((mx - 1) as usize, (my + 4) as usize, boot_c); }
                if mx + 2 < WIDTH as i32 { frame.set((mx + 2) as usize, (my + 3) as usize, boot_c); }
            } else {
                // Running stride
                #[allow(clippy::manual_is_multiple_of)]
                let stride = ((self.run_phase).floor() as usize) % 2 == 0;
                let l_leg = if stride { mx - 1 } else { mx };
                let r_leg = if stride { mx + 1 } else { mx + 2 };
                if l_leg >= 0 && l_leg < WIDTH as i32 { frame.set(l_leg as usize, (my + 4) as usize, boot_c); }
                if r_leg < WIDTH as i32 { frame.set(r_leg as usize, (my + 4) as usize, boot_c); }
            }
        }

        // Raised Arm on Jump
        if self.is_jumping && my > 0 && mx + 1 < WIDTH as i32 {
            frame.set((mx + 1) as usize, (my - 1) as usize, Rgb::WHITE); // White glove in air
        }

        // 5. Star sparkle particles on treble / beat
        if audio_active && treble > 0.4 {
            for i in 0..3 {
                let sx = (i * 10 + 6) % WIDTH;
                let sy = (i * 4 + 2) % 10;
                frame.set(sx, sy, Rgb::new(255, 255, 120));
            }
        }
        if audio_active && mid > 0.3 {
            frame.set(2, 2, Rgb::new(100, 240, 255));
        }
    }
}
