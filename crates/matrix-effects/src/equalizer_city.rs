use crate::util::{audio_bass, audio_mid, audio_treble, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Synthwave Sunset", "Cyber Matrix", "Tokyo Neon", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Skyline speed",
        kind: ParameterKind::Number,
        min: 5.0,
        max: 100.0,
        step: 1.0,
        default: 30.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "glow",
        label: "Neon glow",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 70.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "peak_decay",
        label: "Peak decay",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 90.0,
        step: 1.0,
        default: 50.0,
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

pub struct EqualizerCityEffect {
    values: [f32; 5],
    smooth_bands: [f32; 16],
    peak_y: [f32; 16],
    time_phase: f32,
    color_cycle: f32,
    beat_flare: f32,
}

impl Default for EqualizerCityEffect {
    fn default() -> Self {
        Self {
            values: [30.0, 70.0, 50.0, 3.0, 1.0],
            smooth_bands: [0.0; 16],
            peak_y: [31.0; 16],
            time_phase: 0.0,
            color_cycle: 0.0,
            beat_flare: 0.0,
        }
    }
}

impl EqualizerCityEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn window_color(pal: usize, col_idx: usize, row_idx: usize, color_cycle: f32) -> Rgb {
        let height_t = row_idx as f32 / 24.0;
        match pal {
            0 => {
                // Synthwave Sunset: Deep Magenta at base -> Electric Violet -> Bright Yellow/Orange spire
                if height_t > 0.8 {
                    Rgb::new(255, 230, 40)
                } else if height_t > 0.5 {
                    Rgb::new(255, 60, 140)
                } else {
                    Rgb::new(120, 20, 200)
                }
            }
            1 => {
                // Cyber Matrix: Dark Jade at base -> Neon Emerald -> Crystalline White
                if height_t > 0.85 {
                    Rgb::new(220, 255, 240)
                } else if height_t > 0.4 {
                    Rgb::new(0, 255, 120)
                } else {
                    Rgb::new(0, 140, 60)
                }
            }
            2 => {
                // Tokyo Neon: Laser Cyan & Hot Pink checker
                #[allow(clippy::manual_is_multiple_of)]
                if (col_idx + row_idx) % 2 == 0 {
                    Rgb::new(0, 240, 255)
                } else {
                    Rgb::new(255, 30, 160)
                }
            }
            _ => {
                // Rainbow Fade
                let h = (color_cycle + (col_idx as f32 / 16.0) * 0.7 + height_t * 0.3) % 1.0;
                hsv_to_rgb(h, 0.95, 1.0)
            }
        }
    }
}

