use crate::util::{audio_bass, audio_mid, audio_treble, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Classic Arcade", "Neon Cyber", "Retro Amber", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "Chase speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 35.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "ghosts",
        label: "Ghost count",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 3.0,
        step: 1.0,
        default: 2.0,
        options: &[],
        unit: "",
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

pub struct PacmanChaseEffect {
    values: [f32; 4],
    travel_x: f32,
    mouth_phase: f32,
    scared_timer: f32,
    color_cycle: f32,
    power_pellet_flash: f32,
}

impl Default for PacmanChaseEffect {
    fn default() -> Self {
        Self {
            values: [35.0, 2.0, 0.0, 1.0],
            travel_x: 0.0,
            mouth_phase: 0.0,
            scared_timer: 0.0,
            color_cycle: 0.0,
            power_pellet_flash: 0.0,
        }
    }
}

impl PacmanChaseEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn ghost_color(pal: usize, ghost_idx: usize, is_scared: bool, flash_white: bool, color_cycle: f32) -> Rgb {
        if is_scared {
            if flash_white {
                Rgb::WHITE
            } else {
                Rgb::new(30, 70, 255) // Scared blue
            }
        } else {
            match pal {
                0 => {
                    // Classic Arcade: 0=Blinky(Red), 1=Pinky(Pink), 2=Inky(Cyan)
                    match ghost_idx % 3 {
                        0 => Rgb::new(255, 0, 0),
                        1 => Rgb::new(255, 150, 200),
                        _ => Rgb::new(0, 240, 255),
                    }
                }
                1 => {
                    // Neon Cyber
                    match ghost_idx % 3 {
                        0 => Rgb::new(255, 20, 140),
                        1 => Rgb::new(0, 255, 180),
                        _ => Rgb::new(180, 50, 255),
                    }
                }
                2 => {
                    // Retro Amber
                    let b = 180 + (ghost_idx as u8 * 35);
                    Rgb::new(255, b, 20)
                }
                _ => {
                    // Rainbow
                    let h = (color_cycle + ghost_idx as f32 * 0.25) % 1.0;
                    hsv_to_rgb(h, 0.95, 1.0)
                }
            }
        }
    }

    fn draw_pacman(frame: &mut Frame, cx: i32, cy: i32, open_amount: f32, facing_right: bool, pal: usize, color_cycle: f32) {
        let radius = 4i32;
        let r_sq = radius * radius;

        let col = if pal == 3 {
            hsv_to_rgb(color_cycle, 1.0, 1.0)
        } else if pal == 2 {
            Rgb::new(255, 180, 0)
        } else {
            Rgb::new(255, 230, 0) // Classic Yellow
        };

        let mouth_angle = open_amount * 0.9; // 0.0..0.9 rad

        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let d2 = dx * dx + dy * dy;
                if d2 <= r_sq {
                    let px = cx + dx;
                    let py = cy + dy;

                    let ang = (dy as f32).atan2(if facing_right { dx as f32 } else { -dx as f32 });
                    let in_mouth = ang.abs() < mouth_angle && (dx != 0 || dy != 0);

                    if !in_mouth && px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                        frame.set(px as usize, py as usize, col);
                    }
                }
            }
        }
    }

    fn draw_ghost(frame: &mut Frame, cx: i32, cy: i32, color: Rgb, leg_frame: usize, look_right: bool) {
        // Ghost shape: 7x7 pixels (top rounded dome, wavy bottom tentacles)
        let top_y = cy - 3;
        let left_x = cx - 3;

        for dy in 0..7i32 {
            for dx in 0..7i32 {
                let px = left_x + dx;
                let py = top_y + dy;

                if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                    // Dome shape
                    let is_dome = match dy {
                        0 => (2..=4).contains(&dx),
                        1 => (1..=5).contains(&dx),
                        2..=4 => true,
                        5 => true,
                        _ => {
                            // Wavy skirt tentacles alternating per leg_frame
                            if leg_frame == 0 {
                                dx == 0 || dx == 2 || dx == 4 || dx == 6
                            } else {
                                dx == 1 || dx == 3 || dx == 5
                            }
                        }
                    };

                    if is_dome {
                        // Eyes
                        let eye1_x = if look_right { 3 } else { 1 };
                        let eye2_x = if look_right { 5 } else { 3 };
                        let is_white = dy == 2 && (dx == eye1_x || dx == eye1_x + 1 || dx == eye2_x || dx == eye2_x + 1);
                        let is_pupil = dy == 2 && (if look_right { dx == eye1_x + 1 || dx == eye2_x + 1 } else { dx == eye1_x || dx == eye2_x });

                        let pixel_col = if is_pupil {
                            Rgb::new(0, 0, 180)
                        } else if is_white {
                            Rgb::WHITE
                        } else {
                            color
                        };

                        frame.set(px as usize, py as usize, pixel_col);
                    }
                }
            }
        }
    }
}

