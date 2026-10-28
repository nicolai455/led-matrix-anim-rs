use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Electric", "Magma", "Ocean", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Speed",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 100.0,
        step: 1.0,
        default: 35.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "blobs",
        label: "Blobs",
        kind: ParameterKind::Number,
        min: 2.0,
        max: 5.0,
        step: 1.0,
        default: 3.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Palette",
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
    ParameterDescriptor {
        id: "glow",
        label: "Glow",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 50.0,
        options: &[],
        unit: "%",
    },
];

const MAX_BLOBS: usize = 5;

#[derive(Clone, Copy, Debug)]
struct Blob {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    base_radius: f32,
}

pub struct MetaballsEffect {
    values: [f32; 5],
    blobs: [Blob; MAX_BLOBS],
    color_phase: f32,
    kick_vel: f32,
}

impl Default for MetaballsEffect {
    fn default() -> Self {
        Self {
            values: [35.0, 3.0, 0.0, 1.0, 50.0],
            blobs: Self::initial_blobs(),
            color_phase: 0.0,
            kick_vel: 0.0,
        }
    }
}

impl MetaballsEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn initial_blobs() -> [Blob; MAX_BLOBS] {
        [
            Blob {
                x: 8.0,
                y: 10.0,
                vx: 9.5,
                vy: 7.2,
                base_radius: 5.5,
            },
            Blob {
                x: 22.0,
                y: 8.0,
                vx: -8.1,
                vy: 10.3,
                base_radius: 6.0,
            },
            Blob {
                x: 16.0,
                y: 22.0,
                vx: 7.4,
                vy: -8.8,
                base_radius: 5.0,
            },
            Blob {
                x: 24.0,
                y: 24.0,
                vx: -9.0,
                vy: -6.5,
                base_radius: 5.2,
            },
            Blob {
                x: 6.0,
                y: 20.0,
                vx: 8.2,
                vy: -7.5,
                base_radius: 4.8,
            },
        ]
    }

    fn sample_palette(&self, t: f32, mid_treble_shift: f32) -> Rgb {
        let palette_idx = self.values[2].round() as usize;
        let t = (t + self.color_phase + mid_treble_shift).rem_euclid(1.0);
        let two_pi = 2.0 * PI;

        match palette_idx {
            // Electric: deep cobalt -> vivid cyan -> bright violet/white
            0 => {
                let r = to_byte(255.0 * (0.45 + 0.55 * (t * two_pi - 1.2).sin()));
                let g = to_byte(255.0 * (0.65 + 0.35 * (t * two_pi + 0.8).sin()));
                let b = to_byte(255.0 * (0.85 + 0.15 * (t * two_pi).sin()));
                Rgb::new(r, g, b)
            }
            // Magma: dark ember red -> fiery vermilion -> bright incandescent gold
            1 => {
                let r = to_byte(255.0 * (0.85 + 0.15 * (t * two_pi).sin()));
                let g = to_byte(255.0 * (0.45 + 0.45 * (t * two_pi - 0.7).sin()));
                let b = to_byte(255.0 * (0.15 + 0.15 * (t * two_pi + 1.2).sin()));
                Rgb::new(r, g, b)
            }
            // Ocean: deep oceanic navy -> turquoise -> radiant seafoam
            2 => {
                let r = to_byte(255.0 * (0.15 + 0.15 * (t * two_pi).sin()));
                let g = to_byte(255.0 * (0.65 + 0.35 * (t * two_pi + 0.5).sin()));
                let b = to_byte(255.0 * (0.80 + 0.20 * (t * two_pi + 1.0).sin()));
                Rgb::new(r, g, b)
            }
            // Rainbow
            _ => hsv_to_rgb(t, 1.0, 1.0),
        }
    }
}

