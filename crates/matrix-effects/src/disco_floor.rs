use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, WIDTH,
};

const PATTERNS: &[&str] = &["Checkerboard", "Radial Pulse", "Color Wash", "Audio Waves"];
const PALETTES: &[&str] = &["Saturday Night", "Synthwave 1984", "Funky Grooves", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Groove tempo",
        kind: ParameterKind::Number,
        min: 5.0,
        max: 100.0,
        step: 1.0,
        default: 30.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "pattern",
        label: "Floor pattern",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 3.0,
        step: 1.0,
        default: 0.0,
        options: PATTERNS,
        unit: "",
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
        id: "grid_lines",
        label: "Neon gridlines",
        kind: ParameterKind::Boolean,
        min: 0.0,
        max: 1.0,
        step: 1.0,
        default: 1.0,
        options: &[],
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

pub struct DiscoFloorEffect {
    values: [f32; 5],
    step_time: f32,
    color_cycle: f32,
    bass_pulse: f32,
    beat_flip: bool,
}

impl Default for DiscoFloorEffect {
    fn default() -> Self {
        Self {
            values: [30.0, 0.0, 3.0, 1.0, 1.0],
            step_time: 0.0,
            color_cycle: 0.0,
            bass_pulse: 0.0,
            beat_flip: false,
        }
    }
}

impl DiscoFloorEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn tile_color(pal: usize, color_cycle: f32, tile_x: usize, tile_y: usize, energy: f32) -> Rgb {
        let e = clamp01(energy);
        let coord_norm = (tile_x * 7 + tile_y * 11) as f32 * 0.05;

        match pal {
            0 => {
                // Saturday Night (Red, Yellow, Cyan, Hot Pink)
                let choice = (tile_x + tile_y) % 4;
                match choice {
                    0 => Rgb::new(to_byte(255.0 * e), 0, to_byte(80.0 * e)),
                    1 => Rgb::new(to_byte(255.0 * e), to_byte(220.0 * e), 0),
                    2 => Rgb::new(0, to_byte(230.0 * e), to_byte(255.0 * e)),
                    _ => Rgb::new(to_byte(240.0 * e), to_byte(30.0 * e), to_byte(200.0 * e)),
                }
            }
            1 => {
                // Synthwave 1984 (Neon Magenta, Deep Purple, Laser Cyan)
                let choice = (tile_x ^ tile_y) % 3;
                match choice {
                    0 => Rgb::new(to_byte(255.0 * e), 0, to_byte(180.0 * e)),
                    1 => Rgb::new(to_byte(100.0 * e), 0, to_byte(240.0 * e)),
                    _ => Rgb::new(0, to_byte(220.0 * e), to_byte(240.0 * e)),
                }
            }
            2 => {
                // Funky Grooves (Lime Green, Orange Gold, Electric Blue)
                let choice = (tile_x * 2 + tile_y) % 3;
                match choice {
                    0 => Rgb::new(to_byte(60.0 * e), to_byte(255.0 * e), 0),
                    1 => Rgb::new(to_byte(255.0 * e), to_byte(150.0 * e), 0),
                    _ => Rgb::new(0, to_byte(120.0 * e), to_byte(255.0 * e)),
                }
            }
            _ => {
                // Rainbow Fade
                let h = (color_cycle + coord_norm) % 1.0;
                hsv_to_rgb(h, 0.95, e)
            }
        }
    }
}

