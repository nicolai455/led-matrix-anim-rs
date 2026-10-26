use crate::util::{alloc_boxed_2d, audio_bass, audio_mid, heat_to_rgb, XorShift32};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, HEIGHT, WIDTH,
};

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "intensity",
        label: "Intensity",
        kind: ParameterKind::Number,
        min: 0.0,
        max: 100.0,
        step: 1.0,
        default: 60.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "cooling",
        label: "Cooling",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 55.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "sparking",
        label: "Sparking",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 200.0,
        step: 1.0,
        default: 120.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "speed",
        label: "Speed",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 100.0,
        step: 1.0,
        default: 50.0,
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
        default: 0.0,
        options: &[],
        unit: "",
    },
];

pub struct FireEffect {
    values: [f32; 5],
    heat: Box<[[f32; WIDTH]; HEIGHT]>,
    rng: XorShift32,
    pulse: f32,
    accumulator: f32,
    started: bool,
}

impl Default for FireEffect {
    fn default() -> Self {
        Self {
            values: [60.0, 55.0, 120.0, 50.0, 0.0],
            heat: alloc_boxed_2d(0.0),
            rng: XorShift32::new(0x12345678),
            pulse: 0.0,
            accumulator: 0.0,
            started: false,
        }
    }
}

impl FireEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn tick(&mut self, bass_boost: f32, mid_boost: f32) {
        let dt = 0.016;
        self.pulse *= (-dt * 6.0f32).exp();

        let cooling = self.values[1] / 100.0;
        let intensity = (self.values[0] / 100.0) * (1.0 + mid_boost * 0.5);
        let spark_chance = (self.values[2] / 200.0) * (1.0 + bass_boost) * intensity;

        // Cool every cell
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let cool_amount = (0.01 + cooling * 0.04) * (1.0 + self.rng.next_f32() * 0.5);
                self.heat[y][x] = (self.heat[y][x] - cool_amount * dt * 60.0).max(0.0);
            }
        }

        // Heat rises: propagate upward with diffusion (from bottom to top)
        for y in 0..(HEIGHT - 1) {
            for x in 0..WIDTH {
                let below = self.heat[y + 1][x];
                let left = if x > 0 {
                    self.heat[y + 1][x - 1]
                } else {
                    below
                };
                let right = if x < WIDTH - 1 {
                    self.heat[y + 1][x + 1]
                } else {
                    below
                };
                let diffused =
                    (below * 0.5 + left * 0.2 + right * 0.2 + self.heat[y][x] * 0.1) * 0.98;
                self.heat[y][x] = self.heat[y][x].max(diffused);
            }
        }

        // Spark at bottom rows
        for x in 0..WIDTH {
            if self.rng.next_f32() < spark_chance * dt * 60.0 {
                self.heat[HEIGHT - 1][x] =
                    (self.heat[HEIGHT - 1][x] + 0.6 + self.rng.next_f32() * 0.4).min(1.0);
                if self.rng.next_f32() < 0.5 {
                    self.heat[HEIGHT - 2][x] =
                        (self.heat[HEIGHT - 2][x] + 0.4 + self.rng.next_f32() * 0.3).min(1.0);
                }
            }
        }
    }
}

impl Effect for FireEffect {
    fn id(&self) -> &'static str {
        "fire"
    }

    fn name(&self) -> &'static str {
        "Fire"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "intensity" => Some(self.values[0]),
            "cooling" => Some(self.values[1]),
            "sparking" => Some(self.values[2]),
            "speed" => Some(self.values[3]),
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
            "intensity" => self.values[0] = value,
            "cooling" => self.values[1] = value,
            "sparking" => self.values[2] = value,
            "speed" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.heat.as_mut().fill([0.0; WIDTH]);
        self.pulse = 0.0;
        self.accumulator = 0.0;
        self.started = false;
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let audio_on = self.values[4] != 0.0;
        let mut bass_boost = 0.0;
        let mut mid_boost = 0.0;
        if audio_on && context.audio.available {
            bass_boost = audio_bass(context.audio);
            mid_boost = audio_mid(context.audio);
            if context.audio.beat {
                self.pulse = 0.8;
            }
        }

        let dt = context.delta_seconds.clamp(0.0, 1.0);
        let speed = (self.values[3] / 50.0).max(0.1);
        self.accumulator += dt * speed;

        if !self.started {
            self.tick(bass_boost, mid_boost);
            self.started = true;
        }

        let fixed_step = 0.016;
        let mut steps = 0;
        while self.accumulator >= fixed_step && steps < 10 {
            self.tick(bass_boost, mid_boost);
            self.accumulator -= fixed_step;
            steps += 1;
        }
        if self.accumulator >= fixed_step {
            self.accumulator = 0.0;
        }

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let mut h = self.heat[y][x];
                if self.pulse > 0.01 && h > 0.05 {
                    h = (h + self.pulse * 0.25).min(1.0);
                }
                frame.set(x, y, heat_to_rgb(h));
            }
        }
    }
}
