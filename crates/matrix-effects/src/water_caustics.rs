use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Caribbean Pool", "Bioluminescent", "Sunlit Laguna", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Flow speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 35.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "depth",
        label: "Water clarity",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 90.0,
        step: 1.0,
        default: 50.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "turbulence",
        label: "Surface chop",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 10.0,
        step: 1.0,
        default: 5.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Water body",
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

pub struct WaterCausticsEffect {
    values: [f32; 5],
    time_phase: f32,
    color_cycle: f32,
    drop_ripple: f32,
}

impl Default for WaterCausticsEffect {
    fn default() -> Self {
        Self {
            values: [35.0, 50.0, 5.0, 3.0, 1.0],
            time_phase: 0.0,
            color_cycle: 0.0,
            drop_ripple: 0.0,
        }
    }
}

impl WaterCausticsEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn caustic_color(pal: usize, color_cycle: f32, caustic: f32, clarity: f32) -> Rgb {
        let c = clamp01(caustic);
        let base_ambient = 0.25 + 0.35 * clarity;

        match pal {
            0 => {
                // Caribbean Pool (Turquoise floor -> White light caustics)
                let r = to_byte((10.0 * base_ambient + 245.0 * c.powi(3)).min(255.0));
                let g = to_byte((140.0 * base_ambient + 115.0 * c).min(255.0));
                let blue = to_byte((220.0 * base_ambient + 35.0 * c).min(255.0));
                Rgb::new(r, g, blue)
            }
            1 => {
                // Bioluminescent Deep (Abyssal Navy -> Electric Cyan/White caustics)
                let r = to_byte((5.0 * base_ambient + 200.0 * c.powi(4)).min(255.0));
                let g = to_byte((40.0 * base_ambient + 215.0 * c.powi(2)).min(255.0));
                let blue = to_byte((180.0 * base_ambient + 75.0 * c).min(255.0));
                Rgb::new(r, g, blue)
            }
            2 => {
                // Sunlit Laguna (Deep Emerald -> Golden caustics)
                let r = to_byte((20.0 * base_ambient + 235.0 * c.powi(2)).min(255.0));
                let g = to_byte((180.0 * base_ambient + 75.0 * c).min(255.0));
                let blue = to_byte((80.0 * base_ambient + 80.0 * c.powi(4)).min(255.0));
                Rgb::new(r, g, blue)
            }
            _ => {
                // Rainbow Caustics
                let h = (color_cycle + c * 0.45) % 1.0;
                hsv_to_rgb(h, 0.9, (base_ambient * 0.5 + 0.5 * c).min(1.0))
            }
        }
    }
}

impl Effect for WaterCausticsEffect {
    fn id(&self) -> &'static str {
        "water_caustics"
    }

    fn name(&self) -> &'static str {
        "Liquid Water Caustics"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "depth" => Some(self.values[1]),
            "turbulence" => Some(self.values[2]),
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
            "depth" => self.values[1] = value,
            "turbulence" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.time_phase = 0.0;
        self.color_cycle = 0.0;
        self.drop_ripple = 0.0;
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
            self.drop_ripple = 1.0;
        } else {
            self.drop_ripple = (self.drop_ripple - dt * 2.2).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.7);
        self.time_phase = (self.time_phase + dt * speed * 1.8) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.14) % 1.0;

        let pal = self.values[3] as usize;
        let clarity = self.values[1] / 100.0;
        let turb = self.values[2] * 0.2;

        let t = self.time_phase;

        // Compute optical caustic light refraction field
        for y in 0..HEIGHT {
            let yf = y as f32 * 0.35 * turb;
            for x in 0..WIDTH {
                let xf = x as f32 * 0.35 * turb;

                // 3 wave vector layers intersecting
                let w1 = (xf * 1.1 + yf * 0.4 + t * 1.2).sin();
                let w2 = (xf * 0.3 - yf * 1.2 + t * 1.5).cos();
                let w3 = ((xf + yf) * 0.7 - t * 0.9).sin();

                // Audio drop ripple from center (15.5, 15.5)
                let drop = if self.drop_ripple > 0.05 {
                    let dx = x as f32 - 15.5;
                    let dy = y as f32 - 15.5;
                    let r = (dx * dx + dy * dy).sqrt();
                    ((r * 2.0 - (1.0 - self.drop_ripple) * 16.0).sin() * 0.5).max(0.0) * self.drop_ripple
                } else {
                    0.0
                };

                let surface_chop = if audio_active {
                    (xf * 5.0).sin() * (yf * 5.0).cos() * treble * 0.25
                } else {
                    0.0
                };

                // Approximate surface curvature / light focal focus (2nd derivative / product)
                // Fast x^2.5 computation via x * x * sqrt(x) instead of slow transcendental powf(2.5)
                let c_base = (((w1 * w2 + w3) + 1.2) * 0.45 + drop + surface_chop + mid * 0.15).max(0.0);
                let caustic_val = c_base * c_base * c_base.sqrt();

                let col = Self::caustic_color(pal, self.color_cycle, caustic_val, clarity);
                frame.set(x, y, col);
            }
        }
    }
}
