use crate::util::{clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    AudioSnapshot, Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT,
    WIDTH,
};

const PALETTES: &[&str] = &["Rainbow", "Fire", "Ocean"];
const INPUTS: &[&str] = &["Mic only", "Auto fallback", "Demo wave"];

const PARAMETERS: [ParameterDescriptor; 7] = [
    ParameterDescriptor {
        id: "scale",
        label: "Scale",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 200.0,
        step: 1.0,
        default: 100.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "peak_hold",
        label: "Peak hold",
        kind: ParameterKind::Boolean,
        min: 0.0,
        max: 1.0,
        step: 1.0,
        default: 1.0,
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
        id: "mirror",
        label: "Mirror",
        kind: ParameterKind::Boolean,
        min: 0.0,
        max: 1.0,
        step: 1.0,
        default: 0.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "input_mode",
        label: "Input mode",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 2.0,
        step: 1.0,
        default: 0.0,
        options: INPUTS,
        unit: "",
    },
    ParameterDescriptor {
        id: "wave_height",
        label: "Wave height",
        kind: ParameterKind::Number,
        min: 0.0,
        max: 200.0,
        step: 1.0,
        default: 100.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "wave_speed",
        label: "Wave speed",
        kind: ParameterKind::Number,
        min: 0.0,
        max: 200.0,
        step: 1.0,
        default: 100.0,
        options: &[],
        unit: "%",
    },
];

pub struct FreqBarsEffect {
    values: [f32; 7],
    peaks: [f32; WIDTH],
    bars: [f32; WIDTH],
    phase: f32,
}

impl Default for FreqBarsEffect {
    fn default() -> Self {
        Self {
            values: [100.0, 1.0, 0.0, 0.0, 0.0, 100.0, 100.0],
            peaks: [0.0; WIDTH],
            bars: [0.0; WIDTH],
            phase: 0.0,
        }
    }
}

impl FreqBarsEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_color(&self, x: usize, y: usize, height: usize) -> Rgb {
        let pal = self.values[2] as usize;
        let t = if height > 0 {
            y as f32 / height as f32
        } else {
            0.0
        };

        if pal == 0 {
            // Rainbow across frequency axis X
            let hue = x as f32 / 36.0;
            let bright = 0.6 + 0.4 * t;
            hsv_to_rgb(hue, 1.0, bright)
        } else if pal == 1 {
            // Fire: black -> red -> yellow -> white
            if t < 0.33 {
                Rgb::new(to_byte((t / 0.33) * 255.0), 0, 0)
            } else if t < 0.75 {
                Rgb::new(255, to_byte(((t - 0.33) / 0.42) * 255.0), 0)
            } else {
                Rgb::new(255, 255, to_byte(((t - 0.75) / 0.25) * 255.0))
            }
        } else {
            // Ocean: deep blue -> cyan -> azure -> white
            if t < 0.5 {
                Rgb::new(
                    0,
                    to_byte((t / 0.5) * 180.0),
                    to_byte(100.0 + (t / 0.5) * 155.0),
                )
            } else {
                Rgb::new(
                    to_byte(((t - 0.5) / 0.5) * 220.0),
                    to_byte(180.0 + ((t - 0.5) / 0.5) * 75.0),
                    255,
                )
            }
        }
    }

    fn compute_bars(&mut self, audio: &AudioSnapshot, dt: f32) {
        let scale = self.values[0] / 100.0;
        let input_mode = self.values[4] as usize;
        let wave_h = self.values[5] / 100.0;
        let wave_s = self.values[6] / 100.0;

        let use_demo = input_mode == 2 || (input_mode == 1 && !audio.available);

        let mut raw_bands = [0.0f32; 16];
        if use_demo {
            self.phase += dt * wave_s * 4.0;
            for (i, band) in raw_bands.iter_mut().enumerate() {
                let s1 = (self.phase + i as f32 * 0.4).sin();
                let s2 = (self.phase * 0.7 + i as f32 * 0.8).cos();
                *band = ((s1 * 0.5 + 0.5) * 0.7 + (s2 * 0.5 + 0.5) * 0.3) * wave_h;
            }
        } else if audio.available {
            for (band, audio_band) in raw_bands.iter_mut().zip(audio.bands.iter()) {
                *band = clamp01(*audio_band) * scale;
            }
        }

        let mirror = self.values[3] != 0.0;

        for x in 0..WIDTH {
            let val = if mirror {
                let dist_from_center = if x < 16 { 15 - x } else { x - 16 };
                raw_bands[dist_from_center]
            } else {
                raw_bands[x / 2]
            };

            let target_h = (val * HEIGHT as f32).clamp(0.0, HEIGHT as f32);
            // Smooth attack and decay
            if target_h > self.bars[x] {
                self.bars[x] = target_h;
            } else {
                self.bars[x] = (self.bars[x] - dt * 25.0).max(target_h);
            }

            // Peak hold
            if self.bars[x] >= self.peaks[x] {
                self.peaks[x] = self.bars[x];
            } else {
                self.peaks[x] = (self.peaks[x] - dt * 10.0).max(0.0);
            }
        }
    }
}

impl Effect for FreqBarsEffect {
    fn id(&self) -> &'static str {
        "freq_bars"
    }

    fn name(&self) -> &'static str {
        "Frequency Bars"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "scale" => Some(self.values[0]),
            "peak_hold" => Some(self.values[1]),
            "palette" => Some(self.values[2]),
            "mirror" => Some(self.values[3]),
            "input_mode" => Some(self.values[4]),
            "wave_height" => Some(self.values[5]),
            "wave_speed" => Some(self.values[6]),
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
            "scale" => self.values[0] = value,
            "peak_hold" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "mirror" => self.values[3] = value,
            "input_mode" => self.values[4] = value,
            "wave_height" => self.values[5] = value,
            "wave_speed" => self.values[6] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.peaks = [0.0; WIDTH];
        self.bars = [0.0; WIDTH];
        self.phase = 0.0;
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let dt = context.delta_seconds.clamp(0.0, 1.0);
        self.compute_bars(context.audio, dt);

        frame.clear(Rgb::BLACK);

        let peak_enabled = self.values[1] != 0.0;

        for x in 0..WIDTH {
            let bar_h = self.bars[x].round() as usize;
            for y_from_bottom in 0..bar_h.min(HEIGHT) {
                let y = HEIGHT - 1 - y_from_bottom;
                let color = self.get_color(x, y_from_bottom, bar_h);
                frame.set(x, y, color);
            }

            if peak_enabled {
                let peak_y_from_bottom = self.peaks[x].round() as usize;
                if peak_y_from_bottom > 0 && peak_y_from_bottom <= HEIGHT {
                    let y = HEIGHT - peak_y_from_bottom;
                    frame.set(x, y, Rgb::WHITE);
                }
            }
        }
    }
}