impl Effect for PacmanChaseEffect {
    fn id(&self) -> &'static str {
        "pacman_chase"
    }

    fn name(&self) -> &'static str {
        "Pacman Retro Chase"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "ghosts" => Some(self.values[1]),
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
            "ghosts" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.travel_x = 0.0;
        self.mouth_phase = 0.0;
        self.scared_timer = 0.0;
        self.color_cycle = 0.0;
        self.power_pellet_flash = 0.0;
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

        // Beat triggers Power Pellet Mode (Ghosts turn scared blue and Pacman chases them!)
        if beat {
            self.scared_timer = 3.5;
            self.power_pellet_flash = 1.0;
        } else {
            self.scared_timer = (self.scared_timer - dt).max(0.0);
            self.power_pellet_flash = (self.power_pellet_flash - dt * 3.0).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.7);
        let cycle_len = WIDTH as f32 + 36.0;

        self.travel_x = (self.travel_x + dt * speed * 18.0) % cycle_len;
        self.mouth_phase = (self.mouth_phase + dt * speed * 20.0) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.18) % 1.0;

        let pal = self.values[2] as usize;
        let ghost_count = (self.values[1].round() as usize).clamp(1, 3);
        let is_scared = self.scared_timer > 0.0;
        #[allow(clippy::manual_is_multiple_of)]
        let flash_white = is_scared && self.scared_timer < 1.2 && ((self.scared_timer * 8.0).floor() as usize % 2 == 0);

        // Maze corridor walls at top (y=6) and bottom (y=26)
        let wall_col = if pal == 1 {
            Rgb::new(20, 40, 240)
        } else if pal == 2 {
            Rgb::new(140, 80, 0)
        } else {
            Rgb::new(30, 30, 220) // Classic Blue Maze Wall
        };

        for x in 0..WIDTH {
            frame.set(x, 6, wall_col);
            frame.set(x, 26, wall_col);
        }

        let corridor_y = 16i32;

        // Draw dots / pellets along the corridor
        let pellet_color = Rgb::new(to_byte(250.0 + mid * 5.0), to_byte(185.0 + treble * 70.0), 160);
        for dot_x in (2..WIDTH).step_by(5) {
            let px = dot_x as i32;
            let dot_eaten = if !is_scared {
                (self.travel_x - 18.0) as i32 > px
            } else {
                false
            };

            if !dot_eaten {
                frame.set(dot_x, corridor_y as usize, pellet_color);
            }
        }

        // Draw glowing power pellet at x=28 if flashing
        if (self.color_cycle * 6.0).sin() > 0.0 {
            let pp_col = Rgb::new(255, 230, 180);
            frame.set(28, (corridor_y - 1) as usize, pp_col);
            frame.set(28, (corridor_y + 1) as usize, pp_col);
            frame.set(27, corridor_y as usize, pp_col);
            frame.set(29, corridor_y as usize, pp_col);
        }

        let mouth_open = 0.5 + 0.5 * self.mouth_phase.sin();
        let leg_frame = ((self.travel_x * 0.4).floor() as usize) % 2;

        if !is_scared {
            // Normal mode: Ghosts chase Pacman (Pacman ahead at travel_x - 10, ghosts follow behind)
            let pac_x = (self.travel_x - 10.0).round() as i32;
            Self::draw_pacman(frame, pac_x, corridor_y, mouth_open, true, pal, self.color_cycle);

            for g in 0..ghost_count {
                let ghost_x = (self.travel_x - 22.0 - (g as f32 * 11.0)).round() as i32;
                let col = Self::ghost_color(pal, g, false, false, self.color_cycle);
                Self::draw_ghost(frame, ghost_x, corridor_y, col, leg_frame, true);
            }
        } else {
            // Power Pellet mode: Pacman chases scared ghosts!
            let pac_x = (self.travel_x - 30.0).round() as i32;
            Self::draw_pacman(frame, pac_x, corridor_y, mouth_open, true, pal, self.color_cycle);

            for g in 0..ghost_count {
                let ghost_x = (self.travel_x - 8.0 + (g as f32 * 10.0)).round() as i32;
                let col = Self::ghost_color(pal, g, true, flash_white, self.color_cycle);
                Self::draw_ghost(frame, ghost_x, corridor_y, col, leg_frame, false);
            }
        }
    }
}
