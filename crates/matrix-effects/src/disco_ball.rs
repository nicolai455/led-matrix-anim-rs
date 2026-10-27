use crate::util::{alloc_boxed_2d, audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Silver & Gold", "Studio 54", "Laser Club", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Rotation speed",
        kind: ParameterKind::Number,
        min: 5.0,
        max: 100.0,
        step: 1.0,
        default: 35.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "radius",
        label: "Ball size",
        kind: ParameterKind::Number,
        min: 4.0,
        max: 12.0,
        step: 1.0,
        default: 8.0,
        options: &[],
        unit: "px",
    },
    ParameterDescriptor {
        id: "spots",
        label: "Room reflections",
        kind: ParameterKind::Number,
        min: 8.0,
        max: 32.0,
        step: 1.0,
        default: 16.0,
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

pub struct DiscoBallEffect {
    values: [f32; 5],
    yaw: f32,
    color_cycle: f32,
    flare: f32,
    decay_buffer: Box<[[Rgb; WIDTH]; HEIGHT]>,
}

impl Default for DiscoBallEffect {
    fn default() -> Self {
        Self {
            values: [35.0, 8.0, 16.0, 3.0, 1.0],
            yaw: 0.0,
            color_cycle: 0.0,
            flare: 0.0,
            decay_buffer: alloc_boxed_2d(Rgb::BLACK),
        }
    }
}

impl DiscoBallEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn facet_color(pal: usize, color_cycle: f32, glint: f32, facet_norm: f32) -> Rgb {
        let g = clamp01(glint);
        match pal {
            0 => {
                // Silver & Gold
                let r = to_byte(255.0 * (0.6 * facet_norm + 0.4 * g));
                let gr = to_byte(255.0 * (0.5 * facet_norm + 0.35 * g));
                let b = to_byte(255.0 * (0.3 * facet_norm + 0.6 * g));
                Rgb::new(r, gr, b)
            }
            1 => {
                // Studio 54 (Magenta / Violet / Cyan)
                let r = to_byte(255.0 * (0.8 * g + 0.2 * facet_norm));
                let gr = to_byte(255.0 * (0.1 * facet_norm + 0.4 * g));
                let b = to_byte(255.0 * (0.9 * g + 0.4 * facet_norm));
                Rgb::new(r, gr, b)
            }
            2 => {
                // Laser Club (Emerald & Optical White)
                let r = to_byte(255.0 * (0.2 * facet_norm + 0.7 * g.powi(2)));
                let gr = to_byte(255.0 * (0.9 * g + 0.3 * facet_norm));
                let b = to_byte(255.0 * (0.5 * g + 0.2 * facet_norm));
                Rgb::new(r, gr, b)
            }
            _ => {
                // Rainbow Fade
                let h = (color_cycle + facet_norm * 0.5 + g * 0.3) % 1.0;
                hsv_to_rgb(h, 0.9, (0.3 + 0.7 * g).min(1.0))
            }
        }
    }
}

