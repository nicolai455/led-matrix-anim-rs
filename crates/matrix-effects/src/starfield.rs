use crate::util::{clamp01, hsv_to_rgb, to_byte, XorShift32};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const COLOR_MODES: &[&str] = &["White", "Rainbow", "Blue"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "star_count",
        label: "Stars",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 128.0,
        step: 1.0,
        default: 64.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "warp_speed",
        label: "Warp speed",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 100.0,
        step: 1.0,
        default: 30.0,
        options: &[],
        unit: "",
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
struct Star {
    x: f32,
    y: f32,
    z: f32,
}

pub struct StarfieldEffect {
    values: [f32; 4],
    stars: Box<[Star; 128]>,
    rng: XorShift32,
    flash: f32,
    initialized: bool,
}

impl Default for StarfieldEffect {
    fn default() -> Self {
        let stars = vec![
            Star {
                x: 0.0,
                y: 0.0,
                z: 1.0
            };
            128
        ]
        .into_boxed_slice()
        .try_into()
        .unwrap();
        Self {
            values: [64.0, 30.0, 0.0, 0.0],
            stars,
            rng: XorShift32::new(0xDEADBEEF),
            flash: 0.0,
            initialized: false,
        }
    }
}

impl StarfieldEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn spawn_star(&mut self, i: usize) {
        self.stars[i].x = self.rng.next_signed_f32();
        self.stars[i].y = self.rng.next_signed_f32();
        self.stars[i].z = 1.0;
    }

    fn init_stars(&mut self) {
        for i in 0..128 {
            self.stars[i].x = self.rng.next_signed_f32();
            self.stars[i].y = self.rng.next_signed_f32();
            self.stars[i].z = self.rng.next_f32() * 0.95 + 0.05;
        }
        self.initialized = true;
    }
}

impl Effect for StarfieldEffect {
    fn id(&self) -> &'static str {
        "starfield"
    }

    fn name(&self) -> &'static str {
        "Starfield"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "star_count" => Some(self.values[0]),
            "warp_speed" => Some(self.values[1]),
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
            "star_count" => self.values[0] = value,
            "warp_speed" => self.values[1] = value,
            "color_mode" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.flash = 0.0;
        self.initialized = false;
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        if !self.initialized {
            self.init_stars();
        }

        let audio_on = self.values[3] != 0.0;
        let mut speed_mul = 1.0;
        if audio_on && context.audio.available {
            let level = clamp01(context.audio.level);
            speed_mul = 1.0 + level * 3.0;
            if context.audio.beat {
                self.flash = 1.0;
            }
        }

        let dt = context.delta_seconds.clamp(0.0, 1.0);
        self.flash *= (-dt * 8.0f32).exp();

        let speed = self.values[1] * 0.01 * speed_mul;
        let count = (self.values[0] as usize).clamp(10, 128);

        for i in 0..count {
            self.stars[i].z -= speed * dt * 4.0;
            if self.stars[i].z <= 0.05 {
                self.spawn_star(i);
            }
        }

        frame.clear(Rgb::BLACK);

        let color_mode = self.values[2] as usize;

        for i in 0..count {
            let z = self.stars[i].z;
            if z <= 0.05 {
                continue;
            }

            let sx = (15.5 + (self.stars[i].x / z) * 16.0).round() as i32;
            let sy = (15.5 + (self.stars[i].y / z) * 16.0).round() as i32;

            if sx >= 0 && sx < WIDTH as i32 && sy >= 0 && sy < HEIGHT as i32 {
                let brightness = ((1.0 - z) * 255.0 + self.flash * 80.0).clamp(0.0, 255.0);
                let col = if color_mode == 1 {
                    // Rainbow
                    hsv_to_rgb(z, 0.8, brightness / 255.0)
                } else if color_mode == 2 {
                    // Blue
                    Rgb::new(
                        to_byte(brightness * 0.3),
                        to_byte(brightness * 0.7),
                        to_byte(brightness),
                    )
                } else {
                    // White
                    let b = to_byte(brightness);
                    Rgb::new(b, b, b)
                };
                frame.set(sx as usize, sy as usize, col);
            }
        }
    }
}
