use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Emerald 532nm", "Electric Cyan & Pink", "Red & Blue Laser", "Rainbow Fan"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Vibration speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 50.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "fan_spread",
        label: "Beam fan spread",
        kind: ParameterKind::Number,
        min: 8.0,
        max: 20.0,
        step: 1.0,
        default: 14.0,
        options: &[],
        unit: "deg",
    },
    ParameterDescriptor {
        id: "num_strings",
        label: "Laser strings",
        kind: ParameterKind::Number,
        min: 5.0,
        max: 11.0,
        step: 2.0,
        default: 9.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Laser color",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 3.0,
        step: 1.0,
        default: 1.0,
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

pub struct LaserHarpEffect {
    values: [f32; 5],
    timer: f32,
    vibration_phase: f32,
    string_pluck: [f32; 11],
    strobe_pulse: f32,
    color_cycle: f32,
}

impl Default for LaserHarpEffect {
    fn default() -> Self {
        Self {
            values: [50.0, 14.0, 9.0, 1.0, 1.0],
            timer: 0.0,
            vibration_phase: 0.0,
            string_pluck: [0.0; 11],
            strobe_pulse: 0.0,
            color_cycle: 0.0,
        }
    }
}

impl LaserHarpEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn string_color(str_idx: usize, pal: usize, color_cycle: f32, is_plucked: bool) -> Rgb {
        let base_c = match pal {
            0 => {
                // Emerald 532nm (Laser Green)
                Rgb::new(20, 255, 60)
            }
            1 => {
                // Electric Cyan & Pink
                #[allow(clippy::manual_is_multiple_of)]
                if str_idx % 2 == 0 {
                    Rgb::new(0, 240, 255)
                } else {
                    Rgb::new(255, 20, 160)
                }
            }
            2 => {
                // Red & Blue Laser
                #[allow(clippy::manual_is_multiple_of)]
                if str_idx % 2 == 0 {
                    Rgb::new(255, 15, 30)
                } else {
                    Rgb::new(20, 110, 255)
                }
            }
            _ => {
                // Rainbow Fan
                let h = ((str_idx as f32 / 10.0) + color_cycle) % 1.0;
                hsv_to_rgb(h, 0.95, 1.0)
            }
        };

        if is_plucked {
            Rgb::new(
                base_c.r.saturating_add(140),
                base_c.g.saturating_add(140),
                base_c.b.saturating_add(140),
            )
        } else {
            base_c
        }
    }
}

impl Effect for LaserHarpEffect {
    fn id(&self) -> &'static str {
        "laser_harp"
    }

    fn name(&self) -> &'static str {
        "Laser Harp"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "fan_spread" => Some(self.values[1]),
            "num_strings" => Some(self.values[2]),
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
            "fan_spread" => self.values[1] = value,
            "num_strings" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.timer = 0.0;
        self.vibration_phase = 0.0;
        self.string_pluck = [0.0; 11];
        self.strobe_pulse = 0.0;
        self.color_cycle = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::new(6, 6, 14)); // Deep stage darkness

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[4] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.strobe_pulse = 1.0;
        } else {
            self.strobe_pulse = (self.strobe_pulse - dt * 4.0).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.8);
        self.timer += dt * speed;
        self.vibration_phase = (self.vibration_phase + dt * speed * 25.0) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let pal = self.values[3] as usize;
        let total_strings = self.values[2].round() as usize;
        let fan_half_spread = self.values[1];

        let base_x = 15.5f32;

        // Stage Floor / Projector Base at bottom row (y = 30..=31)
        for x in 0..WIDTH {
            frame.set(x, 30, Rgb::new(45, 50, 60));
            frame.set(x, 31, Rgb::new(20, 22, 28));
        }
        // Projector emitter aperture
        frame.set(14, 30, Rgb::WHITE);
        frame.set(15, 30, Rgb::WHITE);
        frame.set(16, 30, Rgb::WHITE);
        frame.set(17, 30, Rgb::WHITE);

        // Update string vibration & plucks
        for s_idx in 0..total_strings {
            let band_val = if audio_active { context.audio.bands[s_idx % 16] } else { 0.0 };
            let should_pluck = (band_val > 0.55) || (beat && (s_idx == 0 || s_idx == total_strings - 1));

            if should_pluck {
                self.string_pluck[s_idx] = 1.0;
            } else {
                self.string_pluck[s_idx] = (self.string_pluck[s_idx] - dt * 5.0).max(0.0);
            }
        }

        // Render each laser harp string beam rising from (base_x, base_y) to ceiling (y = 0)
        for s_idx in 0..total_strings {
            let s_norm = if total_strings > 1 {
                (s_idx as f32 / (total_strings - 1) as f32) * 2.0 - 1.0 // -1.0 to +1.0
            } else {
                0.0
            };

            let ceiling_target_x = base_x + s_norm * fan_half_spread;
            let pluck_int = self.string_pluck[s_idx];
            let is_hot = pluck_int > 0.45;
            let col = Self::string_color(s_idx, pal, self.color_cycle, is_hot);

            let band_val = if audio_active { context.audio.bands[s_idx % 16] } else { 0.2 };

            // Trace beam line upward from base_y down to 0
            for py in 0..=30i32 {
                let t = (30 - py) as f32 / 30.0; // 0 at base, 1 at ceiling
                let mut beam_x = base_x + (ceiling_target_x - base_x) * t;

                // Standing wave vibration on plucked string
                let standing_wave = (t * PI).sin() * (self.vibration_phase + s_idx as f32 * 2.0).sin();
                let vib_amp = pluck_int * 2.5 + band_val * 1.5;
                beam_x += standing_wave * vib_amp;

                let px_i = beam_x.round() as i32;

                if px_i >= 0 && px_i < WIDTH as i32 {
                    let intensity = clamp01(0.75 + pluck_int * 0.4 + self.strobe_pulse * 0.25);
                    let r_add = to_byte(col.r as f32 * intensity);
                    let g_add = to_byte(col.g as f32 * intensity);
                    let b_add = to_byte(col.b as f32 * intensity);

                    frame.add_rgb(px_i as usize, py as usize, r_add, g_add, b_add);

                    // Pluck hand sparkle ring at mid height (py ~ 14)
                    if is_hot && py == 14 {
                        if px_i > 0 { frame.set((px_i - 1) as usize, py as usize, Rgb::WHITE); }
                        if px_i + 1 < WIDTH as i32 { frame.set((px_i + 1) as usize, py as usize, Rgb::WHITE); }
                    }
                }
            }
        }

        // Haze illumination motes on treble / mid
        if audio_active && treble > 0.35 {
            for i in 0..4 {
                let sx = (i * 9 + 5) % WIDTH;
                let sy = (i * 7 + 4) % 28;
                let cur = frame.get(sx, sy);
                frame.set(
                    sx,
                    sy,
                    Rgb::new(
                        cur.r.saturating_add(150),
                        cur.g.saturating_add(220),
                        cur.b.saturating_add(255),
                    ),
                );
            }
        }
        if audio_active && mid > 0.35 {
            let cur = frame.get(15, 10);
            frame.set(15, 10, Rgb::new(cur.r.saturating_add(80), cur.g.saturating_add(80), cur.b.saturating_add(80)));
        }
    }
}
