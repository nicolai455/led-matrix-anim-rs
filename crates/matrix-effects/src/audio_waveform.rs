use crate::util::{alloc_boxed_2d, audio_bass, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const PALETTES: &[&str] = &["Neon", "Spectrum", "Fire"];

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
        id: "amplitude",
        label: "Amplitude",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 200.0,
        step: 1.0,
        default: 100.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "thickness",
        label: "Thickness",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 5.0,
        step: 1.0,
        default: 2.0,
        options: &[],
        unit: "px",
    },
];

pub struct AudioWaveformEffect {
    values: [f32; 4],
    trail: Box<[[f32; WIDTH]; HEIGHT]>,
    phase: f32,
    beat_flash: f32,
    accumulator: f32,
}

impl Default for AudioWaveformEffect {
    fn default() -> Self {
        Self {
            values: [40.0, 0.0, 100.0, 2.0],
            trail: alloc_boxed_2d(0.0),
            phase: 0.0,
            beat_flash: 0.0,
            accumulator: 0.0,
        }
    }
}

impl AudioWaveformEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_color(&self, x: usize, intensity: f32) -> Rgb {
        let pal = self.values[1] as usize;
        let val = clamp01(intensity);
        if pal == 0 {
            // Neon: cyan and electric magenta
            let t = x as f32 / 31.0;
            let r = to_byte(val * (180.0 * t + 50.0));
            let g = to_byte(val * (255.0 * (1.0 - t * 0.7)));
            let b = to_byte(val * 255.0);
            Rgb::new(r, g, b)
        } else if pal == 1 {
            // Spectrum: rainbow progression across columns
            let hue = x as f32 / 32.0;
            hsv_to_rgb(hue, 1.0, val)
        } else {
            // Fire: yellow core to red edges
            if val < 0.4 {
                Rgb::new(to_byte((val / 0.4) * 220.0), 0, 0)
            } else if val < 0.8 {
                Rgb::new(255, to_byte(((val - 0.4) / 0.4) * 220.0), 0)
            } else {
                Rgb::new(255, 255, to_byte(((val - 0.8) / 0.2) * 255.0))
            }
        }
    }

    fn tick(
        &mut self,
        audio: &matrix_core::AudioSnapshot,
        amp_mul: f32,
        speed_mul: f32,
        beat: bool,
    ) {
        let dt = 0.016;
        self.phase += speed_mul * dt * 4.0;
        if self.phase > 62.831853 {
            self.phase -= 62.831853;
        }

        self.beat_flash *= (-dt * 6.0f32).exp();
        if beat {
            self.beat_flash = 0.8;
        }

        let decay = (-dt * 5.0f32).exp();
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                self.trail[y][x] *= decay;
            }
        }

        let has_audio = audio.available && (audio.level > 0.0001);
        let thick = self.values[3];
        let bass = audio_bass(audio);

        for x in 0..WIDTH {
            let disp = if has_audio {
                let b = x / 2;
                let band_e = audio.bands[b];
                (x as f32 * 0.35 + self.phase).sin() * (3.0 + 11.0 * band_e * amp_mul)
                    + (x as f32 * 0.15 - self.phase * 0.5).cos() * (bass * 4.0 * amp_mul)
            } else {
                (x as f32 * 0.25 + self.phase).sin() * (6.0 * amp_mul)
                    + (x as f32 * 0.55 - self.phase * 0.8).cos() * (2.5 * amp_mul)
            };

            let center_y = 15.5 + disp;
            let sigma = 0.5 + thick * 0.45;
            for y in 0..HEIGHT {
                let dy = y as f32 - center_y;
                let mut glow = (-dy * dy / (2.0 * sigma * sigma)).exp();
                glow *= 1.0 + self.beat_flash;
                self.trail[y][x] = self.trail[y][x].max(glow);
            }
        }
    }
}

impl Effect for AudioWaveformEffect {
    fn id(&self) -> &'static str {
        "audio_waveform"
    }

    fn name(&self) -> &'static str {
        "Waveform"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "palette" => Some(self.values[1]),
            "amplitude" => Some(self.values[2]),
            "thickness" => Some(self.values[3]),
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
            "amplitude" => self.values[2] = value,
            "thickness" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.trail.as_mut().fill([0.0; WIDTH]);
        self.phase = 0.0;
        self.beat_flash = 0.0;
        self.accumulator = 0.0;
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let speed_mul = self.values[0] / 40.0;
        let amp_mul = self.values[2] / 100.0;
        let beat = context.audio.available && context.audio.beat;

        let dt = context.delta_seconds.clamp(0.0, 1.0);
        self.accumulator += dt;

        let fixed_step = 0.016;
        let mut steps = 0;
        while self.accumulator >= fixed_step && steps < 10 {
            self.tick(context.audio, amp_mul, speed_mul, beat);
            self.accumulator -= fixed_step;
            steps += 1;
        }
        if self.accumulator >= fixed_step {
            self.accumulator = 0.0;
        }

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let intensity = self.trail[y][x];
                frame.set(x, y, self.get_color(x, intensity));
            }
        }
    }
}
