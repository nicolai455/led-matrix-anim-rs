use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const SYMMETRIES: &[&str] = &["4-Fold", "6-Fold", "8-Fold", "12-Fold"];
const PALETTES: &[&str] = &["Rainbow", "Cyberpunk", "Emerald", "Sunset"];

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
        id: "symmetry",
        label: "Symmetry",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 3.0,
        step: 1.0,
        default: 1.0,
        options: SYMMETRIES,
        unit: "",
    },
    ParameterDescriptor {
        id: "zoom",
        label: "Zoom",
        kind: ParameterKind::Number,
        min: 5.0,
        max: 50.0,
        step: 1.0,
        default: 20.0,
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
];

pub struct KaleidoscopeEffect {
    values: [f32; 5],
    rotation: f32,
    time_phase: f32,
    zoom_pulse: f32,
    rot_burst: f32,
    palette_phase: f32,
}

impl Default for KaleidoscopeEffect {
    fn default() -> Self {
        Self {
            values: [30.0, 1.0, 20.0, 0.0, 1.0],
            rotation: 0.0,
            time_phase: 0.0,
            zoom_pulse: 0.0,
            rot_burst: 0.0,
            palette_phase: 0.0,
        }
    }
}

impl KaleidoscopeEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn sample_palette(
        &self,
        palette_idx: usize,
        pattern: f32,
        palette_phase: f32,
        mid: f32,
        r: f32,
    ) -> Rgb {
        let pattern = clamp01(pattern);
        match palette_idx {
            // Prism: rich rainbow dispersion cycling with rotation and audio harmonics
            0 => {
                let hue = (pattern * 0.85 + palette_phase + mid * 0.20 + (r / 22.0) * 0.15)
                    .rem_euclid(1.0);
                let sat = 0.95;
                let val = 0.30 + 0.70 * pattern;
                hsv_to_rgb(hue, sat, val)
            }
            // Cyberpunk: neon cyan -> deep purple -> hot pink -> white glow
            1 => {
                let p = (pattern + palette_phase + mid * 0.15).rem_euclid(1.0);
                if p < 0.5 {
                    let f = p / 0.5;
                    let hue = 0.50 + 0.25 * f;
                    let sat = 0.92;
                    let val = 0.25 + 0.75 * pattern;
                    hsv_to_rgb(hue, sat, val)
                } else {
                    let f = (p - 0.5) / 0.5;
                    let hue = 0.85 + 0.10 * f;
                    let sat = (0.95 - 0.35 * (f * f)).clamp(0.4, 0.95);
                    let val = 0.30 + 0.70 * pattern;
                    hsv_to_rgb(hue, sat, val)
                }
            }
            // Emerald: teal ocean -> jade green -> vibrant lime/gold
            2 => {
                let p = (pattern + palette_phase + mid * 0.15).rem_euclid(1.0);
                if p < 0.55 {
                    let f = p / 0.55;
                    let hue = 0.45 - 0.12 * f;
                    let sat = 0.95;
                    let val = 0.25 + 0.75 * pattern;
                    hsv_to_rgb(hue, sat, val)
                } else {
                    let f = (p - 0.55) / 0.45;
                    let hue = 0.33 - 0.16 * f;
                    let sat = (0.95 - 0.25 * f).clamp(0.5, 0.95);
                    let val = 0.30 + 0.70 * pattern;
                    hsv_to_rgb(hue, sat, val)
                }
            }
            // Sunset: dusk indigo -> fiery crimson -> glowing orange/gold
            _ => {
                let p = (pattern + palette_phase + mid * 0.15).rem_euclid(1.0);
                if p < 0.33 {
                    let f = p / 0.33;
                    let hue = 0.75 + 0.23 * f;
                    let sat = 0.95;
                    let val = 0.25 + 0.75 * pattern;
                    hsv_to_rgb(hue, sat, val)
                } else if p < 0.70 {
                    let f = (p - 0.33) / 0.37;
                    let hue = (0.98 + 0.09 * f).rem_euclid(1.0);
                    let sat = 0.95;
                    let val = 0.28 + 0.72 * pattern;
                    hsv_to_rgb(hue, sat, val)
                } else {
                    let f = (p - 0.70) / 0.30;
                    let hue = 0.07 + 0.07 * f;
                    let sat = (0.95 - 0.25 * f).clamp(0.5, 0.95);
                    let val = 0.32 + 0.68 * pattern;
                    hsv_to_rgb(hue, sat, val)
                }
            }
        }
    }
}