impl Effect for DiscoBallEffect {
    fn id(&self) -> &'static str {
        "disco_ball"
    }

    fn name(&self) -> &'static str {
        "Disco Mirror Ball"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "radius" => Some(self.values[1]),
            "spots" => Some(self.values[2]),
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
            "radius" => self.values[1] = value,
            "spots" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.yaw = 0.0;
        self.color_cycle = 0.0;
        self.flare = 0.0;
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
            self.flare = 1.0;
        } else {
            self.flare = (self.flare - dt * 2.8).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.7);
        self.yaw = (self.yaw + dt * speed * 1.6) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        // Fade background reflection spots
        let decay = 0.82f32.powf(dt * 30.0);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let px = &mut self.decay_buffer[y][x];
                px.r = (px.r as f32 * decay) as u8;
                px.g = (px.g as f32 * decay) as u8;
                px.b = (px.b as f32 * decay) as u8;
            }
        }

        let pal = self.values[3] as usize;
        let spot_count = self.values[2] as usize;

        // Cast rotating reflection beams across the room
        let center_x = 15.5f32;
        let center_y = 15.5f32;

        for i in 0..spot_count {
            let offset_ang = (i as f32 / spot_count as f32) * 2.0 * PI;
            let spot_yaw = self.yaw * (if i % 2 == 0 { 1.5 + mid * 0.4 } else { -1.2 - mid * 0.4 }) + offset_ang;
            let pitch = (i as f32 * 1.3 + self.yaw * 0.5).sin() * 0.7;

            let spot_dist = 11.0 + 8.0 * (spot_yaw * 2.0).cos();
            let sx = center_x + spot_dist * spot_yaw.cos() * pitch.cos();
            let sy = center_y + spot_dist * spot_yaw.sin();

            let xi = sx.round() as i32;
            let yi = sy.round() as i32;

            if xi >= 0 && xi < WIDTH as i32 && yi >= 0 && yi < HEIGHT as i32 {
                let glint = 0.7 + 0.3 * (spot_yaw * 4.0).sin();
                let col = Self::facet_color(pal, self.color_cycle, glint + self.flare * 0.5, i as f32 / spot_count as f32);
                let current = &mut self.decay_buffer[yi as usize][xi as usize];
                current.r = current.r.saturating_add(col.r);
                current.g = current.g.saturating_add(col.g);
                current.b = current.b.saturating_add(col.b);
            }
        }

        // Copy reflection trails to output
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                frame.set(x, y, self.decay_buffer[y][x]);
            }
        }

        // Draw 3D Mirror Ball at Center
        let ball_r = self.values[1];
        let r_sq = ball_r * ball_r;

        // Fixed light direction: from upper-left towards camera
        let lx = -0.577f32;
        let ly = -0.577f32;
        let lz = 0.577f32;

        let cy_start = (center_y - ball_r).floor().max(0.0) as usize;
        let cy_end = (center_y + ball_r).ceil().min(HEIGHT as f32 - 1.0) as usize;
        let cx_start = (center_x - ball_r).floor().max(0.0) as usize;
        let cx_end = (center_x + ball_r).ceil().min(WIDTH as f32 - 1.0) as usize;

        for y in cy_start..=cy_end {
            let dy = y as f32 - center_y;
            for x in cx_start..=cx_end {
                let dx = x as f32 - center_x;
                let d2 = dx * dx + dy * dy;
                if d2 <= r_sq {
                    let dz = (r_sq - d2).sqrt();
                    // Normal vector of the sphere surface
                    let nx = dx / ball_r;
                    let ny = dy / ball_r;
                    let nz = dz / ball_r;

                    // Facet discretization (creates tiled mirror grid on sphere)
                    let lon = ny.asin();
                    let lat = (nx / lon.cos().max(0.1)).clamp(-1.0, 1.0).asin() + self.yaw;

                    let facet_lat = (lat * 4.0).floor();
                    let facet_lon = (lon * 4.0).floor();
                    let facet_seed = (facet_lat * 17.0 + facet_lon * 31.0).sin();

                    // Specular glint calculation
                    let diffuse = (nx * lx + ny * ly + nz * lz).max(0.0);
                    let specular = diffuse.powi(6);
                    let facet_glint = (specular * 1.5 + (facet_seed + treble * 0.5).sin() * 0.4 + self.flare * 0.7).clamp(0.0, 1.0);

                    // Ball edge shading (darker near silhouette)
                    let rim = nz.powf(0.6);
                    let mut col = Self::facet_color(pal, self.color_cycle, facet_glint, (facet_lat * 0.1).abs() % 1.0);
                    col.r = (col.r as f32 * rim) as u8;
                    col.g = (col.g as f32 * rim) as u8;
                    col.b = (col.b as f32 * rim) as u8;

                    // Highlight center flare on beat
                    if self.flare > 0.3 && d2 < (ball_r * 0.4).powi(2) {
                        let f_byte = to_byte(self.flare * 200.0);
                        col.r = col.r.saturating_add(f_byte);
                        col.g = col.g.saturating_add(f_byte);
                        col.b = col.b.saturating_add(f_byte);
                    }

                    frame.set(x, y, col);
                }
            }
        }
    }
}
