use crate::util::{audio_bass, audio_mid, audio_treble, hsv_to_rgb, to_byte, XorShift32};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const PALETTES: &[&str] = &["Arcade 1976", "Neon Cyber", "Monochrome CRT", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "Ball speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 45.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "paddle_size",
        label: "Paddle width",
        kind: ParameterKind::Number,
        min: 4.0,
        max: 10.0,
        step: 1.0,
        default: 6.0,
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

const BRICK_ROWS: usize = 4;
const BRICK_COLS: usize = 6;
const BRICK_W: usize = 5;
const BRICK_H: usize = 2;

#[derive(Clone, Copy, Debug)]
struct BrickSpark {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    color: Rgb,
}

pub struct RetroBreakoutEffect {
    values: [f32; 4],
    ball_x: f32,
    ball_y: f32,
    ball_vx: f32,
    ball_vy: f32,
    paddle_x: f32,
    bricks: [[u8; BRICK_COLS]; BRICK_ROWS],
    sparks: [BrickSpark; 16],
    color_cycle: f32,
    rng: XorShift32,
}

impl Default for RetroBreakoutEffect {
    fn default() -> Self {
        let mut inst = Self {
            values: [45.0, 6.0, 0.0, 1.0],
            ball_x: 15.0,
            ball_y: 20.0,
            ball_vx: 14.0,
            ball_vy: -18.0,
            paddle_x: 13.0,
            bricks: [[1; BRICK_COLS]; BRICK_ROWS],
            sparks: [BrickSpark { x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, life: 0.0, color: Rgb::WHITE }; 16],
            color_cycle: 0.0,
            rng: XorShift32::new(192837),
        };
        inst.reset_bricks();
        inst
    }
}

impl RetroBreakoutEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn reset_bricks(&mut self) {
        self.bricks = [[1; BRICK_COLS]; BRICK_ROWS];
    }

    fn brick_color(pal: usize, row: usize, color_cycle: f32) -> Rgb {
        match pal {
            0 => {
                // Arcade 1976 (Red, Orange, Green, Yellow)
                match row {
                    0 => Rgb::new(255, 30, 40),
                    1 => Rgb::new(255, 140, 20),
                    2 => Rgb::new(40, 230, 60),
                    _ => Rgb::new(255, 230, 20),
                }
            }
            1 => {
                // Neon Cyber (Magenta, Violet, Cyan, Lime)
                match row {
                    0 => Rgb::new(255, 20, 160),
                    1 => Rgb::new(160, 40, 255),
                    2 => Rgb::new(0, 240, 255),
                    _ => Rgb::new(60, 255, 120),
                }
            }
            2 => {
                // Monochrome CRT Amber
                let b = 255 - (row as u8 * 45);
                Rgb::new(255, b, 20)
            }
            _ => {
                // Rainbow Fade
                let h = (color_cycle + row as f32 * 0.22) % 1.0;
                hsv_to_rgb(h, 0.95, 1.0)
            }
        }
    }
}

