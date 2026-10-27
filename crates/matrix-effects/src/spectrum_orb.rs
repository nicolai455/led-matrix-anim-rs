use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Quantum Cyan", "Solar Flare", "Neon Dream", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Orbit speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 35.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "core_size",
        label: "Core radius",
        kind: ParameterKind::Number,
        min: 3.0,
        max: 8.0,
        step: 1.0,
        default: 5.0,
        options: &[],
        unit: "px",
    },
    ParameterDescriptor {
        id: "spike_length",
        label: "Spike excursion",
        kind: ParameterKind::Number,
        min: 2.0,
        max: 12.0,
        step: 1.0,
        default: 7.0,
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

const NUM_SPIKES: usize = 16;

#[derive(Clone, Copy, Debug)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

pub struct SpectrumOrbEffect {
    values: [f32; 5],
    smooth_bands: [f32; NUM_SPIKES],
    spike_dirs: [Vec3; NUM_SPIKES],
    rot_yaw: f32,
    rot_pitch: f32,
    color_cycle: f32,
    core_flare: f32,
}

impl Default for SpectrumOrbEffect {
    fn default() -> Self {
        // Uniformly distribute 16 spikes on a sphere surface (Fibonacci sphere)
        let mut dirs = [Vec3 { x: 0.0, y: 0.0, z: 1.0 }; NUM_SPIKES];
        let phi = PI * (3.0 - (5.0f32).sqrt()); // golden angle

        for (i, dir) in dirs.iter_mut().enumerate() {
            let y = 1.0 - (i as f32 / (NUM_SPIKES - 1) as f32) * 2.0; // from 1 to -1
            let radius = (1.0 - y * y).max(0.0).sqrt();
            let theta = phi * i as f32;
            let x = theta.cos() * radius;
            let z = theta.sin() * radius;
            *dir = Vec3 { x, y, z };
        }

        Self {
            values: [35.0, 5.0, 7.0, 3.0, 1.0],
            smooth_bands: [0.0; NUM_SPIKES],
            spike_dirs: dirs,
            rot_yaw: 0.0,
            rot_pitch: 0.0,
            color_cycle: 0.0,
            core_flare: 0.0,
        }
    }
}

impl SpectrumOrbEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn spike_color(pal: usize, color_cycle: f32, band_idx: usize, z_depth: f32, tip_t: f32) -> Rgb {
        let b = clamp01(0.35 + 0.65 * ((z_depth + 1.0) * 0.5));
        let norm_idx = band_idx as f32 / NUM_SPIKES as f32;

        match pal {
            0 => {
                // Quantum Cyan (Dark Indigo base -> Electric Cyan -> Pure White tip)
                if tip_t > 0.85 {
                    Rgb::new(to_byte(220.0 * b), 255, 255)
                } else {
                    let r = to_byte(20.0 * b);
                    let g = to_byte((140.0 + 115.0 * tip_t) * b);
                    let blue = to_byte(255.0 * b);
                    Rgb::new(r, g, blue)
                }
            }
            1 => {
                // Solar Flare (Crimson base -> Orange -> White-Hot tip)
                if tip_t > 0.85 {
                    Rgb::new(255, 255, to_byte(180.0 * b))
                } else {
                    let r = to_byte(255.0 * b);
                    let g = to_byte(220.0 * tip_t.powi(2) * b);
                    let blue = to_byte(15.0 * b);
                    Rgb::new(r, g, blue)
                }
            }
            2 => {
                // Neon Dream (Laser Pink -> Bright Magenta -> Neon Violet)
                let r = to_byte((255.0 - 50.0 * tip_t) * b);
                let g = to_byte(30.0 * b);
                let blue = to_byte((160.0 + 95.0 * tip_t) * b);
                Rgb::new(r, g, blue)
            }
            _ => {
                // Rainbow Spectrum
                let h = (color_cycle + norm_idx * 0.7 + tip_t * 0.3) % 1.0;
                hsv_to_rgb(h, 0.95, b)
            }
        }
    }

    fn draw_spike_line(frame: &mut Frame, x0: f32, y0: f32, x1: f32, y1: f32, col0: Rgb, col1: Rgb) {
        let dx = x1 - x0;
        let dy = y1 - y0;
        let steps = (dx.abs().max(dy.abs()) * 1.5).ceil() as usize;
        if steps == 0 {
            return;
        }

        let x_inc = dx / steps as f32;
        let y_inc = dy / steps as f32;

        let mut cx = x0;
        let mut cy = y0;

        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let xi = cx.round() as i32;
            let yi = cy.round() as i32;

            if xi >= 0 && xi < WIDTH as i32 && yi >= 0 && yi < HEIGHT as i32 {
                let r = to_byte(col0.r as f32 * (1.0 - t) + col1.r as f32 * t);
                let g = to_byte(col0.g as f32 * (1.0 - t) + col1.g as f32 * t);
                let b = to_byte(col0.b as f32 * (1.0 - t) + col1.b as f32 * t);

                let cur = frame.get(xi as usize, yi as usize);
                frame.set(
                    xi as usize,
                    yi as usize,
                    Rgb::new(
                        cur.r.saturating_add(r),
                        cur.g.saturating_add(g),
                        cur.b.saturating_add(b),
                    ),
                );
            }
            cx += x_inc;
            cy += y_inc;
        }
    }
}