impl Effect for DiscoFloorEffect {
    fn id(&self) -> &'static str {
        "disco_floor"
    }

    fn name(&self) -> &'static str {
        "80s Disco Floor"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "pattern" => Some(self.values[1]),
            "palette" => Some(self.values[2]),
            "grid_lines" => Some(self.values[3]),
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
            "pattern" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "grid_lines" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.step_time = 0.0;
        self.color_cycle = 0.0;
        self.bass_pulse = 0.0;
        self.beat_flip = false;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[4] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.beat_flip = !self.beat_flip;
            self.bass_pulse = 1.0;
        } else {
            self.bass_pulse = (self.bass_pulse - dt * 3.0).max(0.0);
        }

        let speed = self.values[0] / 30.0;
        self.step_time += dt * speed * 4.0;
        self.color_cycle = (self.color_cycle + dt * 0.16) % 1.0;

        let pal = self.values[2] as usize;
        let pattern_mode = self.values[1] as usize;
        let show_grid = self.values[3] > 0.5;

        // 8x8 tiles of 4x4 pixels each
        const TILE_SIZE: usize = 4;
        const TILES_PER_ROW: usize = WIDTH / TILE_SIZE; // 8

        let current_step = self.step_time.floor() as usize;

        for ty in 0..TILES_PER_ROW {
            let dy = ty as f32 - 3.5;
            for tx in 0..TILES_PER_ROW {
                let dx = tx as f32 - 3.5;
                let dist = (dx * dx + dy * dy).sqrt();

                let (tile_energy, active_tx, active_ty) = match pattern_mode {
                    0 => {
                        // Checkerboard with beat flip
                        let is_even = (tx + ty) % 2 == 0;
                        let active = if self.beat_flip { !is_even } else { is_even };
                        let pulse = if active { 0.95f32 } else { 0.25f32 };
                        #[allow(clippy::manual_is_multiple_of)]
                        let extra = if (current_step + tx) % 3 == 0 { 0.15f32 } else { 0.0f32 };
                        ((pulse + extra).min(1.0f32), tx + current_step, ty)
                    }
                    1 => {
                        // Radial Pulse outward
                        let ring = ((dist * 1.5 - self.step_time * 1.2).sin() * 0.5 + 0.5).powf(2.0);
                        let e = ring * (1.0 + self.bass_pulse * 0.5) + bass * 0.3;
                        (e.clamp(0.2, 1.0), tx, ty + current_step)
                    }
                    2 => {
                        // Color Wash Sweep
                        let sweep = ((tx as f32 * 0.5 + ty as f32 * 0.5 - self.step_time).sin() * 0.5 + 0.5).powf(1.5);
                        (sweep.clamp(0.25, 1.0), tx + current_step, ty)
                    }
                    _ => {
                        // Audio Waves (bands mapped across floor columns)
                        let band_val = if audio_active {
                            context.audio.bands[(tx * 2) % 16]
                        } else {
                            0.5 + 0.5 * ((tx as f32 + self.step_time).sin())
                        };
                        let row_thresh = (7 - ty) as f32 / 7.0;
                        let active = band_val >= row_thresh;
                        let e = if active { 1.0 } else { 0.15 };
                        (e, tx, ty)
                    }
                };

                let color = Self::tile_color(pal, self.color_cycle, active_tx, active_ty, tile_energy);

                // Render 4x4 tile pixels
                let px_start = tx * TILE_SIZE;
                let py_start = ty * TILE_SIZE;

                for py in 0..TILE_SIZE {
                    let y = py_start + py;
                    for px in 0..TILE_SIZE {
                        let x = px_start + px;

                        // Gridline check
                        let is_grid = show_grid && (px == 0 || py == 0);
                        if is_grid {
                            let grid_bright = to_byte(40.0 + treble * 120.0 + self.bass_pulse * 90.0);
                            frame.set(x, y, Rgb::new(grid_bright, grid_bright, grid_bright));
                        } else {
                            // Tile interior with subtle bevel / center brightness
                            let is_center = (1..=2).contains(&px) && (1..=2).contains(&py);
                            let boost = if is_center { 1.15 + mid * 0.2 } else { 0.85 };
                            let r = to_byte(color.r as f32 * boost);
                            let g = to_byte(color.g as f32 * boost);
                            let b = to_byte(color.b as f32 * boost);
                            frame.set(x, y, Rgb::new(r, g, b));
                        }
                    }
                }
            }
        }
    }
}
