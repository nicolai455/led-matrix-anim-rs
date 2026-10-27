use crate::util::{audio_bass, audio_mid, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Borealis Green", "Arctic Ice", "Solar Storm", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Speed",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 100.0,
        step: 1.0,
        default: 30.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "waves",
        label: "Waves",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 6.0,
        step: 1.0,
        default: 3.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "intensity",
        label: "Intensity",
        kind: ParameterKind::Number,
        min: 20.0,
        max: 100.0,
        step: 1.0,
        default: 70.0,
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

pub struct PolarLightsEffect {
    values: [f32; 5],
    phase: f32,
    color_cycle: f32,
    shimmer: f32,
}

impl Default for PolarLightsEffect {
    fn default() -> Self {
        Self {
            values: [30.0, 3.0, 70.0, 3.0, 1.0],
            phase: 0.0,
            color_cycle: 0.0,
            shimmer: 0.0,
        }
    }
}

impl PolarLightsEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn curtain_color(pal: usize, color_cycle: f32, brightness: f32, y_norm: f32, curtain_idx: usize) -> Rgb {
        let b = clamp01(brightness);
        match pal {
            0 => {
                // Borealis Green (Emerald green bottom transitioning to violet top)
                let r = to_byte(255.0 * (0.15 + 0.75 * y_norm) * b);
                let g = to_byte(255.0 * (0.95 - 0.45 * y_norm) * b);
                let blue = to_byte(255.0 * (0.35 + 0.65 * y_norm) * b);
                Rgb::new(r, g, blue)
            }
            1 => {
                // Arctic Ice (Deep cyan to pale crystalline blue / white)
                let r = to_byte(255.0 * (0.2 + 0.6 * y_norm) * b);
                let g = to_byte(255.0 * (0.7 + 0.3 * y_norm) * b);
                let blue = to_byte(255.0 * 0.95 * b);
                Rgb::new(r, g, blue)
            }
            2 => {
                // Solar Storm (Crimson to fiery gold)
                let r = to_byte(255.0 * 0.95 * b);
                let g = to_byte(255.0 * (0.25 + 0.65 * (1.0 - y_norm)) * b);
                let blue = to_byte(255.0 * (0.1 + 0.2 * y_norm) * b);
                Rgb::new(r, g, blue)
            }
            _ => {
                // Rainbow Fade
                let h = (color_cycle + y_norm * 0.4 + curtain_idx as f32 * 0.2) % 1.0;
                hsv_to_rgb(h, 0.95, b)
            }
        }
    }
}

impl Effect for PolarLightsEffect {
    fn id(&self) -> &'static str {
        "polar_lights"
    }

    fn name(&self) -> &'static str {
        "Polar Lights"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "waves" => Some(self.values[1]),
            "intensity" => Some(self.values[2]),
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
            "waves" => self.values[1] = value,
            "intensity" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.color_cycle = 0.0;
        self.shimmer = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::BLACK);

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[4] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.shimmer = 1.0;
        } else {
            self.shimmer = (self.shimmer - dt * 2.0).max(0.0);
        }

        let speed = self.values[0] / 30.0;
        self.phase = (self.phase + dt * speed * 1.2) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.12) % 1.0;

        let pal = self.values[3] as usize;
        let wave_freq = self.values[1];
        let global_intensity = (self.values[2] / 100.0) * (1.0 + self.shimmer * 0.4);

        // 3 flowing light curtains at different depths
        for curtain in 0..3 {
            let c_phase = self.phase + curtain as f32 * 1.8;
            let c_base_y = 12.0 + curtain as f32 * 5.0;
            let c_amp = (5.0 + curtain as f32 * 2.0) * (1.0 + bass * 0.5);

            for x in 0..WIDTH {
                let x_norm = x as f32 / WIDTH as f32;
                let ripple = if audio_active { (x_norm * 16.0 * PI + self.phase * 3.0).sin() * mid * 1.5 } else { 0.0 };

                let cy = c_base_y
                    + (x_norm * wave_freq * 2.0 * PI + c_phase).sin() * c_amp
                    + (x_norm * wave_freq * 4.0 * PI - c_phase * 1.4).cos() * (c_amp * 0.35)
                    + ripple;

                for y in 0..HEIGHT {
                    let yf = y as f32;
                    let dist = (yf - cy).abs();
                    // Asymmetric curtain: sharp bottom fold, long upward billowing tail
                    let vert_falloff = if yf < cy {
                        // Upward tail
                        (-dist * 0.22).exp()
                    } else {
                        // Downward sharp base
                        (-dist * 0.65).exp()
                    };

                    let brightness = vert_falloff * global_intensity * (0.8 + 0.2 * (x_norm * 8.0 * PI + self.phase).sin());

                    if brightness > 0.03 {
                        let y_norm = yf / HEIGHT as f32;
                        let col = Self::curtain_color(pal, self.color_cycle, brightness, y_norm, curtain);
                        let cur = frame.get(x, y);
                        frame.set(
                            x,
                            y,
                            Rgb::new(
                                cur.r.saturating_add(col.r),
                                cur.g.saturating_add(col.g),
                                cur.b.saturating_add(col.b),
                            ),
                        );
                    }
                }
            }
        }
    }
}
