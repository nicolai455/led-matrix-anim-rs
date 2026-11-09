use crate::util::{audio_bass, hsv_to_rgb, XorShift32};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const PALETTES: &[&str] = &["Classic Arcade", "Cyber Neon", "Laser Green", "Rainbow"];
const ALIEN_ROWS: usize = 2;
const ALIEN_COLUMNS: usize = 4;
const ALIEN_COUNT: usize = ALIEN_ROWS * ALIEN_COLUMNS;
const ALIEN_ROW_SPACING: i32 = 6;
const ALIEN_FORMATION_START_Y: f32 = 3.0;
const ALIEN_MAX_FORMATION_Y: f32 =
    HEIGHT as f32 - 2.0 - 3.0 - (ALIEN_ROWS as f32 - 1.0) * 6.0 - 4.0;

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
    aliens_alive: [bool; ALIEN_COUNT],
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
            march_y: ALIEN_FORMATION_START_Y,
            step_timer: 0.0,
            anim_frame: 0,
            aliens_alive: [true; ALIEN_COUNT],
            lasers: [Laser {
                x: 0.0,
                y: 0.0,
                vy: 0.0,
                is_player: false,
                active: false,
            }; 12],
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

                if px >= 0
                    && px < WIDTH as i32
                    && py >= 0
                    && py < HEIGHT as i32
                    && Self::alien_pixel_on(alien_type, anim_frame, dx, dy)
                {
                    frame.set(px as usize, py as usize, color);
                }
            }
        }
    }

    fn alien_pixel_on(alien_type: usize, anim_frame: usize, dx: i32, dy: i32) -> bool {
        match alien_type {
            0 => match (dy, anim_frame) {
                (0, _) => dx == 2,
                (1, _) => (1..=3).contains(&dx),
                (2, _) => dx == 0 || dx == 2 || dx == 4,
                (3, 0) => dx == 1 || dx == 3,
                _ => dx == 0 || dx == 4,
            },
            1 => match (dy, anim_frame) {
                (0, _) => dx == 1 || dx == 3,
                (1, _) => true,
                (2, _) => dx != 2,
                (3, 0) => dx == 0 || dx == 4,
                _ => dx == 1 || dx == 3,
            },
            _ => match (dy, anim_frame) {
                (0, _) => (1..=3).contains(&dx),
                (1, _) => true,
                (2, _) => dx == 0 || dx == 2 || dx == 4,
                (3, 0) => dx == 1 || dx == 3,
                _ => dx == 0 || dx == 4,
            },
        }
    }

    fn intersected_alien(
        alive: &[bool; ALIEN_COUNT],
        march_x: f32,
        base_march_y: i32,
        anim_frame: usize,
        laser_x: f32,
        previous_y: f32,
        current_y: f32,
    ) -> Option<usize> {
        let laser_x = laser_x.round() as i32;
        let path_min_y = previous_y.min(current_y);
        let path_max_y = previous_y.max(current_y);

        for row in 0..ALIEN_ROWS {
            let alien_y = base_march_y + row as i32 * ALIEN_ROW_SPACING;
            for col in 0..ALIEN_COLUMNS {
                let alien_index = row * ALIEN_COLUMNS + col;
                if !alive[alien_index] {
                    continue;
                }
                let alien_x = (march_x + col as f32 * 6.5).round() as i32;

                for dy in 0..4 {
                    for dx in 0..5 {
                        let pixel_x = alien_x + dx;
                        let pixel_y = alien_y + dy;
                        if pixel_x == laser_x
                            && Self::alien_pixel_on(row as usize, anim_frame, dx, dy)
                            && path_max_y >= pixel_y as f32 - 0.5
                            && path_min_y <= pixel_y as f32 + 0.5
                        {
                            return Some(alien_index);
                        }
                    }
                }
            }
        }
        None
    }

    fn alien_color(pal: usize, row: usize, color_cycle: f32) -> Rgb {
        match pal {
            0 => match row {
                0 => Rgb::new(255, 30, 80), // Red top-row squid
                _ => Rgb::new(0, 240, 255), // Cyan second-row crab
            },
            1 => {
                let base_hue = 0.45 + row as f32 * 0.1;
                hsv_to_rgb((base_hue + color_cycle * 0.2) % 1.0, 1.0, 1.0)
            }
            2 => match row {
                0 => Rgb::new(255, 0, 255),
                _ => Rgb::new(255, 255, 0),
            },
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
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> {
        Some(&PARAMETERS)
    }

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
        self.march_y = ALIEN_FORMATION_START_Y;
        self.step_timer = 0.0;
        self.anim_frame = 0;
        self.aliens_alive = [true; ALIEN_COUNT];
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
        let bass = if audio_active {
            audio_bass(context.audio)
        } else {
            0.0
        };
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
                self.march_y += 1.0;
            } else if self.march_x < 1.0 {
                self.march_x = 1.0;
                self.march_dir = 1.0;
                self.march_y += 1.0;
            }
        }
        if self.aliens_alive.iter().all(|alive| !alive) || self.march_y >= ALIEN_MAX_FORMATION_Y {
            self.march_x = 2.0;
            self.march_dir = 1.0;
            self.march_y = ALIEN_FORMATION_START_Y;
            self.step_timer = 0.0;
            self.anim_frame = 0;
            self.aliens_alive = [true; ALIEN_COUNT];
            for laser in &mut self.lasers {
                if !laser.is_player {
                    laser.active = false;
                }
            }
        }

        let pal = self.values[2] as usize;
        let base_march_y = self.march_y as i32 + 2;
        let player_x = 13 + ((self.march_x * 2.2).sin() * 8.0).round() as i32;

        // Update shots and resolve impacts before drawing, so dead aliens do not
        // appear for one extra frame and fast shots cannot tunnel through them.
        let laser_rate = self.values[1] / 50.0;
        if (beat || self.rng.next_f32() < dt * 3.5 * laser_rate)
            && !self
                .lasers
                .iter()
                .any(|laser| laser.active && laser.is_player)
        {
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

        let alive_count = self.aliens_alive.iter().filter(|alive| **alive).count();
        if alive_count > 0 && self.rng.next_f32() < dt * 4.0 * laser_rate {
            let target = (self.rng.next_f32() * alive_count as f32) as usize;
            if let Some((alien_index, _)) = self
                .aliens_alive
                .iter()
                .enumerate()
                .filter(|(_, alive)| **alive)
                .nth(target)
            {
                let row = alien_index / ALIEN_COLUMNS;
                let col = alien_index % ALIEN_COLUMNS;
                let alien_x = (self.march_x + col as f32 * 6.5).round() as i32;
                let spawn_dx = (0..5)
                    .find(|&dx| Self::alien_pixel_on(row, self.anim_frame, dx, 3))
                    .unwrap_or(2);
                let spawn_y = base_march_y + row as i32 * ALIEN_ROW_SPACING + 3;
                if let Some(slot) = self.lasers.iter_mut().find(|laser| !laser.active) {
                    slot.active = true;
                    slot.x = (alien_x + spawn_dx) as f32;
                    slot.y = spawn_y as f32;
                    slot.vy = 22.0;
                    slot.is_player = false;
                }
            }
        }

        for laser in &mut self.lasers {
            if !laser.active {
                continue;
            }
            let previous_y = laser.y;
            let current_y = previous_y + laser.vy * dt;
            if laser.is_player {
                if let Some(alien_index) = Self::intersected_alien(
                    &self.aliens_alive,
                    self.march_x,
                    base_march_y,
                    self.anim_frame,
                    laser.x,
                    previous_y,
                    current_y,
                ) {
                    self.aliens_alive[alien_index] = false;
                    laser.active = false;
                    continue;
                }
            }
            laser.y = current_y;
            if laser.x.round() < 0.0
                || laser.x.round() >= WIDTH as f32
                || laser.y.round() < 0.0
                || laser.y.round() >= HEIGHT as f32
            {
                laser.active = false;
            }
        }

        for row in 0..ALIEN_ROWS {
            let row_y = base_march_y + row as i32 * ALIEN_ROW_SPACING;

            for col in 0..ALIEN_COLUMNS {
                let alien_index = row * ALIEN_COLUMNS + col;
                if self.aliens_alive[alien_index] {
                    let alien_x = (self.march_x + (col as f32 * 6.5)).round() as i32;
                    let col_color = Self::alien_color(pal, row, self.color_cycle);
                    Self::draw_alien_sprite(frame, row, self.anim_frame, alien_x, row_y, col_color);
                }
            }
        }

        // 3. Draw Player Cannon at Bottom (y=29, Width 5, Height 3)
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

        // 4. Draw active lasers after targets so shots remain visible in-frame.
        for laser in &self.lasers {
            if !laser.active {
                continue;
            }
            let lx = laser.x.round() as i32;
            let ly = laser.y.round() as i32;
            if lx >= 0 && lx < WIDTH as i32 && ly >= 0 && ly < HEIGHT as i32 {
                let col = if laser.is_player {
                    Rgb::new(255, 255, 100)
                } else {
                    Rgb::new(255, 40, 50)
                };
                frame.set(lx as usize, ly as usize, col);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matrix_core::AudioSnapshot;

    #[test]
    fn green_bunker_shapes_are_absent_but_player_cannon_remains() {
        let mut effect = ArcadeInvadersEffect::default();
        effect.values[1] = 0.0;
        effect.values[2] = 0.0;
        effect.values[3] = 0.0;
        let audio = AudioSnapshot::default();
        let context = RenderContext {
            now_ms: 0,
            delta_seconds: 0.1,
            audio: &audio,
            color: Rgb::WHITE,
            text: "",
            scroll_speed: 1.0,
        };
        let mut frame = Frame::new();

        effect.render(&mut frame, &context);

        for x in [4, 5, 7, 8, 13, 14, 16, 17, 22, 23, 25, 26] {
            for y in 23..=25 {
                assert_eq!(
                    frame.get(x, y),
                    Rgb::BLACK,
                    "bunker pixel remains at ({x},{y})"
                );
            }
        }
        assert_eq!(
            frame.get(13, 20),
            Rgb::BLACK,
            "bunker shape is no longer painted on the playfield"
        );
        let player_x = 13 + ((effect.march_x * 2.2).sin() * 8.0).round() as i32;
        assert_eq!(
            frame.get((player_x + 2) as usize, 28),
            Rgb::new(50, 255, 80)
        );
    }

    #[test]
    fn rendered_game_contains_two_alien_rows_and_no_green_lower_enemy_row() {
        let mut effect = ArcadeInvadersEffect::default();
        effect.values[1] = 0.0;
        effect.values[2] = 0.0;
        effect.values[3] = 0.0;
        let audio = AudioSnapshot::default();
        let context = RenderContext {
            now_ms: 0,
            delta_seconds: 0.001,
            audio: &audio,
            color: Rgb::WHITE,
            text: "",
            scroll_speed: 1.0,
        };
        let mut frame = Frame::new();
        effect.render(&mut frame, &context);

        let green = Rgb::new(50, 255, 100);
        assert!(
            !frame.pixels.contains(&green),
            "legacy green invader row remains"
        );
        assert_eq!(
            frame.get(3, 17),
            Rgb::BLACK,
            "removed lower invader row must not be drawn"
        );
        assert!(
            frame
                .pixels
                .iter()
                .any(|pixel| *pixel == Rgb::new(50, 255, 80)),
            "player cannon should remain green at the bottom"
        );
    }

    #[test]
    fn enemy_shot_starts_at_the_bottom_pixel_of_a_live_alien() {
        let mut effect = ArcadeInvadersEffect::default();
        effect.values[1] = 50.0;
        effect.values[2] = 0.0;
        effect.values[3] = 0.0;
        effect.aliens_alive = [false; ALIEN_COUNT];
        effect.aliens_alive[0] = true;
        let dt = 0.1;
        let rate_threshold = dt * 4.0;
        let seed = (1..10_000)
            .find(|seed| {
                let mut rng = XorShift32::new(*seed);
                rng.next_f32() >= dt * 3.5
                    && rng.next_f32() < rate_threshold
                    && (rng.next_f32() * ALIEN_COUNT as f32) as usize == 0
            })
            .expect("choose the sole living alien without firing the player's weapon first");
        effect.rng = XorShift32::new(seed);
        let audio = AudioSnapshot::default();
        let context = RenderContext {
            now_ms: 0,
            delta_seconds: dt,
            audio: &audio,
            color: Rgb::WHITE,
            text: "",
            scroll_speed: 1.0,
        };
        let mut frame = Frame::new();

        effect.render(&mut frame, &context);

        let alien_x = effect.march_x.round() as i32;
        let spawn_pixel = (0..5)
            .find(|dx| ArcadeInvadersEffect::alien_pixel_on(0, effect.anim_frame, *dx, 3))
            .unwrap();
        let laser = effect
            .lasers
            .iter()
            .find(|laser| laser.active && !laser.is_player)
            .unwrap();
        assert_eq!(laser.x, (alien_x + spawn_pixel) as f32);
        assert!(
            laser.y >= (effect.march_y as i32 + 2 + 3) as f32,
            "enemy laser must originate at the live alien's lower edge"
        );
        assert!(
            laser.y < 16.0,
            "enemy laser must not originate near the cannon"
        );
    }

    #[test]
    fn emptied_or_descended_formation_restarts_at_top() {
        let mut effect = ArcadeInvadersEffect::default();
        effect.aliens_alive = [false; ALIEN_COUNT];
        effect.values[1] = 0.0;
        let audio = AudioSnapshot::default();
        let context = RenderContext {
            now_ms: 0,
            delta_seconds: 0.1,
            audio: &audio,
            color: Rgb::WHITE,
            text: "",
            scroll_speed: 1.0,
        };
        let mut frame = Frame::new();
        effect.render(&mut frame, &context);
        assert!(effect.aliens_alive.iter().all(|alive| *alive));

        effect.march_y = ALIEN_MAX_FORMATION_Y;
        effect.render(&mut frame, &context);
        assert_eq!(effect.march_y, ALIEN_FORMATION_START_Y);
        assert!(effect.aliens_alive.iter().all(|alive| *alive));
    }

    #[test]
    fn formation_resets_before_the_cannon() {
        let mut effect = ArcadeInvadersEffect::default();
        effect.values[1] = 0.0;
        effect.values[3] = 0.0;
        effect.march_dir = 0.0;
        effect.march_y = ALIEN_MAX_FORMATION_Y;
        let audio = AudioSnapshot::default();
        let context = RenderContext {
            now_ms: 0,
            delta_seconds: 0.1,
            audio: &audio,
            color: Rgb::WHITE,
            text: "",
            scroll_speed: 1.0,
        };
        let mut frame = Frame::new();

        effect.render(&mut frame, &context);

        assert_eq!(effect.march_y, ALIEN_FORMATION_START_Y);
        assert!(effect.aliens_alive.iter().all(|alive| *alive));
    }

    #[test]
    fn formation_descends_on_each_edge_reversal_then_resets_before_the_cannon() {
        let mut effect = ArcadeInvadersEffect::default();
        effect.values[1] = 0.0;
        effect.values[3] = 0.0;
        effect.march_x = 6.0;
        effect.march_dir = 1.0;
        effect.step_timer = 1.0;
        let audio = AudioSnapshot::default();
        let context = RenderContext {
            now_ms: 0,
            delta_seconds: 0.1,
            audio: &audio,
            color: Rgb::WHITE,
            text: "",
            scroll_speed: 1.0,
        };
        let mut frame = Frame::new();

        for expected_y in 4..ALIEN_MAX_FORMATION_Y as i32 {
            effect.march_x = 6.0;
            effect.march_dir = 1.0;
            effect.step_timer = 0.0;
            for _ in 0..4 {
                effect.render(&mut frame, &context);
            }
            assert_eq!(effect.march_y, expected_y as f32);
        }

        effect.march_x = 6.0;
        effect.march_dir = 1.0;
        effect.step_timer = 0.0;
        for _ in 0..4 {
            effect.render(&mut frame, &context);
        }
        assert_eq!(effect.march_y, ALIEN_FORMATION_START_Y);
        assert!(effect.aliens_alive.iter().all(|alive| *alive));
    }

    #[test]
    fn player_laser_destroys_alien_before_the_hit_frame_is_drawn() {
        let mut effect = ArcadeInvadersEffect::default();
        effect.values[3] = 0.0;
        effect.march_x = 2.0;
        effect.march_dir = 0.0;
        effect.lasers[0] = Laser {
            x: 4.0,
            y: 8.0,
            vy: -35.0,
            is_player: true,
            active: true,
        };
        let audio = AudioSnapshot::default();
        let context = RenderContext {
            now_ms: 0,
            delta_seconds: 0.1,
            audio: &audio,
            color: Rgb::WHITE,
            text: "",
            scroll_speed: 1.0,
        };
        let mut frame = Frame::new();

        effect.render(&mut frame, &context);

        assert!(
            !effect.lasers[0].active,
            "a laser crossing an alien should be consumed on impact"
        );
        assert_eq!(
            frame.get(4, 5),
            Rgb::BLACK,
            "the destroyed alien must not be drawn over or under its impact"
        );

        effect.render(&mut frame, &context);

        assert_eq!(
            frame.get(4, 5),
            Rgb::BLACK,
            "a destroyed alien must stay absent on following frames"
        );

        effect.reset();
        effect.render(&mut frame, &context);
        assert_ne!(
            frame.get(4, 5),
            Rgb::BLACK,
            "reset should restore destroyed aliens"
        );
    }
}
