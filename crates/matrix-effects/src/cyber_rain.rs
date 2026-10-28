use crate::util::{alloc_boxed_2d, audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte, XorShift32};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const PALETTES: &[&str] = &["Classic Green", "Cyan Zion", "Blood Red", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Stream speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 45.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "density",
        label: "Rain density",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 90.0,
        step: 1.0,
        default: 50.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "trail",
        label: "Phosphor trail",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 95.0,
        step: 1.0,
        default: 75.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Code color",
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
struct ColumnDrop {
    head_y: f32,
    speed: f32,
    active: bool,
    length: f32,
}

pub struct CyberRainEffect {
    values: [f32; 5],
    columns: [ColumnDrop; WIDTH],
    trail_buffer: Box<[[f32; WIDTH]; HEIGHT]>,
    emp_flash: f32,
    color_cycle: f32,
    rng: XorShift32,
}

impl Default for CyberRainEffect {
    fn default() -> Self {
        let mut rng = XorShift32::new(314159);
        let mut columns = [ColumnDrop { head_y: -10.0, speed: 1.0, active: false, length: 10.0 }; WIDTH];

        for col in &mut columns {
            col.head_y = -(rng.next_f32() * 30.0);
            col.speed = 15.0 + rng.next_f32() * 20.0;
            col.active = true;
            col.length = 8.0 + rng.next_f32() * 12.0;
        }

        Self {
            values: [45.0, 50.0, 75.0, 0.0, 1.0],
            columns,
            trail_buffer: alloc_boxed_2d(0.0),
            emp_flash: 0.0,
            color_cycle: 0.0,
            rng,
        }
    }
}

impl CyberRainEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn glyph_color(pal: usize, intensity: f32, is_head: bool, color_cycle: f32, x: usize) -> Rgb {
        let b = clamp01(intensity);
        if is_head {
            return match pal {
                0 => Rgb::new(220, 255, 220), // White-hot green
                1 => Rgb::new(200, 255, 255), // White-hot cyan
                2 => Rgb::new(255, 220, 220), // White-hot crimson
                _ => Rgb::WHITE,
            };
        }

        match pal {
            0 => {
                // Classic Matrix Green
                let r = to_byte(30.0 * b.powi(2));
                let g = to_byte(255.0 * b);
                let blue = to_byte(50.0 * b.powi(2));
                Rgb::new(r, g, blue)
            }
            1 => {
                // Cyan Zion
                let r = to_byte(20.0 * b);
                let g = to_byte(230.0 * b);
                let blue = to_byte(255.0 * b);
                Rgb::new(r, g, blue)
            }
            2 => {
                // Blood Red Matrix
                let r = to_byte(255.0 * b);
                let g = to_byte(20.0 * b.powi(2));
                let blue = to_byte(30.0 * b.powi(2));
                Rgb::new(r, g, blue)
            }
            _ => {
                // Rainbow Stream
                let h = (color_cycle + (x as f32 / WIDTH as f32) * 0.7) % 1.0;
                hsv_to_rgb(h, 0.95, b)
            }
        }
    }
}

impl Effect for CyberRainEffect {
    fn id(&self) -> &'static str {
        "cyber_rain"
    }

    fn name(&self) -> &'static str {
        "Cyber Matrix Rain"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "density" => Some(self.values[1]),
            "trail" => Some(self.values[2]),
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
            "trail" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.trail_buffer = alloc_boxed_2d(0.0);
        self.emp_flash = 0.0;
        self.color_cycle = 0.0;
        for (i, col) in self.columns.iter_mut().enumerate() {
            col.head_y = -(i as f32 * 1.5) % 32.0;
            col.speed = 15.0 + self.rng.next_f32() * 20.0;
            col.active = true;
        }
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::BLACK);

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[4] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let _mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.emp_flash = 1.0;
        } else {
            self.emp_flash = (self.emp_flash - dt * 3.5).max(0.0);
        }

        let speed_mult = (self.values[0] / 45.0) * (1.0 + bass * 0.9);
        self.color_cycle = (self.color_cycle + dt * 0.14) % 1.0;

        let trail_decay = (self.values[2] / 100.0).clamp(0.2, 0.96);
        let decay_mult = trail_decay.powf(dt * 30.0);

        // 1. Phosphor decay in trail buffer
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                self.trail_buffer[y][x] *= decay_mult;
            }
        }

        let density = self.values[1] / 100.0;
        let pal = self.values[3] as usize;

        // 2. Advance falling rain drops
        for x in 0..WIDTH {
            let col = &mut self.columns[x];

            // Speed influenced by column frequency band
            let band_idx = (x * 16 / WIDTH).min(15);
            let band_spd = if audio_active { context.audio.bands[band_idx] * 0.5 } else { 0.0 };

            col.head_y += (col.speed * speed_mult + band_spd * 30.0) * dt;

            let hy = col.head_y.round() as i32;

            if hy >= 0 && hy < HEIGHT as i32 {
                self.trail_buffer[hy as usize][x] = 1.0;
            }

            // Reset drop once past bottom
            if col.head_y > (HEIGHT as f32 + col.length) {
                if self.rng.next_f32() < density {
                    col.head_y = -(self.rng.next_f32() * 12.0);
                    col.speed = 12.0 + self.rng.next_f32() * 24.0;
                    col.length = 6.0 + self.rng.next_f32() * 14.0;
                    col.active = true;
                } else {
                    col.head_y = -10.0;
                }
            }
        }

        // 3. Render Trails & Glyph Flickering
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let heat = self.trail_buffer[y][x];
                if heat > 0.04 {
                    let hy = self.columns[x].head_y.round() as i32;
                    let is_head = (y as i32 - hy).abs() <= 1 && heat > 0.85;

                    // Digital glyph noise shimmer
                    let flicker = if audio_active {
                        (self.rng.next_f32() - 0.5) * treble * 0.4
                    } else {
                        0.0
                    };

                    let brightness = (heat + flicker).clamp(0.0, 1.0);
                    let mut col = Self::glyph_color(pal, brightness, is_head, self.color_cycle, x);

                    if self.emp_flash > 0.3 {
                        let flash_b = to_byte(self.emp_flash * 180.0);
                        col.r = col.r.saturating_add(flash_b);
                        col.g = col.g.saturating_add(flash_b);
                        col.b = col.b.saturating_add(flash_b);
                    }

                    frame.set(x, y, col);
                }
            }
        }
    }
}
