use crate::util::{audio_bass, audio_mid, audio_treble, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const GEOMETRIES: &[&str] = &["Hexagon Tunnel", "Liquid Sky Fan", "Laser Star", "Rotating Triangle"];
const PALETTES: &[&str] = &["Emerald 532nm", "RGB Concert", "Red & Blue Laser", "Rainbow Galvo"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Scan speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 45.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "geometry",
        label: "Tunnel shape",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 3.0,
        step: 1.0,
        default: 0.0,
        options: GEOMETRIES,
        unit: "",
    },
    ParameterDescriptor {
        id: "rings",
        label: "Tunnel depth rings",
        kind: ParameterKind::Number,
        min: 2.0,
        max: 8.0,
        step: 1.0,
        default: 5.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Laser wavelength",
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

pub struct LaserScannerEffect {
    values: [f32; 5],
    z_scroll: f32,
    rotation: f32,
    strobe_pulse: f32,
    color_cycle: f32,
}

impl Default for LaserScannerEffect {
    fn default() -> Self {
        Self {
            values: [45.0, 0.0, 5.0, 1.0, 1.0],
            z_scroll: 0.0,
            rotation: 0.0,
            strobe_pulse: 0.0,
            color_cycle: 0.0,
        }
    }
}

impl LaserScannerEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn laser_color(pal: usize, depth_t: f32, color_cycle: f32) -> Rgb {
        match pal {
            0 => {
                // Emerald 532nm DPSS Laser: Laser Green with hot white core
                let g = to_byte(255.0 * (0.6 + 0.4 * depth_t));
                let r = to_byte(40.0 * depth_t);
                let b = to_byte(80.0 * depth_t);
                Rgb::new(r, g, b)
            }
            1 => {
                // RGB Concert: Alternates Red / Green / Blue / Yellow by depth
                let idx = ((depth_t * 3.99).floor() as usize) % 4;
                match idx {
                    0 => Rgb::new(255, 30, 40),
                    1 => Rgb::new(0, 255, 120),
                    2 => Rgb::new(30, 140, 255),
                    _ => Rgb::new(255, 230, 20),
                }
            }
            2 => {
                // Red & Blue Police Laser
                let is_red = depth_t > 0.5;
                if is_red {
                    Rgb::new(255, 15, 30)
                } else {
                    Rgb::new(20, 120, 255)
                }
            }
            _ => {
                // Rainbow Galvo
                let h = (color_cycle + depth_t * 0.7) % 1.0;
                hsv_to_rgb(h, 1.0, 1.0)
            }
        }
    }

    fn draw_laser_line(frame: &mut Frame, x0: f32, y0: f32, x1: f32, y1: f32, col: Rgb, hot_core: bool) {
        let dx = x1 - x0;
        let dy = y1 - y0;
        let steps = (dx.abs().max(dy.abs()) * 2.0).ceil() as usize;
        if steps == 0 {
            return;
        }

        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let px = (x0 + dx * t).round() as i32;
            let py = (y0 + dy * t).round() as i32;

            if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                let pixel_c = if hot_core && (s % 3 == 0) {
                    Rgb::new(
                        col.r.saturating_add(120),
                        col.g.saturating_add(120),
                        col.b.saturating_add(120),
                    )
                } else {
                    col
                };
                let cur = frame.get(px as usize, py as usize);
                frame.set(
                    px as usize,
                    py as usize,
                    Rgb::new(
                        cur.r.saturating_add(pixel_c.r),
                        cur.g.saturating_add(pixel_c.g),
                        cur.b.saturating_add(pixel_c.b),
                    ),
                );
            }
        }
    }
}