impl Effect for EqualizerCityEffect {
    fn id(&self) -> &'static str {
        "equalizer_city"
    }

    fn name(&self) -> &'static str {
        "Equalizer Cyber City"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "glow" => Some(self.values[1]),
            "peak_decay" => Some(self.values[2]),
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
            "glow" => self.values[1] = value,
            "peak_decay" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.smooth_bands = [0.0; 16];
        self.peak_y = [31.0; 16];
        self.time_phase = 0.0;
        self.color_cycle = 0.0;
        self.beat_flare = 0.0;
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
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.beat_flare = 1.0;
        } else {
            self.beat_flare = (self.beat_flare - dt * 3.0).max(0.0);
        }

        let speed = self.values[0] / 30.0;
        self.time_phase = (self.time_phase + dt * speed * 1.5) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.14) % 1.0;

        let pal = self.values[3] as usize;
        let glow_scale = self.values[1] / 100.0;
        let peak_decay_rate = 12.0 * (self.values[2] / 50.0);

        // 1. Cyber Sky & Synthwave Retro Sun in Background
        let sun_cx = 15.5f32;
        let sun_cy = 13.0f32;
        let sun_r = 7.0f32 + bass * 2.0;
        let sun_r_sq = sun_r * sun_r;

        for y in 0..20 {
            let yf = y as f32;
            let dy = yf - sun_cy;

            // Horizontal sun blinds / stripes
            let in_blind = (yf * 1.2).sin() < -0.3 && yf > sun_cy;

            for x in 0..WIDTH {
                let dx = x as f32 - sun_cx;
                let d2 = dx * dx + dy * dy;

                if d2 <= sun_r_sq && !in_blind {
                    let t = (1.0 - (d2 / sun_r_sq).sqrt()).clamp(0.0, 1.0);
                    let sun_c = if pal == 1 {
                        Rgb::new(to_byte(20.0 * t), to_byte(255.0 * t), to_byte(180.0 * t))
                    } else {
                        Rgb::new(255, to_byte((120.0 + 130.0 * t) * glow_scale), to_byte(30.0 * t))
                    };
                    frame.set(x, y, sun_c);
                } else if y > 15 {
                    // Horizon haze
                    let haze = (y - 15) as f32 / 5.0;
                    let haze_byte = to_byte(haze * 25.0);
                    frame.set(x, y, Rgb::new(haze_byte, 0, haze_byte * 2));
                }
            }
        }

        // 2. 16 Skyscraper Towers across Matrix
        // Each tower has width 2 (16 towers * 2 = 32 pixels wide)
        let ground_y = 30i32;

        for col in 0..16 {
            let band_val = if audio_active {
                context.audio.bands[col]
            } else {
                let synthetic_pulse = ((self.time_phase * 2.0 + col as f32 * 0.5).sin() * 0.5 + 0.5).powf(1.5);
                synthetic_pulse * 0.8
            };

            // Smooth band dynamics
            let target = band_val.clamp(0.0, 1.0);
            self.smooth_bands[col] += (target - self.smooth_bands[col]) * (dt * 18.0).min(1.0);

            let tower_height = (self.smooth_bands[col] * 22.0).round() as i32;
            let tower_top_y = (ground_y - tower_height).clamp(4, ground_y);

            // Peak antenna indicator with gravity drop
            if (tower_top_y as f32) < self.peak_y[col] {
                self.peak_y[col] = tower_top_y as f32;
            } else {
                self.peak_y[col] = (self.peak_y[col] + dt * peak_decay_rate).min(ground_y as f32);
            }

            let x0 = col * 2;
            let x1 = x0 + 1;

            // Draw tower body & windows
            for y in tower_top_y..=ground_y {
                let row_from_base = (ground_y - y) as usize;
                #[allow(clippy::manual_is_multiple_of)]
                let is_window = y % 2 == 0;

                let col_rgb = if is_window {
                    Self::window_color(pal, col, row_from_base, self.color_cycle)
                } else {
                    // Dark concrete structure
                    Rgb::new(15, 15, 25)
                };

                frame.set(x0, y as usize, col_rgb);
                frame.set(x1, y as usize, col_rgb);
            }

            // Draw Peak Spire Antenna
            let py = self.peak_y[col].round() as i32;
            if py >= 2 && py < ground_y {
                let peak_c = if beat {
                    Rgb::WHITE
                } else {
                    Rgb::new(255, to_byte(180.0 + treble * 75.0), 200)
                };
                frame.set(x0, py as usize, peak_c);
            }
        }

        // 3. Ground Street Grid (y = 31)
        for x in 0..WIDTH {
            #[allow(clippy::manual_is_multiple_of)]
            let is_lane = (x + (self.time_phase * 10.0) as usize) % 4 == 0;
            let street_pulse = if is_lane {
                Rgb::new(to_byte(180.0 + mid * 70.0), to_byte(40.0 + bass * 180.0), 255)
            } else {
                Rgb::new(20, 10, 40)
            };
            frame.set(x, 31, street_pulse);
        }

        // Lightning flash on beat drop
        if self.beat_flare > 0.4 {
            let f_byte = to_byte(self.beat_flare * 140.0);
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let cur = frame.get(x, y);
                    frame.set(
                        x,
                        y,
                        Rgb::new(
                            cur.r.saturating_add(f_byte),
                            cur.g.saturating_add(f_byte),
                            cur.b.saturating_add(f_byte),
                        ),
                    );
                }
            }
        }
    }
}
