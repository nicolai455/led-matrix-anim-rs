use crate::util::{alloc_boxed_2d, audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Electric Ruby", "Cyber Magenta", "Aqua Pulse", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Pulse tempo",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 45.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "glow_layers",
        label: "Neon glow rings",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 4.0,
        step: 1.0,
        default: 3.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "ecg_line",
        label: "ECG wave trace",
        kind: ParameterKind::Boolean,
        min: 0.0,
        max: 1.0,
        step: 1.0,
        default: 1.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Neon color",
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

pub struct NeonHeartEffect {
    values: [f32; 5],
    heart_phase: f32,
    color_cycle: f32,
    ecg_scroll: f32,
    beat_surge: f32,
    decay_buffer: Box<[[Rgb; WIDTH]; HEIGHT]>,
}

impl Default for NeonHeartEffect {
    fn default() -> Self {
        Self {
            values: [45.0, 3.0, 1.0, 3.0, 1.0],
            heart_phase: 0.0,
            color_cycle: 0.0,
            ecg_scroll: 0.0,
            beat_surge: 0.0,
            decay_buffer: alloc_boxed_2d(Rgb::BLACK),
        }
    }
}

impl NeonHeartEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn neon_color(pal: usize, color_cycle: f32, layer: usize, brightness: f32) -> Rgb {
        let b = clamp01(brightness);
        let l_norm = layer as f32 * 0.25;

        match pal {
            0 => {
                // Electric Ruby (Deep red aura -> Hot incandescent pink/white core)
                if layer == 0 {
                    Rgb::new(255, to_byte(180.0 * b), to_byte(200.0 * b))
                } else {
                    let r = to_byte(255.0 * b);
                    let g = to_byte(20.0 * b);
                    let blue = to_byte(50.0 * b);
                    Rgb::new(r, g, blue)
                }
            }
            1 => {
                // Cyber Magenta (Violet aura -> Neon Magenta core)
                let r = to_byte(255.0 * b);
                let g = to_byte((20.0 + 80.0 * l_norm) * b);
                let blue = to_byte((220.0 - 50.0 * l_norm) * b);
                Rgb::new(r, g, blue)
            }
            2 => {
                // Aqua Pulse (Deep marine -> Electric Cyan core)
                let r = to_byte((10.0 + 30.0 * l_norm) * b);
                let g = to_byte((230.0 + 25.0 * l_norm) * b);
                let blue = to_byte(255.0 * b);
                Rgb::new(r, g, blue)
            }
            _ => {
                // Rainbow Fade
                let h = (color_cycle + l_norm) % 1.0;
                hsv_to_rgb(h, 0.95, b)
            }
        }
    }
}

impl Effect for NeonHeartEffect {
    fn id(&self) -> &'static str {
        "neon_heart"
    }

    fn name(&self) -> &'static str {
        "Pulsing Neon Heart"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "glow_layers" => Some(self.values[1]),
            "ecg_line" => Some(self.values[2]),
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
            "glow_layers" => self.values[1] = value,
            "ecg_line" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.heart_phase = 0.0;
        self.color_cycle = 0.0;
        self.ecg_scroll = 0.0;
        self.beat_surge = 0.0;
        self.decay_buffer = alloc_boxed_2d(Rgb::BLACK);
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
            self.beat_surge = 1.0;
        } else {
            self.beat_surge = (self.beat_surge - dt * 3.2).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.7);
        self.heart_phase = (self.heart_phase + dt * speed * 4.5) % (2.0 * PI);
        self.ecg_scroll = (self.ecg_scroll + dt * speed * 25.0) % (WIDTH as f32);
        self.color_cycle = (self.color_cycle + dt * 0.14) % 1.0;

        // Double heartbeat contraction waveform ("lub-dub" pulse)
        let t_cycle = self.heart_phase / (2.0 * PI);
        let pulse_shape = if t_cycle < 0.25 {
            (t_cycle * 4.0 * PI).sin()
        } else if t_cycle < 0.45 {
            ((t_cycle - 0.25) * 5.0 * PI).sin() * 0.6
        } else {
            0.0
        };

        let base_scale = 1.0 + pulse_shape.max(0.0) * 0.22 + bass * 0.18 + self.beat_surge * 0.25;

        // Fade phosphor trail buffer
        let decay_mult = 0.76f32.powf(dt * 30.0);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let px = &mut self.decay_buffer[y][x];
                px.r = (px.r as f32 * decay_mult) as u8;
                px.g = (px.g as f32 * decay_mult) as u8;
                px.b = (px.b as f32 * decay_mult) as u8;
            }
        }

        let pal = self.values[3] as usize;
        let num_layers = (self.values[1].round() as usize).clamp(1, 4);

        let cx = 15.5f32;
        let cy = 15.0f32;

        // Sample parametric heart curve
        // x(t) = 16 * sin^3(t)
        // y(t) = 13 * cos(t) - 5 * cos(2t) - 2 * cos(3t) - cos(4t)
        const STEPS: usize = 120;

        for layer in 0..num_layers {
            let layer_scale = base_scale * (0.8 + layer as f32 * 0.18);
            let layer_brightness = 1.0 - (layer as f32 * 0.22);
            let col = Self::neon_color(pal, self.color_cycle, layer, layer_brightness);

            for i in 0..STEPS {
                let t = (i as f32 / STEPS as f32) * 2.0 * PI;
                let sin_t = t.sin();
                let cos_t = t.cos();

                let hx = 16.0 * sin_t * sin_t * sin_t;
                let hy = -(13.0 * cos_t - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos());

                // Scale to matrix coordinates
                let px = (cx + (hx * 0.72) * layer_scale).round() as i32;
                let py = (cy + (hy * 0.72) * layer_scale).round() as i32;

                if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                    let cur = &mut self.decay_buffer[py as usize][px as usize];
                    cur.r = cur.r.saturating_add(col.r);
                    cur.g = cur.g.saturating_add(col.g);
                    cur.b = cur.b.saturating_add(col.b);
                }
            }
        }

        // Draw ECG heart monitor line across matrix (y = 16)
        let show_ecg = self.values[2] > 0.5;
        if show_ecg {
            for x in 0..WIDTH {
                let xf = x as f32;
                let rel_x = (xf - self.ecg_scroll).rem_euclid(WIDTH as f32);

                // ECG heartbeat spike shape at rel_x in 12..18
                let spike = if (12.0..14.0).contains(&rel_x) {
                    -2.0
                } else if (14.0..16.0).contains(&rel_x) {
                    8.0 + bass * 4.0
                } else if (16.0..18.0).contains(&rel_x) {
                    -4.0
                } else if audio_active {
                    let wave_sample = context.audio.waveform[(x * 4) % 128];
                    wave_sample * (4.0 + mid * 3.0)
                } else {
                    0.0
                };

                let ey = (cy + spike).round() as i32;
                if ey >= 0 && ey < HEIGHT as i32 {
                    let ecg_col = Rgb::new(to_byte(120.0 + treble * 135.0), 255, to_byte(200.0 + mid * 55.0));
                    let cur = &mut self.decay_buffer[ey as usize][x];
                    cur.r = cur.r.saturating_add(ecg_col.r);
                    cur.g = cur.g.saturating_add(ecg_col.g);
                    cur.b = cur.b.saturating_add(ecg_col.b);
                }
            }
        }

        // Copy buffer to output frame
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                frame.set(x, y, self.decay_buffer[y][x]);
            }
        }
    }
}
