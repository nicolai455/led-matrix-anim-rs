use crate::util::{alloc_boxed_2d, audio_bass, audio_mid, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const RATIOS: &[&str] = &[
    "1:2 Harmony",
    "3:4 Knot",
    "5:6 Web",
    "2:3 Infinity",
    "Waveform XY",
];

const PALETTES: &[&str] = &["Neon Phosphor", "Amber CRT", "Laser Cyan", "Rainbow"];

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
        id: "ratio",
        label: "Ratio",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 4.0,
        step: 1.0,
        default: 0.0,
        options: RATIOS,
        unit: "",
    },
    ParameterDescriptor {
        id: "trail",
        label: "Trail length",
        kind: ParameterKind::Number,
        min: 20.0,
        max: 95.0,
        step: 1.0,
        default: 75.0,
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

pub struct LissajousEffect {
    values: [f32; 5],
    phase: f32,
    delta: f32,
    color_cycle: f32,
    buffer: Box<[[Rgb; WIDTH]; HEIGHT]>,
}

impl Default for LissajousEffect {
    fn default() -> Self {
        Self {
            values: [30.0, 0.0, 75.0, 3.0, 1.0],
            phase: 0.0,
            delta: 0.0,
            color_cycle: 0.0,
            buffer: alloc_boxed_2d(Rgb::BLACK),
        }
    }
}

impl LissajousEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn sample_color(&self, t_norm: f32) -> Rgb {
        let pal = self.values[3] as usize;
        match pal {
            0 => {
                // Neon Phosphor (Classic CRT Green)
                let intensity = 0.7 + 0.3 * (t_norm * 2.0 * PI).sin();
                Rgb::new(
                    to_byte(30.0 * intensity),
                    to_byte(255.0 * intensity),
                    to_byte(40.0 * intensity),
                )
            }
            1 => {
                // Amber CRT
                let intensity = 0.75 + 0.25 * (t_norm * 2.0 * PI).sin();
                Rgb::new(
                    to_byte(255.0 * intensity),
                    to_byte(160.0 * intensity),
                    to_byte(20.0 * intensity),
                )
            }
            2 => {
                // Laser Cyan
                let intensity = 0.7 + 0.3 * (t_norm * 2.0 * PI).sin();
                Rgb::new(
                    to_byte(10.0 * intensity),
                    to_byte(230.0 * intensity),
                    to_byte(255.0 * intensity),
                )
            }
            _ => {
                // Rainbow Fade
                let h = (self.color_cycle + t_norm) % 1.0;
                hsv_to_rgb(h, 1.0, 1.0)
            }
        }
    }

    fn draw_point(&mut self, px: f32, py: f32, color: Rgb) {
        let x0 = px.floor() as i32;
        let y0 = py.floor() as i32;
        let fx = px - px.floor();
        let fy = py - py.floor();

        // 4-pixel bilinear anti-aliased splat
        for dy in 0..=1 {
            for dx in 0..=1 {
                let xi = x0 + dx;
                let yi = y0 + dy;
                if xi >= 0 && xi < WIDTH as i32 && yi >= 0 && yi < HEIGHT as i32 {
                    let wx = if dx == 0 { 1.0 - fx } else { fx };
                    let wy = if dy == 0 { 1.0 - fy } else { fy };
                    let weight = wx * wy;
                    if weight > 0.05 {
                        let target = &mut self.buffer[yi as usize][xi as usize];
                        target.r = target.r.saturating_add((color.r as f32 * weight) as u8);
                        target.g = target.g.saturating_add((color.g as f32 * weight) as u8);
                        target.b = target.b.saturating_add((color.b as f32 * weight) as u8);
                    }
                }
            }
        }
    }
}

impl Effect for LissajousEffect {
    fn id(&self) -> &'static str {
        "lissajous"
    }

    fn name(&self) -> &'static str {
        "Lissajous"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "ratio" => Some(self.values[1]),
            "trail" => Some(self.values[2]),
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
            "ratio" => self.values[1] = value,
            "trail" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.delta = 0.0;
        self.color_cycle = 0.0;
        self.buffer = alloc_boxed_2d(Rgb::BLACK);
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[4] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };

        // Decay buffer for phosphor trail
        let trail_decay = (self.values[2] / 100.0).clamp(0.2, 0.96);
        let decay_mult = trail_decay.powf(dt * 30.0);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let px = &mut self.buffer[y][x];
                px.r = (px.r as f32 * decay_mult) as u8;
                px.g = (px.g as f32 * decay_mult) as u8;
                px.b = (px.b as f32 * decay_mult) as u8;
            }
        }

        let speed = self.values[0] / 30.0;
        self.color_cycle = (self.color_cycle + dt * 0.16) % 1.0;

        let delta_speed = if audio_active && context.audio.beat {
            2.5
        } else {
            0.6
        };
        self.delta = (self.delta + dt * speed * delta_speed) % (2.0 * PI);
        self.phase = (self.phase + dt * speed * 1.5) % (2.0 * PI);

        let ratio_mode = self.values[1] as usize;
        let center_x = 15.5f32;
        let center_y = 15.5f32;
        let radius = (13.5 * (1.0 + bass * 0.25)).min(15.0);

        const SAMPLES: usize = 256;

        if ratio_mode == 4 && audio_active {
            // Waveform XY mode (Oscilloscope)
            for i in 0..64 {
                let t_norm = i as f32 / 64.0;
                let wx = context.audio.waveform[i] * radius * 1.3;
                let wy = context.audio.waveform[(i + 32) % 128] * radius * 1.3;
                let px = center_x + wx;
                let py = center_y + wy;
                let col = self.sample_color(t_norm);
                self.draw_point(px, py, col);
            }
        } else {
            let (wx, wy) = match ratio_mode {
                1 => (3.0, 4.0),
                2 => (5.0, 6.0),
                3 => (2.0, 3.0),
                _ => (1.0, 2.0),
            };

            let wobble = if audio_active { mid * 0.2 } else { 0.0 };

            for i in 0..SAMPLES {
                let t = (i as f32 / SAMPLES as f32) * 2.0 * PI;
                let t_norm = i as f32 / SAMPLES as f32;

                let px = center_x + radius * (wx * t + self.phase + self.delta).sin() * (1.0 + wobble * (t * 8.0).sin());
                let py = center_y + radius * (wy * t + self.phase).sin() * (1.0 + wobble * (t * 8.0).cos());

                let col = self.sample_color(t_norm);
                self.draw_point(px, py, col);
            }
        }

        // Copy buffer to output frame
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                frame.set(x, y, self.buffer[y][x]);
            }
        }
    }
}
