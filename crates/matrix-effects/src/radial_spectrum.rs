use crate::util::{clamp01, hsv_to_rgb};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Cosmos", "Rainbow", "Electric"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "rotation_speed",
        label: "Rotation speed",
        kind: ParameterKind::Number,
        min: 0.0,
        max: 100.0,
        step: 1.0,
        default: 25.0,
        options: &[],
        unit: "deg/s",
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
        id: "mirror",
        label: "Mirror",
        kind: ParameterKind::Boolean,
        min: 0.0,
        max: 1.0,
        step: 1.0,
        default: 1.0,
        options: &[],
        unit: "",
    },
];

pub struct RadialSpectrumEffect {
    values: [f32; 4],
    angle_phase: f32,
    smooth_bands: [f32; 16],
    core_pulse: f32,
}

impl Default for RadialSpectrumEffect {
    fn default() -> Self {
        Self {
            values: [25.0, 0.0, 100.0, 1.0],
            angle_phase: 0.0,
            smooth_bands: [0.0; 16],
            core_pulse: 0.0,
        }
    }
}

impl RadialSpectrumEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_color(&self, angle_norm: f32, radius_norm: f32, intensity: f32) -> Rgb {
        let pal = self.values[1] as usize;
        let v = clamp01(intensity);
        if pal == 0 {
            // Cosmos: deep purple/blue at center -> pink/magenta -> white at peaks
            let hue = 0.7 + angle_norm * 0.2 + radius_norm * 0.1;
            let sat = (1.0 - radius_norm * 0.5).clamp(0.2, 1.0);
            hsv_to_rgb(hue, sat, v)
        } else if pal == 1 {
            // Rainbow: hue mapped around the circle
            let hue = angle_norm;
            hsv_to_rgb(hue, 1.0, v)
        } else {
            // Electric: cyan -> blue -> violet -> white
            let hue = 0.5 + angle_norm * 0.35;
            let sat = (0.7 + 0.3 * (angle_norm * std::f32::consts::TAU).sin()).clamp(0.0, 1.0);
            hsv_to_rgb(hue, sat, v)
        }
    }
}

impl Effect for RadialSpectrumEffect {
    fn id(&self) -> &'static str {
        "radial_spectrum"
    }

    fn name(&self) -> &'static str {
        "Radial Spectrum"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "rotation_speed" => Some(self.values[0]),
            "palette" => Some(self.values[1]),
            "scale" => Some(self.values[2]),
            "mirror" => Some(self.values[3]),
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
            "rotation_speed" => self.values[0] = value,
            "palette" => self.values[1] = value,
            "scale" => self.values[2] = value,
            "mirror" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.angle_phase = 0.0;
        self.smooth_bands = [0.0; 16];
        self.core_pulse = 0.0;
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let dt = context.delta_seconds.clamp(0.0, 1.0);
        let rot_speed = self.values[0] * (PI / 180.0);
        self.angle_phase = (self.angle_phase + rot_speed * dt) % (2.0 * PI);

        self.core_pulse *= (-dt * 5.0f32).exp();
        if context.audio.available && context.audio.beat {
            self.core_pulse = 0.9;
        }

        let has_audio = context.audio.available && context.audio.level > 0.0001;
        for i in 0..16 {
            let target = if has_audio {
                context.audio.bands[i]
            } else {
                0.35 + 0.3 * (i as f32 * 0.5 + self.angle_phase * 2.0).sin()
            };
            if target >= self.smooth_bands[i] {
                self.smooth_bands[i] = target;
            } else {
                self.smooth_bands[i] = (self.smooth_bands[i] - 3.5 * dt).max(0.0);
            }
        }

        let scale = self.values[2] / 100.0;
        let mirror = self.values[3] != 0.0;
        let two_pi = 2.0 * PI;

        let cx = 15.5;
        let cy = 15.5;

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt();

                let mut angle = dy.atan2(dx) + self.angle_phase;
                angle = (angle % two_pi + two_pi) % two_pi;

                let mut norm_angle = angle / two_pi;
                if mirror {
                    norm_angle *= 2.0;
                    if norm_angle >= 1.0 {
                        norm_angle = 2.0 - norm_angle;
                    }
                }

                let band_idx = ((norm_angle * 15.999) as usize).clamp(0, 15);
                let band_level = self.smooth_bands[band_idx];
                let max_radius = 3.0 + band_level * scale * 17.0;

                let mut intensity = 0.0;
                if dist <= max_radius {
                    let edge_glow = (max_radius - dist) / 1.5;
                    intensity = edge_glow.min(1.0) * (0.5 + 0.5 * band_level);
                }

                let core = (-dist * 0.6).exp() * (0.3 + self.core_pulse);
                intensity = (intensity + core).min(1.0);

                frame.set(x, y, self.get_color(norm_angle, dist / 22.0, intensity));
            }
        }
    }
}