impl Effect for MetaballsEffect {
    fn id(&self) -> &'static str {
        "metaballs"
    }

    fn name(&self) -> &'static str {
        "Metaballs"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "blobs" => Some(self.values[1]),
            "palette" => Some(self.values[2]),
            "audio_reactive" => Some(self.values[3]),
            "glow" => Some(self.values[4]),
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
            "blobs" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            "glow" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.blobs = Self::initial_blobs();
        self.color_phase = 0.0;
        self.kick_vel = 0.0;
    }

    fn deactivate(&mut self) {}

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let dt = context.delta_seconds.clamp(0.0, 0.1);
        let speed_factor = self.values[0] / 35.0;
        let active_blobs = (self.values[1].round() as usize).clamp(2, MAX_BLOBS);
        let audio_enabled = self.values[3] != 0.0 && context.audio.available;

        let mut bass_boost = 0.0;
        let mut mid_treble_shift = 0.0;

        if audio_enabled {
            bass_boost = audio_bass(context.audio);
            let mid = audio_mid(context.audio);
            let treble = audio_treble(context.audio);
            mid_treble_shift = (mid * 0.4 + treble * 0.6) * 0.5;

            if context.audio.beat {
                self.kick_vel = 2.5;
                self.color_phase = (self.color_phase + 0.15).rem_euclid(1.0);
            }
        }

        // Kick decays smoothly back to 0.0
        self.kick_vel = (self.kick_vel - dt * 6.0).max(0.0);
        let impulse = 1.0 + self.kick_vel;

        self.color_phase = (self.color_phase + dt * 0.08 * speed_factor).rem_euclid(1.0);

        // Update blob positions and bounce off boundaries
        let w_bound = WIDTH as f32;
        let h_bound = HEIGHT as f32;

        for blob in &mut self.blobs[..active_blobs] {
            let vx = blob.vx * speed_factor * impulse;
            let vy = blob.vy * speed_factor * impulse;

            blob.x += vx * dt;
            blob.y += vy * dt;

            // Bounce with soft margin to stay within matrix
            let margin = 2.0;
            if blob.x < margin {
                blob.x = margin;
                blob.vx = blob.vx.abs();
            } else if blob.x >= w_bound - margin {
                blob.x = w_bound - margin - 0.01;
                blob.vx = -blob.vx.abs();
            }

            if blob.y < margin {
                blob.y = margin;
                blob.vy = blob.vy.abs();
            } else if blob.y >= h_bound - margin {
                blob.y = h_bound - margin - 0.01;
                blob.vy = -blob.vy.abs();
            }
        }

        // Glow threshold: bass boosts blob radius and glow threshold
        let glow_param = self.values[4] / 100.0;
        let glow_threshold = (glow_param * (1.0 + bass_boost * 0.6)).clamp(0.1, 1.5);
        let threshold = (0.75 - glow_threshold * 0.35).max(0.1);

        // Precompute radius squared for active blobs, scaled by bass
        let mut effective_radii_sq = [0.0f32; MAX_BLOBS];
        let mut blob_coords = [(0.0f32, 0.0f32); MAX_BLOBS];
        for i in 0..active_blobs {
            let r = self.blobs[i].base_radius * (1.0 + bass_boost * 0.5);
            effective_radii_sq[i] = r * r;
            blob_coords[i] = (self.blobs[i].x, self.blobs[i].y);
        }

        for y in 0..HEIGHT {
            let fy = y as f32 + 0.5;
            for x in 0..WIDTH {
                let fx = x as f32 + 0.5;

                // Field value: sum of (R^2 / distance^2)
                let mut field = 0.0f32;
                for i in 0..active_blobs {
                    let (bx, by) = blob_coords[i];
                    let dx = fx - bx;
                    let dy = fy - by;
                    let dist_sq = dx * dx + dy * dy + 1.0;
                    field += effective_radii_sq[i] / dist_sq;
                }

                if field > threshold {
                    // Inside blob core / body
                    let intensity = ((field - threshold) / (2.0 + threshold)).min(1.0);
                    let palette_color = self.sample_palette(intensity, mid_treble_shift);

                    // Add rim glow and brightness boost near center
                    let brightness = clamp01(0.25 + intensity * 0.75 + bass_boost * 0.2);
                    let r = to_byte(palette_color.r as f32 * brightness);
                    let g = to_byte(palette_color.g as f32 * brightness);
                    let b = to_byte(palette_color.b as f32 * brightness);

                    frame.set(x, y, Rgb::new(r, g, b));
                } else {
                    // Soft outer glow falloff
                    let glow_val = (field / threshold).powi(3) * 0.20 * glow_threshold;
                    if glow_val > 0.02 {
                        let base_col = self.sample_palette(0.0, mid_treble_shift);
                        let r = to_byte(base_col.r as f32 * glow_val);
                        let g = to_byte(base_col.g as f32 * glow_val);
                        let b = to_byte(base_col.b as f32 * glow_val);
                        frame.set(x, y, Rgb::new(r, g, b));
                    } else {
                        frame.set(x, y, Rgb::BLACK);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matrix_core::AudioSnapshot;

    #[test]
    fn test_metaballs_metadata() {
        let effect = MetaballsEffect::new();
        assert_eq!(effect.id(), "metaballs");
        assert_eq!(effect.name(), "Metaballs");
        assert_eq!(effect.parameters().len(), 5);

        let ids: Vec<&str> = effect.parameters().iter().map(|p| p.id).collect();
        assert_eq!(
            ids,
            vec!["speed", "blobs", "palette", "audio_reactive", "glow"]
        );
    }

    #[test]
    fn test_metaballs_parameters() {
        let mut effect = MetaballsEffect::new();

        // Defaults
        assert_eq!(effect.parameter("speed"), Some(35.0));
        assert_eq!(effect.parameter("blobs"), Some(3.0));
        assert_eq!(effect.parameter("palette"), Some(0.0));
        assert_eq!(effect.parameter("audio_reactive"), Some(1.0));
        assert_eq!(effect.parameter("glow"), Some(50.0));
        assert_eq!(effect.parameter("non_existent"), None);

        // Validation bounds
        assert!(effect.set_parameter("speed", 1.0).is_ok());
        assert!(effect.set_parameter("speed", 100.0).is_ok());
        assert!(effect.set_parameter("speed", 0.0).is_err());
        assert!(effect.set_parameter("speed", 101.0).is_err());

        assert!(effect.set_parameter("blobs", 2.0).is_ok());
        assert!(effect.set_parameter("blobs", 5.0).is_ok());
        assert!(effect.set_parameter("blobs", 1.0).is_err());
        assert!(effect.set_parameter("blobs", 6.0).is_err());

        assert!(effect.set_parameter("palette", 0.0).is_ok());
        assert!(effect.set_parameter("palette", 3.0).is_ok());
        assert!(effect.set_parameter("palette", 4.0).is_err());

        assert!(effect.set_parameter("audio_reactive", 0.0).is_ok());
        assert!(effect.set_parameter("audio_reactive", 1.0).is_ok());

        assert!(effect.set_parameter("glow", 10.0).is_ok());
        assert!(effect.set_parameter("glow", 100.0).is_ok());
        assert!(effect.set_parameter("glow", 5.0).is_err());
    }

    #[test]
    fn test_metaballs_render_and_palettes() {
        let mut effect = MetaballsEffect::new();
        let mut frame = Frame::default();
        let audio = AudioSnapshot::default();
        let ctx = RenderContext {
            now_ms: 1000,
            delta_seconds: 0.016,
            audio: &audio,
            color: Rgb::WHITE,
            text: "",
            scroll_speed: 1.0,
        };

        for palette_idx in 0..=3 {
            effect.set_parameter("palette", palette_idx as f32).unwrap();
            effect.render(&mut frame, &ctx);

            let non_black = frame.pixels.iter().any(|&p| p != Rgb::BLACK);
            assert!(
                non_black,
                "Palette {} rendered completely black frame",
                palette_idx
            );
        }
    }

    #[test]
    fn test_metaballs_audio_reactivity() {
        let mut effect = MetaballsEffect::new();
        let mut frame = Frame::default();

        let mut audio = AudioSnapshot {
            available: true,
            level: 0.8,
            peak: 0.9,
            beat: true,
            bands: [0.8; 16],
            ..Default::default()
        };

        let ctx1 = RenderContext {
            now_ms: 500,
            delta_seconds: 0.033,
            audio: &audio,
            color: Rgb::WHITE,
            text: "",
            scroll_speed: 1.0,
        };

        effect.render(&mut frame, &ctx1);
        assert!(effect.kick_vel > 0.0, "Beat should trigger kick velocity");

        // Next frame without beat
        audio.beat = false;
        let prev_kick = effect.kick_vel;
        let ctx2 = RenderContext {
            now_ms: 533,
            delta_seconds: 0.033,
            audio: &audio,
            color: Rgb::WHITE,
            text: "",
            scroll_speed: 1.0,
        };
        effect.render(&mut frame, &ctx2);
        assert!(
            effect.kick_vel < prev_kick,
            "Kick velocity should decay over time"
        );
    }

    #[test]
    fn test_metaballs_reset_and_deactivate() {
        let mut effect = MetaballsEffect::new();
        let initial_blob0 = effect.blobs[0];

        let mut frame = Frame::default();
        let audio = AudioSnapshot::default();
        let ctx = RenderContext {
            now_ms: 100,
            delta_seconds: 0.05,
            audio: &audio,
            color: Rgb::WHITE,
            text: "",
            scroll_speed: 1.0,
        };
        effect.render(&mut frame, &ctx);

        // Position should have changed
        assert_ne!(effect.blobs[0].x, initial_blob0.x);

        effect.reset();
        assert_eq!(effect.blobs[0].x, initial_blob0.x);
        assert_eq!(effect.color_phase, 0.0);
        assert_eq!(effect.kick_vel, 0.0);

        effect.deactivate(); // should not panic
    }
}
