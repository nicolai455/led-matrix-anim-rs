use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Chrome & Neon", "Red & Blue Police", "Gold Palace", "Rainbow Rave"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "Flight speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 40.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "facets",
        label: "Facet density",
        kind: ParameterKind::Number,
        min: 4.0,
        max: 16.0,
        step: 2.0,
        default: 8.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Mirror tint",
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

pub struct MirrorCavernEffect {
    values: [f32; 4],
    cam_z: f32,
    cam_rot: f32,
    strobe_pulse: f32,
    color_cycle: f32,
}

impl Default for MirrorCavernEffect {
    fn default() -> Self {
        Self {
            values: [40.0, 8.0, 0.0, 1.0],
            cam_z: 0.0,
            cam_rot: 0.0,
            strobe_pulse: 0.0,
            color_cycle: 0.0,
        }
    }
}

impl MirrorCavernEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn facet_color(pal: usize, facet_hash: f32, color_cycle: f32, specular: f32) -> Rgb {
        let spec = clamp01(specular);
        match pal {
            0 => {
                // Chrome & Neon: Mirrored steel with electric cyan/magenta highlights
                let is_pink = facet_hash > 0.5;
                if spec > 0.75 {
                    Rgb::WHITE
                } else if is_pink {
                    Rgb::new(to_byte(255.0 * spec), to_byte(20.0 * spec), to_byte(180.0 * spec))
                } else {
                    Rgb::new(to_byte(10.0 * spec), to_byte(220.0 * spec), to_byte(255.0 * spec))
                }
            }
            1 => {
                // Red & Blue Police
                let is_red = facet_hash > 0.5;
                if spec > 0.8 {
                    Rgb::WHITE
                } else if is_red {
                    Rgb::new(to_byte(255.0 * spec), to_byte(20.0 * spec), to_byte(30.0 * spec))
                } else {
                    Rgb::new(to_byte(15.0 * spec), to_byte(90.0 * spec), to_byte(255.0 * spec))
                }
            }
            2 => {
                // Gold Palace: Polished brass and warm amber
                if spec > 0.8 {
                    Rgb::WHITE
                } else {
                    Rgb::new(to_byte(255.0 * spec), to_byte(200.0 * spec), to_byte(30.0 * spec))
                }
            }
            _ => {
                // Rainbow Rave
                let h = (color_cycle + facet_hash) % 1.0;
                let c = hsv_to_rgb(h, 0.9, spec);
                if spec > 0.85 { Rgb::WHITE } else { c }
            }
        }
    }
}

