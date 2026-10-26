use crate::util::{alloc_boxed_2d, clamp01, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const PALETTES: &[&str] = &["Heat", "Ocean", "Cyberpunk"];

const PARAMETERS: [ParameterDescriptor; 3] = [
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
        id: "palette",
        label: "Palette",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 2.0,
        step: 1.0,
        default: 0.0,
        options: PALETTES,
        unit: "",
    },
    ParameterDescriptor {
        id: "gain",
        label: "Gain",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 200.0,
        step: 1.0,
        default: 100.0,
        options: &[],
        unit: "%",
    },
];

pub struct SpectrogramEffect {
    values: [f32; 3],
    history: Box<[[f32; WIDTH]; HEIGHT]>,
    accumulator: f32,
    idle_phase: f32,
    beat_boost: f32,
}

impl Default for SpectrogramEffect {
    fn default() -> Self {
        Self {
            values: [50.0, 0.0, 100.0],
            history: alloc_boxed_2d(0.0),
            accumulator: 0.0,
            idle_phase: 0.0,
            beat_boost: 0.0,
        }
    }
}

impl SpectrogramEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn color_pixel(&self, val: f32) -> Rgb {
        let t = clamp01(val);
        let pal = self.values[1] as usize;
        if pal == 1 {
            // Ocean: black -> dark blue -> cyan -> white
            if t <= 0.0 {
                Rgb::BLACK
            } else if t < 0.33 {
                Rgb::new(0, 0, to_byte((t / 0.33) * 180.0))
            } else if t < 0.66 {
                let frac = (t - 0.33) / 0.33;
                Rgb::new(0, to_byte(frac * 220.0), to_byte(180.0 + frac * 75.0))
            } else {
                let frac = (t - 0.66) / 0.34;
                Rgb::new(to_byte(frac * 255.0), 220 + to_byte(frac * 35.0), 255)
            }
        } else if pal == 2 {
            // Cyberpunk: black -> purple -> magenta -> cyan
            if t <= 0.0 {
                Rgb::BLACK
            } else if t < 0.33 {
                let frac = t / 0.33;
                Rgb::new(to_byte(frac * 120.0), 0, to_byte(frac * 180.0))
            } else if t < 0.66 {
                let frac = (t - 0.33) / 0.33;
                Rgb::new(
                    to_byte(120.0 + frac * 135.0),
                    0,
                    to_byte(180.0 + frac * 40.0),
                )
            } else {
                let frac = (t - 0.66) / 0.34;
                Rgb::new(
                    to_byte(255.0 * (1.0 - frac * 0.8)),
                    to_byte(frac * 240.0),
                    220,
                )
            }
        } else {
            // Heat: black -> red -> yellow -> white
            if t <= 0.0 {
                Rgb::BLACK
            } else if t < 0.25 {
                Rgb::new(to_byte(t * 4.0 * 255.0), 0, 0)
            } else if t < 0.50 {
                Rgb::new(255, to_byte((t - 0.25) * 4.0 * 200.0), 0)
            } else if t < 0.75 {
                Rgb::new(
                    255,
                    200 + to_byte((t - 0.50) * 4.0 * 55.0),
                    to_byte((t - 0.50) * 4.0 * 80.0),
                )
            } else {
                Rgb::new(255, 255, 80 + to_byte((t - 0.75) * 4.0 * 175.0))
            }
        }
    }

    fn scroll_up(&mut self) {
        for y in 0..(HEIGHT - 1) {
            self.history[y] = self.history[y + 1];
        }
    }
}

impl Effect for SpectrogramEffect {
    fn id(&self) -> &'static str {
        "spectrogram"
    }

    fn name(&self) -> &'static str {
        "Spectrogram"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "palette" => Some(self.values[1]),
            "gain" => Some(self.values[2]),
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
            "palette" => self.values[1] = value,
            "gain" => self.values[2] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.history.as_mut().fill([0.0; WIDTH]);
        self.accumulator = 0.0;
        self.idle_phase = 0.0;
        self.beat_boost = 0.0;
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let dt = context.delta_seconds.clamp(0.0, 1.0);
        self.beat_boost *= (-dt * 5.0f32).exp();

        if context.audio.available && context.audio.beat {
            self.beat_boost = 0.4;
        }

        let gain = self.values[2] / 100.0;
        let speed_rows_per_sec = 5.0 + self.values[0] * 0.4;
        let row_interval = 1.0 / speed_rows_per_sec;

        self.accumulator += dt;
        self.idle_phase += dt * 2.0;

        let mut scrolled = false;
        if self.accumulator >= row_interval * HEIGHT as f32 {
            self.accumulator %= row_interval;
            self.history.as_mut().fill([0.0; WIDTH]);
            scrolled = true;
        } else {
            let mut steps = 0;
            while self.accumulator >= row_interval && steps < HEIGHT {
                self.accumulator -= row_interval;
                self.scroll_up();
                scrolled = true;
                steps += 1;
            }
            if self.accumulator >= row_interval {
                self.accumulator = 0.0;
            }
        }

        if scrolled {
            let mut new_row = [0.0f32; WIDTH];
            let max_band = context.audio.bands.iter().copied().fold(0.0f32, f32::max);
            let active_audio =
                context.audio.available && (context.audio.level.max(max_band) > 0.01);

            if active_audio {
                for (col, val) in new_row.iter_mut().enumerate() {
                    let band_f = col as f32 * 15.0 / 31.0;
                    let b0 = band_f.floor() as usize;
                    let b1 = (b0 + 1).min(15);
                    let frac = band_f - b0 as f32;
                    let raw =
                        context.audio.bands[b0] * (1.0 - frac) + context.audio.bands[b1] * frac;
                    *val = clamp01(raw * gain + self.beat_boost * 0.3);
                }
            } else {
                for (col, val) in new_row.iter_mut().enumerate() {
                    let s1 = (self.idle_phase + col as f32 * 0.3).sin();
                    let s2 = (self.idle_phase * 0.5 + col as f32 * 0.6).cos();
                    let v = ((s1 * 0.5 + 0.5) * 0.6 + (s2 * 0.5 + 0.5) * 0.4) * gain * 0.7;
                    *val = clamp01(v);
                }
            }
            self.history[HEIGHT - 1] = new_row;
        }

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                frame.set(x, y, self.color_pixel(self.history[y][x]));
            }
        }
    }
}
