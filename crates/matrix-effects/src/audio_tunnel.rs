use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Hyperdrive", "Neon", "Fire", "Rainbow"];

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
        id: "twist",
        label: "Twist",
        kind: ParameterKind::Number,
        min: 0.0,
        max: 100.0,
        step: 1.0,
        default: 30.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "rings",
        label: "Rings",
        kind: ParameterKind::Number,
        min: 2.0,
        max: 16.0,
        step: 1.0,
        default: 8.0,
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

pub struct AudioTunnelEffect {
    values: [f32; 5],
    z_offset: f32,
    warp_boost: f32,
    smooth_bands: [f32; 16],
    smooth_level: f32,
    color_cycle: f32,
}

impl Default for AudioTunnelEffect {
    fn default() -> Self {
        Self {
            values: [35.0, 30.0, 8.0, 3.0, 1.0],
            z_offset: 0.0,
            warp_boost: 0.0,
            smooth_bands: [0.0; 16],
            smooth_level: 0.0,
            color_cycle: 0.0,
        }
    }
}

impl AudioTunnelEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn sample_palette(
        &self,
        palette_idx: usize,
        depth_t: f32,
        ring_val: f32,
        rib_val: f32,
        band_energy: f32,
        total_intensity: f32,
    ) -> Rgb {
        if total_intensity <= 0.015 {
            return Rgb::BLACK;
        }

        match palette_idx {
            // Hyperdrive: Cosmic blue / cyan warp with blinding white hyperdrive peaks
            0 => {
                let hue = (0.54 + 0.10 * depth_t + 0.03 * rib_val).rem_euclid(1.0);
                let whiteness = (self.warp_boost * 0.7
                    + total_intensity.powi(3) * 0.5
                    + band_energy * 0.3)
                    .clamp(0.0, 0.95);
                let sat = (0.95 - whiteness).clamp(0.05, 1.0);
                let val = total_intensity;
                hsv_to_rgb(hue, sat, val)
            }
            // Neon: Synthwave magenta rings, electric cyan ribs, deep violet background
            1 => {
                let is_rib = (rib_val / (ring_val + rib_val + 0.001)).clamp(0.0, 1.0);
                let hue = if is_rib > 0.45 {
                    0.50 + 0.04 * depth_t
                } else {
                    0.86 + 0.06 * depth_t
                };
                let sat = (1.0 - self.warp_boost * 0.5).clamp(0.25, 1.0);
                let val = total_intensity;
                hsv_to_rgb(hue, sat, val)
            }
            // Fire: Scorching inferno from crimson abyss to yellow/white-hot magma rings
            2 => {
                let heat = (depth_t * 0.25
                    + ring_val * 0.55
                    + rib_val * 0.35
                    + self.warp_boost * 0.4
                    + band_energy * 0.25)
                    .clamp(0.0, 1.0);
                let r = to_byte((total_intensity * 255.0).min(255.0));
                let g = to_byte((total_intensity * (heat * 210.0 + self.warp_boost * 45.0)).min(255.0));
                let b = to_byte(
                    (total_intensity * (heat.powi(3) * 160.0 + self.warp_boost * 80.0)).min(255.0),
                );
                Rgb::new(r, g, b)
            }
            // Rainbow: Dynamic continuous rainbow spectrum rings and twisting ribs
            _ => {
                let hue = (self.color_cycle + depth_t * 0.8 + rib_val * 0.25).rem_euclid(1.0);
                hsv_to_rgb(hue, 1.0, total_intensity)
            }
        }
    }
}

