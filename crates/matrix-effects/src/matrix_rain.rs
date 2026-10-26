use crate::util::{alloc_boxed_2d, clamp01, to_byte, XorShift32};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const COLOR_MODES: &[&str] = &["Green", "Blue", "Purple"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "Speed",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 100.0,
        step: 1.0,
        default: 40.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "density",
        label: "Density",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 100.0,
        step: 1.0,
        default: 50.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "color_mode",
        label: "Color",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 2.0,
        step: 1.0,
        default: 0.0,
        options: COLOR_MODES,
        unit: "",
    },
    ParameterDescriptor {
        id: "audio_reactive",
        label: "Audio reactive",
        kind: ParameterKind::Boolean,
        min: 0.0,
        max: 1.0,
        step: 1.0,
        default: 0.0,
        options: &[],
        unit: "",
    },
];

#[derive(Clone, Copy, Debug)]
struct Drop {
    head_y: f32,
    speed: f32,
    length: usize,
    active: bool,
}

pub struct MatrixRainEffect {
    values: [f32; 4],
    drops: Box<[Drop; WIDTH]>,
    trail: Box<[[f32; WIDTH]; HEIGHT]>,
    rng: XorShift32,
    flash: f32,
}

impl Default for MatrixRainEffect {
    fn default() -> Self {
        let drops = vec![
            Drop {
                head_y: -1.0,
                speed: 10.0,
                length: 8,
                active: false,
            };
            WIDTH
        ]
        .into_boxed_slice()
        .try_into()
        .unwrap();
        Self {
            values: [40.0, 50.0, 0.0, 0.0],
            drops,
            trail: alloc_boxed_2d(0.0),
            rng: XorShift32::new(0xCAFEBABE),
            flash: 0.0,
        }
    }
}

impl MatrixRainEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn spawn_drop(&mut self, col: usize) {
        self.drops[col].head_y = -1.0;
        let speed_base = 8.0 + self.values[0] * 0.3;
        self.drops[col].speed = speed_base * (0.7 + self.rng.next_f32() * 0.6);
        self.drops[col].length = 4 + (self.rng.next_u32() % 12) as usize;
        self.drops[col].active = true;
    }

    fn rain_color(&self, brightness: f32, is_head: bool) -> Rgb {
        let b = clamp01(brightness);
        if is_head {
            // White-ish head
            return Rgb::new(
                to_byte(180.0 + b * 75.0),
                to_byte(220.0 + b * 35.0),
                to_byte(180.0 + b * 75.0),
            );
        }

        let mode = self.values[2] as usize;
        if mode == 1 {
            // Blue
            Rgb::new(0, to_byte(b * 120.0), to_byte(b * 255.0))
        } else if mode == 2 {
            // Purple
            Rgb::new(to_byte(b * 200.0), 0, to_byte(b * 255.0))
        } else {
            // Green (classic)
            Rgb::new(to_byte(b * 40.0), to_byte(b * 255.0), to_byte(b * 30.0))
        }
    }
}

impl Effect for MatrixRainEffect {
    fn id(&self) -> &'static str {
        "matrix_rain"
    }

    fn name(&self) -> &'static str {
        "Matrix Rain"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "density" => Some(self.values[1]),
            "color_mode" => Some(self.values[2]),
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
            "density" => self.values[1] = value,
            "color_mode" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        for d in self.drops.iter_mut() {
            d.active = false;
        }
        self.trail.as_mut().fill([0.0; WIDTH]);
        self.flash = 0.0;
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let audio_on = self.values[3] != 0.0;
        let mut speed_mul = 1.0;
        if audio_on && context.audio.available {
            let level = clamp01(context.audio.level);
            speed_mul = 1.0 + level * 2.0;
            if context.audio.beat {
                self.flash = 0.6;
            }
        }

        let dt = context.delta_seconds.clamp(0.0, 1.0);
        self.flash *= (-dt * 6.0f32).exp();

        let spawn_chance = (self.values[1] / 100.0) * 0.08 * (1.0 + (speed_mul - 1.0) * 0.5);

        // Fade existing trail
        let decay = (-dt * 4.0f32).exp();
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                self.trail[y][x] *= decay;
            }
        }

        // Advance drops
        for col in 0..WIDTH {
            if !self.drops[col].active {
                if self.rng.next_f32() < spawn_chance * dt * 60.0 {
                    self.spawn_drop(col);
                }
            } else {
                let spd = self.drops[col].speed * speed_mul;
                self.drops[col].head_y += spd * dt;

                let hy = self.drops[col].head_y as i32;
                if hy >= 0 && hy < HEIGHT as i32 {
                    self.trail[hy as usize][col] = 1.0;
                }

                if self.drops[col].head_y - self.drops[col].length as f32 > HEIGHT as f32 {
                    self.drops[col].active = false;
                }
            }
        }

        // Render to frame
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let is_head = self.drops[x].active && (self.drops[x].head_y as i32 == y as i32);
                let mut b = self.trail[y][x];
                if self.flash > 0.01 {
                    b = (b + self.flash * 0.3).min(1.0);
                }
                if b > 0.01 || is_head {
                    frame.set(x, y, self.rain_color(b, is_head));
                } else {
                    frame.set(x, y, Rgb::BLACK);
                }
            }
        }
    }
}
