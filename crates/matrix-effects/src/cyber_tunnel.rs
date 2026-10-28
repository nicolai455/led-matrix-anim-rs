use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Neon Tron", "Cyberpunk Violet", "Emerald Conduit", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Rush speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 40.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "twist",
        label: "Hex twist",
        kind: ParameterKind::Number,
        min: 0.0,
        max: 10.0,
        step: 1.0,
        default: 4.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "ribs",
        label: "Vertex ribs",
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

pub struct CyberTunnelEffect {
    values: [f32; 5],
    z_scroll: f32,
    color_cycle: f32,
    warp_boost: f32,
}

impl Default for CyberTunnelEffect {
    fn default() -> Self {
        Self {
            values: [40.0, 4.0, 1.0, 3.0, 1.0],
            z_scroll: 0.0,
            color_cycle: 0.0,
            warp_boost: 0.0,
        }
    }
}

impl CyberTunnelEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn tunnel_color(pal: usize, color_cycle: f32, brightness: f32, depth_norm: f32, is_rib: bool) -> Rgb {
        let b = clamp01(brightness);
        match pal {
            0 => {
                // Neon Tron (Laser Cyan rings, Blazing Orange ribs)
                if is_rib {
                    Rgb::new(255, to_byte(140.0 * b), 20)
                } else {
                    Rgb::new(to_byte(20.0 * b), to_byte(230.0 * b), to_byte(255.0 * b))
                }
            }
            1 => {
                // Cyberpunk Violet (Deep purple -> Hot pink rings -> White ribs)
                if is_rib {
                    Rgb::new(to_byte(255.0 * b), to_byte(220.0 * b), 255)
                } else {
                    Rgb::new(to_byte(255.0 * b), to_byte(20.0 * b), to_byte(180.0 * b))
                }
            }
            2 => {
                // Emerald Conduit (Forest green depth -> Neon lime ribs)
                if is_rib {
                    Rgb::new(to_byte(160.0 * b), 255, to_byte(50.0 * b))
                } else {
                    Rgb::new(0, to_byte(200.0 * b), to_byte(120.0 * b))
                }
            }
            _ => {
                // Rainbow Warp
                let h = (color_cycle + depth_norm * 0.7 + if is_rib { 0.3 } else { 0.0 }) % 1.0;
                hsv_to_rgb(h, 0.95, b)
            }
        }
    }
}

impl Effect for CyberTunnelEffect {
    fn id(&self) -> &'static str {
        "cyber_tunnel"
    }

    fn name(&self) -> &'static str {
        "Hex Cyber Tunnel"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "twist" => Some(self.values[1]),
            "ribs" => Some(self.values[2]),
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
            "ribs" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.z_scroll = 0.0;
        self.color_cycle = 0.0;
        self.warp_boost = 0.0;
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
            self.warp_boost = 1.0;
        } else {
            self.warp_boost = (self.warp_boost - dt * 3.5).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.9 + self.warp_boost * 1.5);
        self.z_scroll = (self.z_scroll + dt * speed * 4.5) % 1000.0;
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let pal = self.values[3] as usize;
        let twist = self.values[1] * 0.15;
        let show_ribs = self.values[2] > 0.5;

        let cx = 15.5f32;
        let cy = 15.5f32;
        const CONSTANT: f32 = 13.0;
        const EPSILON: f32 = 0.3;

        let two_pi_over_6 = 2.0 * PI / 6.0;

        for y in 0..HEIGHT {
            let dy = y as f32 - cy;
            for x in 0..WIDTH {
                let dx = x as f32 - cx;
                let r = (dx * dx + dy * dy).sqrt();
                let angle = dy.atan2(dx);

                // Inverse reciprocal depth coordinate z
                let depth = CONSTANT / (r + EPSILON);
                let z = depth + self.z_scroll;

                // Hexagonal symmetry coordinate:
                // Modulo angle by 60 deg (PI/3)
                let twisted_ang = angle + z * twist;
                let hex_sub_ang = (twisted_ang % two_pi_over_6) - (two_pi_over_6 * 0.5);

                // Hexagonal perimeter distance: r_hex = r * cos(sub_angle)
                let r_hex = r * (hex_sub_ang.cos());
                let hex_depth = CONSTANT / (r_hex + EPSILON);

                let depth_norm = (r / 16.0).min(1.0);

                // Concentric hexagonal rings
                let ring_phase = (hex_depth * 1.2 - self.z_scroll * 1.2) % 1.0;
                let in_ring = (ring_phase - 0.5).abs() < (0.12 + bass * 0.08);

                // Longitudinal vertex ribs (edges of the 6 faces)
                let is_rib = show_ribs && hex_sub_ang.abs() > 0.44;

                if in_ring || is_rib {
                    // Ring brightness modulated by depth & frequency bands
                    let band_idx = ((1.0 - depth_norm) * 15.0).round() as usize % 16;
                    let band_boost = if audio_active { context.audio.bands[band_idx] * 0.6 } else { 0.0 };

                    let brightness = (depth_norm.powf(0.8) * 0.8 + band_boost + self.warp_boost * 0.35 + mid * 0.15).min(1.0);

                    let col = Self::tunnel_color(pal, self.color_cycle, brightness, depth_norm, is_rib);
                    frame.set(x, y, col);
                }
            }
        }

        // Warp speed hyperdrive particles from treble
        if audio_active && treble > 0.4 {
            for i in 0..4 {
                let ang = (i as f32 * 0.5) * PI + self.z_scroll * 0.2;
                let pr = 4.0 + (i as f32 * 3.5);
                let px = (cx + ang.cos() * pr).round() as i32;
                let py = (cy + ang.sin() * pr).round() as i32;
                if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                    frame.set(px as usize, py as usize, Rgb::WHITE);
                }
            }
        }
    }
}