impl Effect for RetroBreakoutEffect {
    fn id(&self) -> &'static str {
        "retro_breakout"
    }

    fn name(&self) -> &'static str {
        "Retro Breakout"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "paddle_size" => Some(self.values[1]),
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
            "paddle_size" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.ball_x = 15.0;
        self.ball_y = 20.0;
        self.ball_vx = 14.0;
        self.ball_vy = -18.0;
        self.paddle_x = 13.0;
        self.color_cycle = 0.0;
        for s in &mut self.sparks {
            s.life = 0.0;
        }
        self.reset_bricks();
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::BLACK);

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[3] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let _mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        let speed_mult = (self.values[0] / 45.0) * (1.0 + bass * 0.7);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let pal = self.values[2] as usize;
        let base_paddle_w = self.values[1] + bass * 2.0;

        // 1. Paddle AI: Tracks ball smoothly
        let target_paddle_x = self.ball_x - base_paddle_w * 0.5;
        self.paddle_x += (target_paddle_x - self.paddle_x) * (dt * 15.0).min(1.0);
        self.paddle_x = self.paddle_x.clamp(1.0, WIDTH as f32 - base_paddle_w - 1.0);

        // 2. Ball Motion & Physics
        let spd = speed_mult * (if beat { 1.5 } else { 1.0 });
        self.ball_x += self.ball_vx * spd * dt;
        self.ball_y += self.ball_vy * spd * dt;

        // Left / Right wall bounce
        if self.ball_x <= 1.0 {
            self.ball_x = 1.0;
            self.ball_vx = self.ball_vx.abs();
        } else if self.ball_x >= (WIDTH as f32 - 2.0) {
            self.ball_x = WIDTH as f32 - 2.0;
            self.ball_vx = -self.ball_vx.abs();
        }

        // Top ceiling bounce
        if self.ball_y <= 1.0 {
            self.ball_y = 1.0;
            self.ball_vy = self.ball_vy.abs();
        }

        // Bottom Paddle Bounce
        let paddle_y = 29.0f32;
        if self.ball_y >= (paddle_y - 1.2) && self.ball_y <= (paddle_y + 1.2) {
            let hit_offset = (self.ball_x - (self.paddle_x + base_paddle_w * 0.5)) / (base_paddle_w * 0.5);
            if (-1.2..=1.2).contains(&hit_offset) {
                self.ball_vy = -self.ball_vy.abs();
                self.ball_vx = hit_offset * 18.0;
            }
        }

        // Bottom drain reset
        if self.ball_y > 31.0 {
            self.ball_x = self.paddle_x + base_paddle_w * 0.5;
            self.ball_y = 24.0;
            self.ball_vy = -18.0;
            self.ball_vx = 12.0;
        }

        // 3. Brick Collision Detection
        let brick_start_x = 1.0f32;
        let brick_start_y = 3.0f32;

        let mut all_cleared = true;

        for (r, row) in self.bricks.iter_mut().enumerate() {
            let by0 = brick_start_y + (r * (BRICK_H + 1)) as f32;
            let by1 = by0 + BRICK_H as f32;

            for (c, brick) in row.iter_mut().enumerate() {
                if *brick == 1 {
                    all_cleared = false;
                    let bx0 = brick_start_x + (c * (BRICK_W + 1)) as f32;
                    let bx1 = bx0 + BRICK_W as f32;

                    if self.ball_x >= bx0 && self.ball_x <= bx1 && self.ball_y >= by0 && self.ball_y <= by1 {
                        // Brick hit! Destroy brick and bounce ball
                        *brick = 0;
                        self.ball_vy = -self.ball_vy;

                        // Spawn sparks
                        let col = Self::brick_color(pal, r, self.color_cycle);
                        for s in &mut self.sparks {
                            if s.life <= 0.0 {
                                s.life = 0.5;
                                s.x = self.ball_x;
                                s.y = self.ball_y;
                                s.vx = (self.rng.next_f32() - 0.5) * 20.0;
                                s.vy = (self.rng.next_f32() - 0.5) * 20.0;
                                s.color = col;
                                break;
                            }
                        }
                    }
                }
            }
        }

        if all_cleared {
            self.reset_bricks();
        }

        // 4. Draw Bricks
        for (r, row) in self.bricks.iter().enumerate() {
            let by0 = (brick_start_y + (r * (BRICK_H + 1)) as f32).round() as usize;
            let col = Self::brick_color(pal, r, self.color_cycle);

            for (c, &brick) in row.iter().enumerate() {
                if brick == 1 {
                    let bx0 = (brick_start_x + (c * (BRICK_W + 1)) as f32).round() as usize;
                    for dy in 0..BRICK_H {
                        let py = by0 + dy;
                        for dx in 0..BRICK_W {
                            let px = bx0 + dx;
                            if px < WIDTH && py < HEIGHT {
                                frame.set(px, py, col);
                            }
                        }
                    }
                }
            }
        }

        // 5. Draw Paddle (at y=29)
        let px_start = self.paddle_x.round() as i32;
        let pw = base_paddle_w.round() as i32;
        let pad_col = if beat { Rgb::WHITE } else { Rgb::new(0, 240, 255) };

        for x in px_start..(px_start + pw) {
            if x >= 0 && x < WIDTH as i32 {
                frame.set(x as usize, 29, pad_col);
            }
        }

        // 6. Draw Ball (Energy core at ball_x, ball_y)
        let bx = self.ball_x.round() as i32;
        let by = self.ball_y.round() as i32;

        if bx >= 0 && bx < WIDTH as i32 && by >= 0 && by < HEIGHT as i32 {
            frame.set(bx as usize, by as usize, Rgb::WHITE);
        }

        // 7. Update and Draw Sparks
        for s in &mut self.sparks {
            if s.life > 0.0 {
                s.life -= dt;
                s.x += s.vx * dt;
                s.y += s.vy * dt;

                let sx = s.x.round() as i32;
                let sy = s.y.round() as i32;

                if sx >= 0 && sx < WIDTH as i32 && sy >= 0 && sy < HEIGHT as i32 {
                    let cur = frame.get(sx as usize, sy as usize);
                    frame.set(
                        sx as usize,
                        sy as usize,
                        Rgb::new(
                            cur.r.saturating_add(s.color.r),
                            cur.g.saturating_add(s.color.g),
                            cur.b.saturating_add(s.color.b),
                        ),
                    );
                }
            }
        }

        // Border walls at left (x=0) and right (x=31)
        let wall_c = Rgb::new(to_byte(40.0 + treble * 50.0), to_byte(40.0 + treble * 50.0), 60);
        for y in 0..HEIGHT {
            frame.set(0, y, wall_c);
            frame.set(31, y, wall_c);
        }
    }
}
