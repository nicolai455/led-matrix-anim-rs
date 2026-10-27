use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Cyan Cyberwire", "Synthwave Neon", "Emerald Matrix", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Wave speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 35.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "grid_density",
        label: "Grid lines",
        kind: ParameterKind::Number,
        min: 8.0,
        max: 20.0,
        step: 2.0,
        default: 12.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "wave_height",
        label: "Elevation scale",
        kind: ParameterKind::Number,
        min: 2.0,
        max: 10.0,
        step: 1.0,
        default: 5.0,
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

pub struct AudioMeshEffect {
    values: [f32; 5],
    phase: f32,
    color_cycle: f32,
    drop_wave: f32,
}

impl Default for AudioMeshEffect {
    fn default() -> Self {
        Self {
            values: [35.0, 12.0, 5.0, 3.0, 1.0],
            phase: 0.0,
            color_cycle: 0.0,
            drop_wave: 0.0,
        }
    }
}

impl AudioMeshEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn line(frame: &mut Frame, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgb) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        let mut cx = x0;
        let mut cy = y0;

        loop {
            if cx >= 0 && cx < WIDTH as i32 && cy >= 0 && cy < HEIGHT as i32 {
                let cur = frame.get(cx as usize, cy as usize);
                frame.set(
                    cx as usize,
                    cy as usize,
                    Rgb::new(
                        cur.r.saturating_add(color.r),
                        cur.g.saturating_add(color.g),
                        cur.b.saturating_add(color.b),
                    ),
                );
            }

            if cx == x1 && cy == y1 {
                break;
            }

            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                cx += sx;
            }
            if e2 <= dx {
                err += dx;
                cy += sy;
            }
        }
    }

    fn wire_color(pal: usize, color_cycle: f32, elevation_norm: f32, depth_norm: f32) -> Rgb {
        let el = clamp01(elevation_norm);
        let d = clamp01(depth_norm);
        let b = 0.35 + 0.65 * (d * 0.7 + el * 0.3);

        match pal {
            0 => {
                // Cyan Cyberwire (Deep navy -> Laser cyan -> Pure white peaks)
                let r = to_byte(255.0 * el.powi(3) * b);
                let g = to_byte((140.0 + 115.0 * el) * b);
                let blue = to_byte(255.0 * b);
                Rgb::new(r, g, blue)
            }
            1 => {
                // Synthwave Neon (Dark violet valley -> Hot pink ridges -> Yellow peaks)
                if el > 0.7 {
                    let yel = (el - 0.7) / 0.3;
                    Rgb::new(to_byte(255.0 * b), to_byte(220.0 * yel * b), to_byte(40.0 * b))
                } else {
                    let r = to_byte(255.0 * b);
                    let g = to_byte(20.0 * b);
                    let blue = to_byte((160.0 + 80.0 * (1.0 - el)) * b);
                    Rgb::new(r, g, blue)
                }
            }
            2 => {
                // Emerald Matrix (Forest green depth -> Neon Lime wire)
                let r = to_byte(40.0 * el * b);
                let g = to_byte(255.0 * b);
                let blue = to_byte(80.0 * el * b);
                Rgb::new(r, g, blue)
            }
            _ => {
                // Rainbow Fade
                let h = (color_cycle + depth_norm * 0.4 + el * 0.3) % 1.0;
                hsv_to_rgb(h, 0.95, b)
            }
        }
    }
}

impl Effect for AudioMeshEffect {
    fn id(&self) -> &'static str {
        "audio_mesh"
    }

    fn name(&self) -> &'static str {
        "3D Ripple Mesh"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "grid_density" => Some(self.values[1]),
            "wave_height" => Some(self.values[2]),
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
            "grid_density" => self.values[1] = value,
            "wave_height" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.color_cycle = 0.0;
        self.drop_wave = 0.0;
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
            self.drop_wave = 1.0;
        } else {
            self.drop_wave = (self.drop_wave - dt * 2.2).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.7);
        self.phase = (self.phase + dt * speed * 2.5) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.14) % 1.0;

        let pal = self.values[3] as usize;
        let grid_n = (self.values[1].round() as usize).clamp(8, 20);
        let max_height = self.values[2] * (1.0 + bass * 0.6);

        // Perspective 3D isometric projection grid:
        // World coordinates: u in [-1..1], v in [0..1] (v=0 is distant horizon, v=1 is foreground)
        // Project to screen (sx, sy):
        // sx = 15.5 + u * (14.0 + v * 12.0)
        // sy = 8.0 + v * 20.0 - z * elevation

        let mut pts = [[(0i32, 0i32, Rgb::BLACK); 21]; 21];

        #[allow(clippy::needless_range_loop)]
        for (vi, pt_row) in pts.iter_mut().enumerate().take(grid_n) {
            let v = vi as f32 / (grid_n - 1) as f32;
            let depth_norm = v; // 0=far, 1=near

            for ui in 0..grid_n {
                let u = (ui as f32 / (grid_n - 1) as f32) * 2.0 - 1.0;

                // Radial wave from center (u=0, v=0.5)
                let du = u * 1.5;
                let dv = (v - 0.5) * 2.0;
                let dist = (du * du + dv * dv).sqrt();

                let ripple = (dist * 7.0 - self.phase * 2.0).sin();
                let beat_pulse = ((dist * 8.0 - (1.0 - self.drop_wave) * 12.0).abs() < 1.5) as i32 as f32 * self.drop_wave;

                // Frequency band modulation along U
                let band_idx = (ui * 16 / grid_n).min(15);
                let band_boost = if audio_active { context.audio.bands[band_idx] * 0.6 } else { 0.0 };

                let elevation = (ripple * 0.5 + 0.5) * max_height + beat_pulse * 4.0 + band_boost * 3.0 + mid * (dist * 12.0).cos() * 0.8;
                let el_norm = (elevation / (max_height + 4.0)).clamp(0.0, 1.0);

                // Perspective screen mapping
                let sx = (15.5 + u * (10.0 + v * 15.0)).round() as i32;
                let sy = (8.0 + v * 22.0 - elevation).round() as i32;

                let col = Self::wire_color(pal, self.color_cycle, el_norm, depth_norm);
                pt_row[ui] = (sx, sy, col);
            }
        }

        // Draw horizontal wireframe grid lines
        #[allow(clippy::needless_range_loop)]
        for vi in 0..grid_n {
            for ui in 0..(grid_n - 1) {
                let (x0, y0, c0) = pts[vi][ui];
                let (x1, y1, _) = pts[vi][ui + 1];
                Self::line(frame, x0, y0, x1, y1, c0);
            }
        }

        // Draw longitudinal wireframe grid lines
        #[allow(clippy::needless_range_loop)]
        for ui in 0..grid_n {
            for vi in 0..(grid_n - 1) {
                let (x0, y0, c0) = pts[vi][ui];
                let (x1, y1, _) = pts[vi + 1][ui];
                Self::line(frame, x0, y0, x1, y1, c0);
            }
        }

        // Distant horizon starry sparkle from treble
        if audio_active && treble > 0.4 {
            for x in (1..WIDTH).step_by(6) {
                #[allow(clippy::manual_is_multiple_of)]
                if (x + (self.phase * 8.0) as usize) % 7 == 0 {
                    frame.set(x, 2, Rgb::WHITE);
                }
            }
        }
    }
}