impl Effect for LaserScannerEffect {
    fn id(&self) -> &'static str {
        "laser_scanner"
    }

    fn name(&self) -> &'static str {
        "Laser Tunnel Scanner"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "geometry" => Some(self.values[1]),
            "rings" => Some(self.values[2]),
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
            "geometry" => self.values[1] = value,
            "rings" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.z_scroll = 0.0;
        self.rotation = 0.0;
        self.strobe_pulse = 0.0;
        self.color_cycle = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::BLACK); // High-contrast pitch black laser hall

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

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.8 + self.strobe_pulse * 0.6);
        self.z_scroll = (self.z_scroll + dt * speed * 2.8) % 1.0;
        self.rotation = (self.rotation + dt * speed * 2.2) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.16) % 1.0;

        let pal = self.values[3] as usize;
        let geom = self.values[1] as usize;
        let num_rings = self.values[2].round() as usize;

        let cx = 15.5f32;
        let cy = 15.5f32;

        // Render laser tunnel rings from far (depth ~ 0.1) to near (depth ~ 1.0)
        for r_idx in 0..num_rings {
            let ring_progress = ((r_idx as f32 / num_rings as f32) + self.z_scroll) % 1.0;
            // Perspective exponential expansion
            let scale = ring_progress.powf(2.0) * (15.5 + bass * 6.0);
            if scale < 1.0 {
                continue;
            }

            let col = Self::laser_color(pal, ring_progress, self.color_cycle);
            let rot = self.rotation + ring_progress * 1.5;

            match geom {
                0 => {
                    // Hexagon Tunnel
                    let sides = 6;
                    for s in 0..sides {
                        let a0 = rot + (s as f32) * (2.0 * PI / sides as f32);
                        let a1 = rot + ((s + 1) as f32) * (2.0 * PI / sides as f32);

                        let x0 = cx + a0.cos() * scale;
                        let y0 = cy + a0.sin() * scale;
                        let x1 = cx + a1.cos() * scale;
                        let y1 = cy + a1.sin() * scale;

                        Self::draw_laser_line(frame, x0, y0, x1, y1, col, true);
                    }
                }
                1 => {
                    // Liquid Sky Fan: Oscillating horizontal undulating laser sheet
                    let wave_amp = 4.0 + treble * 6.0;
                    let fan_y = cy + (rot * 2.0 + ring_progress * 5.0).sin() * wave_amp;

                    let mut prev_x = 0.0f32;
                    let mut prev_y = fan_y;

                    for step in 1..=8 {
                        let cur_x = (step as f32 / 8.0) * 31.0;
                        let cur_y = fan_y + ((cur_x * 0.4 + rot * 3.0).sin()) * (2.0 + mid * 3.0);
                        Self::draw_laser_line(frame, prev_x, prev_y, cur_x, cur_y, col, false);
                        prev_x = cur_x;
                        prev_y = cur_y;
                    }
                }
                2 => {
                    // Laser Star (5-pointed geometric laser star)
                    let points = 5;
                    for p in 0..(points * 2) {
                        let rad0 = if p % 2 == 0 { scale } else { scale * 0.45 };
                        let rad1 = if (p + 1) % 2 == 0 { scale } else { scale * 0.45 };

                        let a0 = rot + (p as f32) * (PI / points as f32);
                        let a1 = rot + ((p + 1) as f32) * (PI / points as f32);

                        let x0 = cx + a0.cos() * rad0;
                        let y0 = cy + a0.sin() * rad0;
                        let x1 = cx + a1.cos() * rad1;
                        let y1 = cy + a1.sin() * rad1;

                        Self::draw_laser_line(frame, x0, y0, x1, y1, col, true);
                    }
                }
                _ => {
                    // Rotating Triangle
                    let sides = 3;
                    for s in 0..sides {
                        let a0 = rot + (s as f32) * (2.0 * PI / 3.0);
                        let a1 = rot + ((s + 1) as f32) * (2.0 * PI / 3.0);

                        let x0 = cx + a0.cos() * scale;
                        let y0 = cy + a0.sin() * scale;
                        let x1 = cx + a1.cos() * scale;
                        let y1 = cy + a1.sin() * scale;

                        Self::draw_laser_line(frame, x0, y0, x1, y1, col, true);
                    }
                }
            }
        }

        // Central laser aperture dot
        frame.set(15, 15, Rgb::WHITE);
        frame.set(16, 15, Rgb::WHITE);
        frame.set(15, 16, Rgb::WHITE);
        frame.set(16, 16, Rgb::WHITE);

        // Beat blast flash
        if self.strobe_pulse > 0.2 {
            let blast = to_byte(self.strobe_pulse * 100.0);
            for x in (0..WIDTH).step_by(4) {
                frame.set(x, 0, Rgb::new(blast, blast, blast));
                frame.set(x, 31, Rgb::new(blast, blast, blast));
            }
        }
    }
}
