use crate::util::{alloc_boxed_2d, audio_bass, hsv_to_rgb, to_byte, XorShift32};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const PALETTES: &[&str] = &["Emerald Matrix", "Neon Synth", "Solar Flare", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Speed",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 60.0,
        step: 1.0,
        default: 15.0,
        options: &[],
        unit: "gen/s",
    },
    ParameterDescriptor {
        id: "density",
        label: "Initial density",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 70.0,
        step: 1.0,
        default: 30.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "decay",
        label: "Decay fade",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 90.0,
        step: 1.0,
        default: 60.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Palette",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 3.0,
        step: 1.0,
        default: 3.0,
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

pub struct GameOfLifeEffect {
    values: [f32; 5],
    grid: Box<[[u8; WIDTH]; HEIGHT]>,
    next_grid: Box<[[u8; WIDTH]; HEIGHT]>,
    age: Box<[[u8; WIDTH]; HEIGHT]>,
    decay_buffer: Box<[[Rgb; WIDTH]; HEIGHT]>,
    accumulator: f32,
    color_cycle: f32,
    rng: XorShift32,
}

impl Default for GameOfLifeEffect {
    fn default() -> Self {
        let mut inst = Self {
            values: [15.0, 30.0, 60.0, 3.0, 1.0],
            grid: alloc_boxed_2d(0),
            next_grid: alloc_boxed_2d(0),
            age: alloc_boxed_2d(0),
            decay_buffer: alloc_boxed_2d(Rgb::BLACK),
            accumulator: 0.0,
            color_cycle: 0.0,
            rng: XorShift32::new(1234567),
        };
        inst.randomize();
        inst
    }
}

impl GameOfLifeEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn randomize(&mut self) {
        let thresh = self.values[1] / 100.0;
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                if self.rng.next_f32() < thresh {
                    self.grid[y][x] = 1;
                    self.age[y][x] = 1;
                } else {
                    self.grid[y][x] = 0;
                    self.age[y][x] = 0;
                }
            }
        }
    }

    fn seed_glider_cluster(&mut self, cx: usize, cy: usize) {
        for dy in 0..3 {
            for dx in 0..3 {
                let gx = (cx + dx) % WIDTH;
                let gy = (cy + dy) % HEIGHT;
                if self.rng.next_f32() < 0.6 {
                    self.grid[gy][gx] = 1;
                    self.age[gy][gx] = 1;
                }
            }
        }
    }

    fn cell_color(pal: usize, color_cycle: f32, age: u8, x: usize, y: usize) -> Rgb {
        let age_norm = (age as f32 / 30.0).min(1.0);

        match pal {
            0 => {
                // Emerald Matrix
                let g = to_byte(255.0 * (1.0 - age_norm * 0.3));
                let r = to_byte(50.0 * age_norm);
                let b = to_byte(70.0 * (1.0 - age_norm));
                Rgb::new(r, g, b)
            }
            1 => {
                // Neon Synth
                if age < 4 {
                    Rgb::new(255, 255, 255)
                } else if age < 15 {
                    Rgb::new(255, 30, 180)
                } else {
                    Rgb::new(40, 180, 255)
                }
            }
            2 => {
                // Solar Flare
                let r = 255;
                let g = to_byte(240.0 * (1.0 - age_norm * 0.7));
                let b = to_byte(30.0 * (1.0 - age_norm));
                Rgb::new(r, g, b)
            }
            _ => {
                // Rainbow
                let pos_norm = ((x + y) as f32 / (WIDTH + HEIGHT) as f32) * 0.5;
                let h = (color_cycle + pos_norm + age_norm * 0.25) % 1.0;
                hsv_to_rgb(h, 0.95, 1.0)
            }
        }
    }
}

impl Effect for GameOfLifeEffect {
    fn id(&self) -> &'static str {
        "game_of_life"
    }

    fn name(&self) -> &'static str {
        "Game of Life"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "density" => Some(self.values[1]),
            "decay" => Some(self.values[2]),
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
            "density" => self.values[1] = value,
            "decay" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.accumulator = 0.0;
        self.color_cycle = 0.0;
        self.decay_buffer = alloc_boxed_2d(Rgb::BLACK);
        self.randomize();
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[4] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        // On audio beat, inject fresh life clusters
        if beat {
            let cx = (self.rng.next_f32() * (WIDTH - 4) as f32) as usize;
            let cy = (self.rng.next_f32() * (HEIGHT - 4) as f32) as usize;
            self.seed_glider_cluster(cx, cy);
        }

        // Speed calculation
        let gen_rate = self.values[0] * (1.0 + bass * 0.8);
        let step_time = 1.0 / gen_rate.max(1.0);
        self.accumulator += dt;

        while self.accumulator >= step_time {
            self.accumulator -= step_time;

            // Compute next generation
            let mut live_count = 0usize;
            for y in 0..HEIGHT {
                let ym = if y == 0 { HEIGHT - 1 } else { y - 1 };
                let yp = if y + 1 == HEIGHT { 0 } else { y + 1 };
                for x in 0..WIDTH {
                    let xm = if x == 0 { WIDTH - 1 } else { x - 1 };
                    let xp = if x + 1 == WIDTH { 0 } else { x + 1 };

                    let neighbors = self.grid[ym][xm]
                        + self.grid[ym][x]
                        + self.grid[ym][xp]
                        + self.grid[y][xm]
                        + self.grid[y][xp]
                        + self.grid[yp][xm]
                        + self.grid[yp][x]
                        + self.grid[yp][xp];

                    let alive = self.grid[y][x];
                    if alive == 1 {
                        if neighbors == 2 || neighbors == 3 {
                            self.next_grid[y][x] = 1;
                            self.age[y][x] = self.age[y][x].saturating_add(1);
                            live_count += 1;
                        } else {
                            self.next_grid[y][x] = 0;
                            self.age[y][x] = 0;
                        }
                    } else if neighbors == 3 {
                        self.next_grid[y][x] = 1;
                        self.age[y][x] = 1;
                        live_count += 1;
                    } else {
                        self.next_grid[y][x] = 0;
                        self.age[y][x] = 0;
                    }
                }
            }

            // Swap buffers
            std::mem::swap(&mut self.grid, &mut self.next_grid);

            // Re-seed if grid died out or stagnated
            if live_count < 15 {
                self.randomize();
            }
        }

        // Phosphor decay for dead cells
        let decay_rate = (self.values[2] / 100.0).clamp(0.1, 0.95);
        let decay_mult = decay_rate.powf(dt * 20.0);
        let pal = self.values[3] as usize;
        let color_cycle = self.color_cycle;

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let current = &mut self.decay_buffer[y][x];
                if self.grid[y][x] == 1 {
                    let col = Self::cell_color(pal, color_cycle, self.age[y][x], x, y);
                    *current = col;
                } else {
                    current.r = (current.r as f32 * decay_mult) as u8;
                    current.g = (current.g as f32 * decay_mult) as u8;
                    current.b = (current.b as f32 * decay_mult) as u8;
                }
                frame.set(x, y, *current);
            }
        }
    }
}