impl Effect for AudioTunnelEffect {
    fn id(&self) -> &'static str {
        "audio_tunnel"
    }

    fn name(&self) -> &'static str {
        "Audio Tunnel"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "twist" => Some(self.values[1]),
            "rings" => Some(self.values[2]),
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
            "twist" => self.values[1] = value,
            "rings" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.z_offset = 0.0;
        self.warp_boost = 0.0;
        self.smooth_bands = [0.0; 16];
        self.smooth_level = 0.0;
    }

    fn deactivate(&mut self) {}

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let dt = context.delta_seconds.clamp(0.0, 0.1);
        let speed_val = self.values[0];
        let twist_val = self.values[1];
        let rings_val = self.values[2];
        let palette_idx = self.values[3].round() as usize;
        let audio_enabled = self.values[4] != 0.0 && context.audio.available;

        // Process audio snapshot features
        let (bass, mid, treble) = if audio_enabled {
            (
                audio_bass(context.audio),
                audio_mid(context.audio),
                audio_treble(context.audio),
            )
        } else {
            (0.0, 0.0, 0.0)
        };

        if audio_enabled {
            for i in 0..16 {
                let target = context.audio.bands[i];
                if target >= self.smooth_bands[i] {
                    self.smooth_bands[i] = target;
                } else {
                    self.smooth_bands[i] = (self.smooth_bands[i] - 3.5 * dt).max(0.0);
                }
            }

            let smoothing = (dt * 12.0).clamp(0.0, 1.0);
            let target_level = context.audio.level.max(bass);
            self.smooth_level += (target_level - self.smooth_level) * smoothing;

            // Audio beat triggers an intense forward warp speed surge
            if context.audio.beat {
                self.warp_boost = (self.warp_boost + 0.85).min(1.5);
            }
        } else {
            for i in 0..16 {
                self.smooth_bands[i] = (self.smooth_bands[i] - 2.5 * dt).max(0.0);
            }
            self.smooth_level = (self.smooth_level - 2.5 * dt).max(0.0);
        }

        // Warp boost exponential decay
        self.warp_boost *= (-5.5 * dt).exp();

        // Advance tunnel depth coordinate z
        let base_speed = (speed_val / 35.0) * 3.2;
        let warp_surge = 1.0 + self.warp_boost * 2.8 + bass * 0.6;
        let speed = base_speed * warp_surge;
        self.z_offset = (self.z_offset + dt * speed).rem_euclid(1000.0);
        self.color_cycle = (self.color_cycle + dt * 0.15).rem_euclid(1.0);

        let twist_factor = (twist_val / 100.0) * 0.40;
        let ring_freq = rings_val * 0.35;
        let two_pi = 2.0 * PI;

        // Modulate ring thickness and brightness by audio level
        let thickness_mod = (self.smooth_level * 0.65 + mid * 0.35).clamp(0.0, 1.0);
        let ring_exp = 5.0 - thickness_mod * 3.4; // Exponent 5.0 (thin) down to 1.6 (bold)
        let ring_bright_boost = 0.65 + 0.55 * self.smooth_level + 0.35 * bass;

        let center_x = 15.5f32;
        let center_y = 15.5f32;
        const CONSTANT: f32 = 14.0;
        const EPSILON: f32 = 0.35;

        for y in 0..HEIGHT {
            let dy = y as f32 - center_y;
            for x in 0..WIDTH {
                let dx = x as f32 - center_x;
                let r = (dx * dx + dy * dy).sqrt();
                let angle = dy.atan2(dx);

                // Tunnel depth coordinate z = (constant / (r + epsilon)) + time * speed
                let depth = CONSTANT / (r + EPSILON);
                let z = depth + self.z_offset;

                // Twist angle = angle + z * twist_factor
                let twist_angle = angle + z * twist_factor;

                // Concentric depth slices mapped to the 16 frequency bands
                // Outer/near rings (r large, depth ~0.6..2.0) -> bass/mid bands (0..4)
                // Inner/distant rings (r small, depth ~6.0..13.5) -> high frequency bands (11..15)
                let depth_t = clamp01((depth - 0.6) / 12.0);
                let band_pos = depth_t * 15.0;
                let b0 = (band_pos as usize).min(15);
                let b1 = (b0 + 1).min(15);
                let b_frac = band_pos - b0 as f32;
                let band_energy =
                    self.smooth_bands[b0] * (1.0 - b_frac) + self.smooth_bands[b1] * b_frac;

                // Concentric rings
                let ring_phase = z * ring_freq;
                let ring_cos = (ring_phase * two_pi).cos();
                let ring_norm = (ring_cos * 0.5 + 0.5).clamp(0.0, 1.0);
                let ring_val = ring_norm.powf(ring_exp);
                let ring_bright = (ring_val * ring_bright_boost).clamp(0.0, 1.0);

                // 8 Radial ribs twisting along the tunnel length
                let rib_cos = (twist_angle * 8.0).cos();
                let rib_norm = (rib_cos * 0.5 + 0.5).clamp(0.0, 1.0);
                let rib_val = rib_norm.powf(5.5);
                let rib_bright = rib_val * (0.35 + 0.45 * treble + 0.30 * self.smooth_level);

                // Center vanishing point warp core glow
                let core_proximity = (1.0 - r / 2.8).max(0.0);
                let core_glow = core_proximity * (0.30 + 0.70 * self.warp_boost + 0.50 * bass);

                // Combine rings, ribs, and core glow
                let total_intensity =
                    (ring_bright + rib_bright + core_glow + band_energy * 0.25).clamp(0.0, 1.0);

                let color = self.sample_palette(
                    palette_idx,
                    depth_t,
                    ring_val,
                    rib_val,
                    band_energy,
                    total_intensity,
                );

                frame.set(x, y, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matrix_core::AudioSnapshot;

    #[test]
    fn test_audio_tunnel_metadata() {
        let effect = AudioTunnelEffect::new();
        assert_eq!(effect.id(), "audio_tunnel");
        assert_eq!(effect.name(), "Audio Tunnel");
        assert_eq!(effect.parameters().len(), 5);

        let ids: Vec<&str> = effect.parameters().iter().map(|p| p.id).collect();
        assert_eq!(
            ids,
            vec!["speed", "twist", "rings", "palette", "audio_reactive"]
        );
    }

    #[test]
    fn test_audio_tunnel_parameters() {
        let mut effect = AudioTunnelEffect::new();

        // Defaults
        assert_eq!(effect.parameter("speed"), Some(35.0));
        assert_eq!(effect.parameter("twist"), Some(30.0));
        assert_eq!(effect.parameter("rings"), Some(8.0));
        assert_eq!(effect.parameter("palette"), Some(3.0));
        assert_eq!(effect.parameter("audio_reactive"), Some(1.0));
        assert_eq!(effect.parameter("unknown"), None);

        // Valid ranges
        assert!(effect.set_parameter("speed", 1.0).is_ok());
        assert!(effect.set_parameter("speed", 100.0).is_ok());
        assert!(effect.set_parameter("speed", 0.0).is_err());
        assert!(effect.set_parameter("speed", 101.0).is_err());

        assert!(effect.set_parameter("twist", 0.0).is_ok());
        assert!(effect.set_parameter("twist", 100.0).is_ok());
        assert!(effect.set_parameter("twist", -1.0).is_err());
        assert!(effect.set_parameter("twist", 101.0).is_err());

        assert!(effect.set_parameter("rings", 2.0).is_ok());
        assert!(effect.set_parameter("rings", 16.0).is_ok());
        assert!(effect.set_parameter("rings", 1.0).is_err());
        assert!(effect.set_parameter("rings", 17.0).is_err());

        assert!(effect.set_parameter("palette", 0.0).is_ok());
        assert!(effect.set_parameter("palette", 3.0).is_ok());
        assert!(effect.set_parameter("palette", 4.0).is_err());

        assert!(effect.set_parameter("audio_reactive", 0.0).is_ok());
        assert!(effect.set_parameter("audio_reactive", 1.0).is_ok());
        assert!(effect.set_parameter("audio_reactive", 2.0).is_err());
    }

    #[test]
    fn test_audio_tunnel_render_and_palettes() {
        let mut effect = AudioTunnelEffect::new();
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

        for pal in 0..=3 {
            effect.set_parameter("palette", pal as f32).unwrap();
            effect.render(&mut frame, &ctx);

            let non_black_count = frame.pixels.iter().filter(|&&p| p != Rgb::BLACK).count();
            assert!(
                non_black_count > 0,
                "Palette {} rendered completely black frame",
                pal
            );
        }
    }

    #[test]
    fn test_audio_tunnel_audio_reactivity() {
        let mut effect = AudioTunnelEffect::new();
        let mut frame = Frame::default();

        let audio = AudioSnapshot {
            available: true,
            level: 0.9,
            peak: 1.0,
            beat: true,
            bands: [0.85; 16],
            ..Default::default()
        };

        let ctx = RenderContext {
            now_ms: 500,
            delta_seconds: 0.033,
            audio: &audio,
            color: Rgb::WHITE,
            text: "",
            scroll_speed: 1.0,
        };

        effect.render(&mut frame, &ctx);
        assert!(
            effect.warp_boost > 0.0,
            "Beat should trigger forward warp rush boost"
        );

        // Test difference between silent and active audio render
        let mut effect_silent = AudioTunnelEffect::new();
        let mut frame_silent = Frame::default();
        let silent_audio = AudioSnapshot {
            available: false,
            ..Default::default()
        };
        let ctx_silent = RenderContext {
            now_ms: 500,
            delta_seconds: 0.033,
            audio: &silent_audio,
            color: Rgb::WHITE,
            text: "",
            scroll_speed: 1.0,
        };
        effect_silent.render(&mut frame_silent, &ctx_silent);

        assert_ne!(
            frame_silent.pixels, frame.pixels,
            "Tunnel render should visibly react to audio"
        );
    }

    #[test]
    fn test_audio_tunnel_reset_and_deactivate() {
        let mut effect = AudioTunnelEffect::new();
        let mut frame = Frame::default();
        let audio = AudioSnapshot {
            available: true,
            level: 0.9,
            peak: 1.0,
            beat: true,
            bands: [0.85; 16],
            ..Default::default()
        };
        let ctx = RenderContext {
            now_ms: 500,
            delta_seconds: 0.033,
            audio: &audio,
            color: Rgb::WHITE,
            text: "",
            scroll_speed: 1.0,
        };

        effect.render(&mut frame, &ctx);
        effect.deactivate();
        effect.reset();

        assert_eq!(effect.z_offset, 0.0);
        assert_eq!(effect.warp_boost, 0.0);
        assert_eq!(effect.smooth_level, 0.0);
    }
}
