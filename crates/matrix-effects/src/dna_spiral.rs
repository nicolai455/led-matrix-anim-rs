use crate::util::{audio_bass, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Bio Luminescent", "Cyber Synth", "Vaporwave", "Rainbow"];

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
        id: "frequency",
        label: "Frequency",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 6.0,
        step: 1.0,
        default: 2.0,
        options: &[],
        unit: "waves",
    },
    ParameterDescriptor {
        id: "width",
        label: "Width",
        kind: ParameterKind::Number,
        min: 4.0,
        max: 14.0,
        step: 1.0,
        default: 9.0,
        options: &[],
        unit: "px",
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

pub struct DnaSpiralEffect {
    values: [f32; 5],
    phase: f32,
    color_cycle: f32,
    kick: f32,
}

impl Default for DnaSpiralEffect {
    fn default() -> Self {
        Self {
            values: [35.0, 2.0, 9.0, 3.0, 1.0],
            phase: 0.0,
            color_cycle: 0.0,
            kick: 0.0,
        }
    }
}

impl DnaSpiralEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn strand_color(&self, strand: usize, z: f32, y: usize) -> Rgb {
        let pal = self.values[3] as usize;
        let depth_factor = clamp01(0.35 + 0.65 * ((z + 1.0) * 0.5));
        let y_norm = y as f32 / HEIGHT as f32;

        match pal {
            0 => {
                // Bio Luminescent (Cyan / Lime)
                if strand == 0 {
                    // Cyan
                    Rgb::new(
                        to_byte(20.0 * depth_factor),
                        to_byte(240.0 * depth_factor),
                        to_byte(220.0 * depth_factor),
                    )
                } else {
                    // Lime
                    Rgb::new(
                        to_byte(120.0 * depth_factor),
                        to_byte(255.0 * depth_factor),
                        to_byte(30.0 * depth_factor),
                    )
                }
            }
            1 => {
                // Cyber Synth (Magenta / Electric Blue)
                if strand == 0 {
                    Rgb::new(
                        to_byte(255.0 * depth_factor),
                        to_byte(20.0 * depth_factor),
                        to_byte(180.0 * depth_factor),
                    )
                } else {
                    Rgb::new(
                        to_byte(20.0 * depth_factor),
                        to_byte(140.0 * depth_factor),
                        to_byte(255.0 * depth_factor),
                    )
                }
            }
            2 => {
                // Vaporwave (Purple / Gold)
                if strand == 0 {
                    Rgb::new(
                        to_byte(180.0 * depth_factor),
                        to_byte(50.0 * depth_factor),
                        to_byte(240.0 * depth_factor),
                    )
                } else {
                    Rgb::new(
                        to_byte(255.0 * depth_factor),
                        to_byte(170.0 * depth_factor),
                        to_byte(40.0 * depth_factor),
                    )
                }
            }
            _ => {
                // Rainbow
                let h = (self.color_cycle + y_norm * 0.4 + strand as f32 * 0.5) % 1.0;
                hsv_to_rgb(h, 0.95, depth_factor)
            }
        }
    }

    fn rung_color(&self, t: f32, z_avg: f32) -> Rgb {
        let pal = self.values[3] as usize;
        let depth_factor = clamp01(0.25 + 0.55 * ((z_avg + 1.0) * 0.5));
        match pal {
            0 => Rgb::new(
                to_byte(80.0 * depth_factor),
                to_byte(240.0 * depth_factor),
                to_byte(140.0 * depth_factor),
            ),
            1 => Rgb::new(
                to_byte(160.0 * depth_factor),
                to_byte(80.0 * depth_factor),
                to_byte(220.0 * depth_factor),
            ),
            2 => Rgb::new(
                to_byte(220.0 * depth_factor),
                to_byte(110.0 * depth_factor),
                to_byte(160.0 * depth_factor),
            ),
            _ => {
                let h = (self.color_cycle + t * 0.25) % 1.0;
                hsv_to_rgb(h, 0.75, depth_factor)
            }
        }
    }
}

impl Effect for DnaSpiralEffect {
    fn id(&self) -> &'static str {
        "dna_spiral"
    }

    fn name(&self) -> &'static str {
        "DNA Spiral"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "frequency" => Some(self.values[1]),
            "width" => Some(self.values[2]),
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
            "frequency" => self.values[1] = value,
            "width" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.color_cycle = 0.0;
        self.kick = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::BLACK);

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[4] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };

        if audio_active && context.audio.beat {
            self.kick = 1.0;
        } else {
            self.kick = (self.kick - dt * 2.5).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + self.kick * 0.8);
        self.phase = (self.phase + dt * speed * 2.0) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.18) % 1.0;

        let waves = self.values[1];
        let base_width = self.values[2] * (1.0 + bass * 0.3);
        let center_x = 15.5f32;

        // Render line by line
        for y in 0..HEIGHT {
            let yf = y as f32;
            let angle = self.phase + (yf / HEIGHT as f32) * waves * 2.0 * PI;

            // Audio waveform displacement for rungs
            let wave_disp = if audio_active {
                let wave_idx = ((y * 128) / HEIGHT).min(127);
                context.audio.waveform[wave_idx] * 2.5
            } else {
                0.0
            };

            let sin_a = angle.sin();
            let cos_a = angle.cos();

            let x1 = center_x + sin_a * base_width;
            let z1 = cos_a;

            let x2 = center_x - sin_a * base_width;
            let z2 = -cos_a;

            // Draw horizontal rungs every 2 lines
            if y % 2 == 0 {
                let (min_x, max_x) = if x1 < x2 { (x1, x2) } else { (x2, x1) };
                let start = (min_x.floor() as i32).clamp(0, WIDTH as i32 - 1) as usize;
                let end = (max_x.ceil() as i32).clamp(0, WIDTH as i32 - 1) as usize;

                if end > start {
                    let z_avg = (z1 + z2) * 0.5;
                    for x in start..=end {
                        let span = (end - start).max(1) as f32;
                        let t = (x - start) as f32 / span;
                        let rung_c = self.rung_color(t, z_avg);
                        let py = (yf + wave_disp * (t * PI).sin()).round() as i32;
                        if py >= 0 && py < HEIGHT as i32 {
                            frame.set(x, py as usize, rung_c);
                        }
                    }
                }
            }

            // Draw the two strands with depth priority (back strand first, front strand on top)
            let (first, second) = if z1 < z2 {
                ((x1, z1, 0usize), (x2, z2, 1usize))
            } else {
                ((x2, z2, 1usize), (x1, z1, 0usize))
            };

            for &(sx, sz, strand_idx) in &[first, second] {
                let col = self.strand_color(strand_idx, sz, y);
                let xi = sx.round() as i32;
                if xi >= 0 && xi < WIDTH as i32 {
                    frame.set(xi as usize, y, col);
                }
                // Thicker front strand
                if sz > 0.4 {
                    if xi > 0 {
                        let dim_col = Rgb::new(col.r / 2, col.g / 2, col.b / 2);
                        frame.set((xi - 1) as usize, y, dim_col);
                    }
                    if xi + 1 < WIDTH as i32 {
                        let dim_col = Rgb::new(col.r / 2, col.g / 2, col.b / 2);
                        frame.set((xi + 1) as usize, y, dim_col);
                    }
                }
            }
        }
    }
}