impl Effect for KaleidoscopeEffect {
    fn id(&self) -> &'static str {
        "kaleidoscope"
    }

    fn name(&self) -> &'static str {
        "Kaleidoscope"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "symmetry" => Some(self.values[1]),
            "zoom" => Some(self.values[2]),
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
            "symmetry" => self.values[1] = value,
            "zoom" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.rotation = 0.0;
        self.time_phase = 0.0;
        self.zoom_pulse = 0.0;
        self.rot_burst = 0.0;
        self.palette_phase = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let dt = context.delta_seconds.clamp(0.0, 0.1);
        let speed_factor = self.values[0] / 30.0;
        let audio_enabled = self.values[4] != 0.0 && context.audio.available;

        let mut bass = 0.0;
        let mut mid = 0.0;
        let mut treble = 0.0;

        if audio_enabled {
            bass = audio_bass(context.audio);
            mid = audio_mid(context.audio);
            treble = audio_treble(context.audio);

            // Beat triggers rotational jump/burst or zoom pulse
            if context.audio.beat {
                self.zoom_pulse = 1.0;
                self.rot_burst = (self.rot_burst + 0.40).min(1.0);
                self.rotation = (self.rotation + 0.15).rem_euclid(2.0 * PI);
                self.palette_phase = (self.palette_phase + 0.125).rem_euclid(1.0);
            }
        }

        // Decay beat impulses
        let decay = (-5.0 * dt).exp();
        self.zoom_pulse *= decay;
        self.rot_burst *= decay;

        // Dynamic rotation with burst acceleration
        let rot_speed = (0.35 + self.rot_burst * 1.2) * speed_factor;
        self.rotation = (self.rotation + rot_speed * dt).rem_euclid(2.0 * PI);

        // Pattern time phase evolution
        self.time_phase = (self.time_phase + dt * 1.5 * speed_factor).rem_euclid(2000.0 * PI);
        self.palette_phase = (self.palette_phase + dt * 0.06 * speed_factor).rem_euclid(1.0);

        // Symmetry: 4-Fold (0), 6-Fold (1), 8-Fold (2), 12-Fold (3)
        let folds = match self.values[1].round() as usize {
            0 => 4.0,
            1 => 6.0,
            2 => 8.0,
            3 => 12.0,
            _ => 6.0,
        };
        let sector = (2.0 * PI) / folds;
        let half_sector = PI / folds;

        // Bass expands radius / zoom factor
        let base_zoom = self.values[2] * 0.012;
        let effective_zoom = base_zoom * (1.0 + bass * 0.50 + self.zoom_pulse * 0.35);

        // Treble modulates fine harmonic ripple
        let ripple_freq = 3.0 + treble * 4.0;
        let ripple_amp = 0.25 + treble * 0.75;

        let palette_idx = (self.values[3].round() as usize).min(3);

        // Render 32x32 polar kaleidoscope
        for y in 0..HEIGHT {
            let dy = y as f32 - 15.5;
            for x in 0..WIDTH {
                let dx = x as f32 - 15.5;
                let r = (dx * dx + dy * dy).sqrt();
                let theta = dy.atan2(dx) + self.rotation;

                // Reflect theta into symmetric sector: angle = abs(angle % (2*PI/N) - PI/N)
                let angle = (theta.rem_euclid(sector) - half_sector).abs();

                // Compute dynamic rotating wave pattern / fractal coordinate warp
                let u = r * angle.cos() * effective_zoom;
                let v = r * angle.sin() * effective_zoom;

                let wu = u + 0.45 * (v * 1.5 + self.time_phase * 0.8).sin();
                let wv = v + 0.45 * (u * 1.5 - self.time_phase * 0.7).cos();

                // Sample dynamic pattern with multiple harmonic sines
                let h1 = (wu * 1.2 + self.time_phase).sin();
                let h2 = (wv * 1.3 - self.time_phase * 0.85).cos();
                let wr = (wu * wu + wv * wv).sqrt();
                let h3 = (wr * 1.6 - self.time_phase * 1.3).sin();
                let h4 = ((wu + wv) * ripple_freq + self.time_phase * 2.2).sin() * ripple_amp;

                let raw = (h1 + h2 + h3 + h4) / (2.6 + ripple_amp * 0.5);
                let pattern = clamp01(raw * 0.5 + 0.5);

                let mut color =
                    self.sample_palette(palette_idx, pattern, self.palette_phase, mid, r);

                // Add center glow flare on beat pulse
                let center_glow = (1.0 - (r / 5.0)).max(0.0) * self.zoom_pulse;
                if center_glow > 0.0 {
                    let r_byte = to_byte(color.r as f32 + center_glow * 100.0);
                    let g_byte = to_byte(color.g as f32 + center_glow * 100.0);
                    let b_byte = to_byte(color.b as f32 + center_glow * 100.0);
                    color = Rgb::new(r_byte, g_byte, b_byte);
                }

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
    fn test_kaleidoscope_metadata_and_parameters() {
        let mut effect = KaleidoscopeEffect::new();
        assert_eq!(effect.id(), "kaleidoscope");
        assert_eq!(effect.name(), "Kaleidoscope");

        let params = effect.parameters();
        assert_eq!(params.len(), 5);

        assert_eq!(effect.parameter("speed"), Some(30.0));
        assert_eq!(effect.parameter("symmetry"), Some(1.0));
        assert_eq!(effect.parameter("zoom"), Some(20.0));
        assert_eq!(effect.parameter("palette"), Some(0.0));
        assert_eq!(effect.parameter("audio_reactive"), Some(1.0));
        assert_eq!(effect.parameter("nonexistent"), None);

        // Validation bounds
        assert!(effect.set_parameter("speed", 50.0).is_ok());
        assert_eq!(effect.parameter("speed"), Some(50.0));
        assert!(effect.set_parameter("speed", 0.0).is_err());
        assert!(effect.set_parameter("speed", 101.0).is_err());

        assert!(effect.set_parameter("symmetry", 3.0).is_ok());
        assert!(effect.set_parameter("symmetry", 4.0).is_err());

        assert!(effect.set_parameter("zoom", 5.0).is_ok());
        assert!(effect.set_parameter("zoom", 50.0).is_ok());
        assert!(effect.set_parameter("zoom", 4.0).is_err());
        assert!(effect.set_parameter("zoom", 51.0).is_err());

        assert!(effect.set_parameter("palette", 2.0).is_ok());
        assert!(effect.set_parameter("palette", 4.0).is_err());

        assert!(effect.set_parameter("audio_reactive", 0.0).is_ok());
        assert!(effect.set_parameter("audio_reactive", 2.0).is_err());

        assert!(effect.set_parameter("nonexistent", 1.0).is_err());
    }

    #[test]
    fn test_kaleidoscope_render_lifecycle() {
        let mut effect = KaleidoscopeEffect::new();
        let mut frame = Frame::default();
        let mut audio = AudioSnapshot {
            available: true,
            level: 0.8,
            beat: true,
            ..Default::default()
        };
        audio.bands[1] = 0.9;
        audio.bands[6] = 0.7;
        audio.bands[12] = 0.8;

        let ctx = RenderContext {
            now_ms: 50,
            delta_seconds: 0.016,
            audio: &audio,
            color: Rgb::WHITE,
            text: "",
            scroll_speed: 1.0,
        };

        for pal in 0..4 {
            effect.set_parameter("palette", pal as f32).unwrap();
            for sym in 0..4 {
                effect.set_parameter("symmetry", sym as f32).unwrap();
                effect.render(&mut frame, &ctx);
            }
        }

        effect.reset();
        assert_eq!(effect.rotation, 0.0);
        assert_eq!(effect.time_phase, 0.0);

        effect.deactivate();
        assert_eq!(effect.rotation, 0.0);
    }
}