impl Effect for MirrorCavernEffect {
    fn id(&self) -> &'static str {
        "mirror_cavern"
    }

    fn name(&self) -> &'static str {
        "Mirror Cavern"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "facets" => Some(self.values[1]),
            "palette" => Some(self.values[2]),
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
            "facets" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.cam_z = 0.0;
        self.cam_rot = 0.0;
        self.strobe_pulse = 0.0;
        self.color_cycle = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::BLACK);

        // Integrate the bounded interval once, then render the final state.
        let dt = if context.delta_seconds.is_finite() {
            context.delta_seconds.clamp(0.0, 0.5)
        } else {
            0.0
        };
        let audio_active = self.values[3] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.strobe_pulse = 1.0;
        } else {
            self.strobe_pulse = (self.strobe_pulse - dt * 3.5).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.9 + self.strobe_pulse * 0.5);
        self.cam_z = (self.cam_z + dt * speed * 4.5) % 1000.0;
        self.cam_rot = (self.cam_rot + dt * speed * 0.6) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let pal = self.values[2] as usize;
        let facet_density = self.values[1];

        let cx = 15.5f32;
        let cy = 15.5f32;

        let cos_r = self.cam_rot.cos();
        let sin_r = self.cam_rot.sin();

        // Moving spotlight in the cavern chamber
        let light_x = (self.cam_rot * 2.0).sin() * 4.0;
        let light_y = (self.cam_rot * 2.0).cos() * 4.0;

        for y in 0..HEIGHT {
            let dy = y as f32 - cy;
            for x in 0..WIDTH {
                let dx = x as f32 - cx;

                // Rotate screen ray
                let rx = dx * cos_r - dy * sin_r;
                let ry = dx * sin_r + dy * cos_r;

                let r = (rx * rx + ry * ry).sqrt().max(0.2);
                let depth = 14.0 / r; // Perspective z

                let world_z = depth + self.cam_z;
                let ang = ry.atan2(rx);

                // Cylinder/Box unfolded UV coordinates
                let u = (ang / (2.0 * PI) + 0.5) * facet_density;
                let v = world_z * 0.45;

                // Discrete mirror tile indices
                let tile_u = u.floor();
                let tile_v = v.floor();
                let frac_u = u - tile_u;
                let frac_v = v - tile_v;

                // Pseudo-random facet tilt & normal vector
                let hash = ((tile_u * 37.13 + tile_v * 91.71).sin() * 43_758.547).fract().abs();
                let facet_tilt_x = (hash * 2.0 * PI).cos() * 0.4;
                let facet_tilt_y = (hash * 2.0 * PI).sin() * 0.4;

                // Specular reflection glint calculation
                let dist_to_light = ((rx - light_x + facet_tilt_x * 8.0).powi(2)
                    + (ry - light_y + facet_tilt_y * 8.0).powi(2)).sqrt();
                let specular_glint = (1.0 - dist_to_light / 10.0).max(0.0).powi(3);

                // Tile border / bevel seam (dark grout line between mirrors)
                let is_bevel = frac_u < 0.1 || frac_u > 0.9 || frac_v < 0.1 || frac_v > 0.9;

                let depth_fog = (r / 16.0).min(1.0);
                let base_ambient = 0.18 + mid * 0.15;

                let spec = if is_bevel {
                    0.05
                } else {
                    (base_ambient + specular_glint * 0.9 + self.strobe_pulse * 0.5 + bass * 0.25)
                        * (0.3 + 0.7 * depth_fog)
                };

                let col = Self::facet_color(pal, hash, self.color_cycle, spec);
                frame.set(x, y, col);
            }
        }

        // Treble specular glitter spots on mirrors
        if audio_active && treble > 0.4 {
            for i in 0..5 {
                let sx = (i * 7 + 3) % WIDTH;
                let sy = (i * 6 + 4) % HEIGHT;
                frame.set(sx, sy, Rgb::WHITE);
            }
        }
    }
}


#[cfg(test)]
mod timing_regression {
    use super::*;
    use matrix_core::AudioSnapshot;

    #[test]
    fn elapsed_time_advances_camera_and_decays_audio_with_engine_cap() {
        let mut effect = MirrorCavernEffect::new();
        effect.strobe_pulse = 1.0;
        let audio = AudioSnapshot::default();
        let mut frame = Frame::default();
        let render = |effect: &mut MirrorCavernEffect, frame: &mut Frame, dt| {
            effect.render(frame, &RenderContext { now_ms: 0, delta_seconds: dt, audio: &audio, color: Rgb::WHITE, text: "", scroll_speed: 1.0 });
        };
        render(&mut effect, &mut frame, 0.2);
        assert!((effect.cam_z - 1.38).abs() < 1e-5);
        assert!((effect.cam_rot - 0.184).abs() < 1e-5);
        assert!((effect.strobe_pulse - 0.3).abs() < 1e-5);
        render(&mut effect, &mut frame, 0.0);
        assert!((effect.cam_z - 1.38).abs() < 1e-5);
        assert!((effect.strobe_pulse - 0.3).abs() < 1e-5);
        render(&mut effect, &mut frame, 1.0);
        assert!((effect.cam_z - 4.38).abs() < 1e-5);
        assert_eq!(effect.strobe_pulse, 0.0);
    }
}