impl Effect for SpectrumOrbEffect {
    fn id(&self) -> &'static str {
        "spectrum_orb"
    }

    fn name(&self) -> &'static str {
        "Spectrum Orb"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "core_size" => Some(self.values[1]),
            "spike_length" => Some(self.values[2]),
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
            "core_size" => self.values[1] = value,
            "spike_length" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.smooth_bands = [0.0; NUM_SPIKES];
        self.rot_yaw = 0.0;
        self.rot_pitch = 0.0;
        self.color_cycle = 0.0;
        self.core_flare = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::BLACK);

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[4] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let _mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.core_flare = 1.0;
        } else {
            self.core_flare = (self.core_flare - dt * 2.8).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.7);
        self.rot_yaw = (self.rot_yaw + dt * speed * 1.5) % (2.0 * PI);
        self.rot_pitch = (self.rot_pitch + dt * speed * 1.1) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.14) % 1.0;

        let pal = self.values[3] as usize;
        let core_r = self.values[1] * (1.0 + bass * 0.4 + self.core_flare * 0.3);
        let spike_max = self.values[2];

        let center_x = 15.5f32;
        let center_y = 15.5f32;

        let cyaw = self.rot_yaw.cos();
        let syaw = self.rot_yaw.sin();
        let cpitch = self.rot_pitch.cos();
        let spitch = self.rot_pitch.sin();

        // 1. Render glowing solid core sphere at center
        let core_r_sq = core_r * core_r;
        for y in (center_y - core_r).floor().max(0.0) as usize..=(center_y + core_r).ceil().min(HEIGHT as f32 - 1.0) as usize {
            let dy = y as f32 - center_y;
            for x in (center_x - core_r).floor().max(0.0) as usize..=(center_x + core_r).ceil().min(WIDTH as f32 - 1.0) as usize {
                let dx = x as f32 - center_x;
                let d2 = dx * dx + dy * dy;
                if d2 <= core_r_sq {
                    let dz = (core_r_sq - d2).sqrt() / core_r;
                    let diffuse = dz.powf(0.8);
                    let mut col = Self::spike_color(pal, self.color_cycle, 0, dz, 0.0);
                    col.r = to_byte(col.r as f32 * diffuse);
                    col.g = to_byte(col.g as f32 * diffuse);
                    col.b = to_byte(col.b as f32 * diffuse);
                    if self.core_flare > 0.3 {
                        let f_b = to_byte(self.core_flare * 150.0);
                        col.r = col.r.saturating_add(f_b);
                        col.g = col.g.saturating_add(f_b);
                        col.b = col.b.saturating_add(f_b);
                    }
                    frame.set(x, y, col);
                }
            }
        }

        // 2. Rotate and Project 16 Frequency Spike Ribs
        for i in 0..NUM_SPIKES {
            let target_band = if audio_active {
                context.audio.bands[i]
            } else {
                let synth = ((self.rot_yaw * 2.0 + i as f32 * 0.4).sin() * 0.5 + 0.5).powf(1.5);
                synth * 0.75
            };

            // Smooth band motion
            self.smooth_bands[i] += (target_band - self.smooth_bands[i]) * (dt * 20.0).min(1.0);
            let band_val = self.smooth_bands[i];
            let spike_len = core_r + band_val * spike_max;

            let dir = self.spike_dirs[i];

            // 3D rotation (Yaw around Y, then Pitch around X)
            let x1 = dir.x * cyaw + dir.z * syaw;
            let z1 = -dir.x * syaw + dir.z * cyaw;
            let y1 = dir.y * cpitch - z1 * spitch;
            let z2 = dir.y * spitch + z1 * cpitch;

            // Base point on sphere surface
            let bx = center_x + x1 * core_r;
            let by = center_y + y1 * core_r;

            // Tip point protruding outward
            let tx = center_x + x1 * spike_len;
            let ty = center_y + y1 * spike_len;

            let c_base = Self::spike_color(pal, self.color_cycle, i, z2, 0.2);
            let c_tip = Self::spike_color(pal, self.color_cycle, i, z2, 1.0);

            Self::draw_spike_line(frame, bx, by, tx, ty, c_base, c_tip);

            // Glowing tip spark
            let ti_x = tx.round() as i32;
            let ti_y = ty.round() as i32;
            if ti_x >= 0 && ti_x < WIDTH as i32 && ti_y >= 0 && ti_y < HEIGHT as i32 {
                let spark_c = if band_val > 0.75 { Rgb::WHITE } else { c_tip };
                frame.set(ti_x as usize, ti_y as usize, spark_c);
            }
        }

        // Treble perimeter sparkle aura
        if audio_active && treble > 0.4 {
            for s in 0..3 {
                let ang = self.rot_yaw * 3.0 + s as f32 * 2.1;
                let sx = (center_x + ang.cos() * (core_r + spike_max * 0.8)).round() as i32;
                let sy = (center_y + ang.sin() * (core_r + spike_max * 0.8)).round() as i32;
                if sx >= 0 && sx < WIDTH as i32 && sy >= 0 && sy < HEIGHT as i32 {
                    frame.set(sx as usize, sy as usize, Rgb::WHITE);
                }
            }
        }
    }
}
