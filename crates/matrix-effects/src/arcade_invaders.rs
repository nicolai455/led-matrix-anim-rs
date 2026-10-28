use crate::util::{audio_bass, audio_mid, audio_treble, hsv_to_rgb, to_byte, XorShift32};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const PALETTES: &[&str] = &["Classic Arcade", "Cyber Neon", "Laser Green", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "March tempo",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 35.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "lasers",
        label: "Laser fire rate",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 50.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Alien colors",
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
struct Laser {
    x: f32,
    y: f32,
    vy: f32,
    is_player: bool,
    active: bool,
}

pub struct ArcadeInvadersEffect {
    values: [f32; 4],
    march_x: f32,
    march_dir: f32,
    march_y: f32,
    step_timer: f32,
    anim_frame: usize,
    lasers: [Laser; 12],
    color_cycle: f32,
    rng: XorShift32,
}

impl Default for ArcadeInvadersEffect {
    fn default() -> Self {
        Self {
            values: [35.0, 50.0, 0.0, 1.0],
            march_x: 2.0,
            march_dir: 1.0,
            march_y: 3.0,
            step_timer: 0.0,
            anim_frame: 0,
            lasers: [Laser { x: 0.0, y: 0.0, vy: 0.0, is_player: false, active: false }; 12],
            color_cycle: 0.0,
            rng: XorShift32::new(7654321),
        }
    }
}

impl ArcadeInvadersEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn draw_alien_sprite(
        frame: &mut Frame,
        alien_type: usize,
        anim_frame: usize,
        ox: i32,
        oy: i32,
        color: Rgb,
    ) {
        // 5x4 pixel art alien sprites
        for dy in 0..4i32 {
            for dx in 0..5i32 {
                let px = ox + dx;
                let py = oy + dy;

                if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                    let pixel_on = match alien_type {
                        0 => {
                            // Squid / Top Row (3x4)
                            match (dy, anim_frame) {
                                (0, _) => dx == 2,
                                (1, _) => (1..=3).contains(&dx),
                                (2, _) => dx == 0 || dx == 2 || dx == 4,
                                (3, 0) => dx == 1 || dx == 3,
                                _ => dx == 0 || dx == 4,
                            }
                        }
                        1 => {
                            // Crab / Mid Row (5x4)
                            match (dy, anim_frame) {
                                (0, _) => dx == 1 || dx == 3,
                                (1, _) => true,
                                (2, _) => dx != 2,
                                (3, 0) => dx == 0 || dx == 4,
                                _ => dx == 1 || dx == 3,
                            }
                        }
                        _ => {
                            // Octopus / Bottom Row (5x4)
                            match (dy, anim_frame) {
                                (0, _) => (1..=3).contains(&dx),
                                (1, _) => true,
                                (2, _) => dx == 0 || dx == 2 || dx == 4,
                                (3, 0) => dx == 1 || dx == 3,
                                _ => dx == 0 || dx == 4,
                            }
                        }
                    };

                    if pixel_on {
                        frame.set(px as usize, py as usize, color);
                    }
                }
            }
        }
    }

    fn alien_color(pal: usize, row: usize, color_cycle: f32) -> Rgb {
        match pal {
            0 => {
                // Classic Arcade
                match row {
                    0 => Rgb::new(255, 30, 80),   // Top: Red Squid
                    1 => Rgb::new(0, 240, 255),   // Mid: Cyan Crab
                    _ => Rgb::new(50, 255, 100),  // Bottom: Lime Octopus
                }
            }
            1 => {
                // Cyber Neon
                match row {
                    0 => Rgb::new(255, 20, 180),
                    1 => Rgb::new(240, 220, 20),
                    _ => Rgb::new(0, 220, 255),
                }
            }
            2 => {
                // Laser Green
                let g = 255 - (row as u8 * 45);
                Rgb::new(30, g, 40)
            }
            _ => {
                // Rainbow Fade
                let h = (color_cycle + row as f32 * 0.28) % 1.0;
                hsv_to_rgb(h, 0.95, 1.0)
            }
        }
    }
}

impl Effect for ArcadeInvadersEffect {
    fn id(&self) -> &'static str {
        "arcade_invaders"
    }

    fn name(&self) -> &'static str {
        "Space Invaders Galaxy"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "lasers" => Some(self.values[1]),
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
            "lasers" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.march_x = 2.0;
        self.march_dir = 1.0;
        self.march_y = 3.0;
        self.step_timer = 0.0;
        self.anim_frame = 0;
        for laser in &mut self.lasers {
            laser.active = false;
        }
        self.color_cycle = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::BLACK);

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[3] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        self.color_cycle = (self.color_cycle + dt * 0.16) % 1.0;

        // March cadence: accelerated by tempo & beats
        let march_tempo = (self.values[0] / 30.0) * (if beat { 3.5 } else { 1.0 + bass * 0.8 });
        let step_period = 0.35 / march_tempo.max(0.5);

        self.step_timer += dt;
        if self.step_timer >= step_period {
            self.step_timer -= step_period;
            self.anim_frame = (self.anim_frame + 1) % 2;

            self.march_x += self.march_dir * 1.5;
            if self.march_x > 6.0 {
                self.march_x = 6.0;
                self.march_dir = -1.0;
                self.march_y = (self.march_y + 1.0) % 10.0;
            } else if self.march_x < 1.0 {
                self.march_x = 1.0;
                self.march_dir = 1.0;
                self.march_y = (self.march_y + 1.0) % 10.0;
            }
        }

        let pal = self.values[2] as usize;
        let base_march_y = self.march_y as i32 + 2;

        // 1. Draw 3 Rows of 4 Aliens (12 Invaders total)
        for row in 0..3 {
            let row_y = base_march_y + row * 6;
            let col_color = Self::alien_color(pal, row as usize, self.color_cycle);

            for col in 0..4 {
                let alien_x = (self.march_x + (col as f32 * 6.5)).round() as i32;
                Self::draw_alien_sprite(frame, row as usize, self.anim_frame, alien_x, row_y, col_color);
            }
        }

        // 2. Draw Defensive Bunkers at y=23 (3 Green Shields)
        for b_idx in 0..3 {
            let bx = 4 + b_idx * 9;
            let shield_c = Rgb::new(to_byte(40.0 + treble * 60.0), 255, to_byte(80.0 + mid * 80.0));
            // 5x3 bunker with doorway arch
            for by in 0..3i32 {
                for bdx in 0..5i32 {
                    let is_doorway = by == 2 && bdx == 2;
                    if !is_doorway {
                        frame.set((bx + bdx) as usize, (23 + by) as usize, shield_c);
                    }
                }
            }
        }

        // 3. Draw Player Cannon at Bottom (y=29, Width 5, Height 3)
        let player_x = 13 + ((self.march_x * 2.2).sin() * 8.0).round() as i32;
        let cannon_c = if pal == 3 {
            hsv_to_rgb((self.color_cycle + 0.5) % 1.0, 1.0, 1.0)
        } else {
            Rgb::new(50, 255, 80) // Classic Cannon Green
        };

        for py in 0..3i32 {
            for px in 0..5i32 {
                let is_barrel = py == 0 && px == 2;
                let is_turret = py == 1 && (1..=3).contains(&px);
                let is_base = py == 2;
                if is_barrel || is_turret || is_base {
                    let rx = player_x + px;
                    let ry = 28 + py;
                    if rx >= 0 && rx < WIDTH as i32 && ry < HEIGHT as i32 {
                        frame.set(rx as usize, ry as usize, cannon_c);
                    }
                }
            }
        }

        // 4. Update and Draw Lasers
        let laser_rate = self.values[1] / 50.0;
        // Fire player laser on beat or random timer
        if (beat || self.rng.next_f32() < dt * 3.5 * laser_rate) && !self.lasers.iter().any(|l| l.active && l.is_player) {
            for laser in &mut self.lasers {
                if !laser.active {
                    laser.active = true;
                    laser.x = (player_x + 2) as f32;
                    laser.y = 27.0;
                    laser.vy = -35.0;
                    laser.is_player = true;
                    break;
                }
            }
        }

        // Alien bombs dropped downwards
        if self.rng.next_f32() < dt * 4.0 * laser_rate {
            for laser in &mut self.lasers {
                if !laser.active {
                    laser.active = true;
                    let rand_col = (self.rng.next_f32() * 4.0) as usize;
                    laser.x = self.march_x + rand_col as f32 * 6.5 + 2.0;
                    laser.y = (base_march_y + 16) as f32;
                    laser.vy = 22.0;
                    laser.is_player = false;
                    break;
                }
            }
        }

        for laser in &mut self.lasers {
            if !laser.active {
                continue;
            }
            laser.y += laser.vy * dt;

            let lx = laser.x.round() as i32;
            let ly = laser.y.round() as i32;

            if lx >= 0 && lx < WIDTH as i32 && ly >= 0 && ly < HEIGHT as i32 {
                let col = if laser.is_player {
                    Rgb::new(255, 255, 100)
                } else {
                    Rgb::new(255, 40, 50)
                };
                frame.set(lx as usize, ly as usize, col);
            } else {
                laser.active = false;
            }
        }
    }
}
