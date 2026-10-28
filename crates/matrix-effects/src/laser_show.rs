use crate::util::{alloc_boxed_2d, audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PATTERNS: &[&str] = &["Fan Sweep", "Tunnel Cone", "Cross Scanners", "Liquid Sky"];
const PALETTES: &[&str] = &["Laser Emerald", "Cyber Rave", "Solar Ruby", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Scan speed",
        kind: ParameterKind::Number,
        min: 5.0,
        max: 100.0,
        step: 1.0,
        default: 35.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "beams",
        label: "Beam count",
        kind: ParameterKind::Number,
        min: 3.0,
        max: 12.0,
        step: 1.0,
        default: 7.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "pattern",
        label: "Laser pattern",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 3.0,
        step: 1.0,
        default: 0.0,
        options: PATTERNS,
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

pub struct LaserShowEffect {
    values: [f32; 5],
    scan_phase: f32,
    color_cycle: f32,
    strobe: f32,
    decay_buffer: Box<[[Rgb; WIDTH]; HEIGHT]>,
}

impl Default for LaserShowEffect {
    fn default() -> Self {
        Self {
            values: [35.0, 7.0, 0.0, 3.0, 1.0],
            scan_phase: 0.0,
            color_cycle: 0.0,
            strobe: 0.0,
            decay_buffer: alloc_boxed_2d(Rgb::BLACK),
        }
    }
}

impl LaserShowEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn laser_color(pal: usize, color_cycle: f32, intensity: f32, beam_idx: usize, total_beams: usize) -> Rgb {
        let b = clamp01(intensity);
        let b_norm = beam_idx as f32 / total_beams.max(1) as f32;

        match pal {
            0 => {
                // Laser Emerald (Bright 532nm DPSS green with white-hot core)
                let hot = b.powi(3);
                let r = to_byte(255.0 * hot * 0.7);
                let g = to_byte(255.0 * b);
                let blue = to_byte(255.0 * hot * 0.4);
                Rgb::new(r, g, blue)
            }
            1 => {
                // Cyber Rave (Neon Cyan & Electric Magenta alternate)
                #[allow(clippy::manual_is_multiple_of)]
                if beam_idx % 2 == 0 {
                    // Cyan
                    let r = to_byte(30.0 * b);
                    let g = to_byte(230.0 * b);
                    let blue = to_byte(255.0 * b);
                    Rgb::new(r, g, blue)
                } else {
                    // Magenta
                    let r = to_byte(255.0 * b);
                    let g = to_byte(20.0 * b);
                    let blue = to_byte(200.0 * b);
                    Rgb::new(r, g, blue)
                }
            }
            2 => {
                // Solar Ruby (Fiery 650nm Red & Amber Gold)
                let r = to_byte(255.0 * b);
                let g = to_byte(255.0 * b.powi(2) * 0.7);
                let blue = to_byte(40.0 * b.powi(4));
                Rgb::new(r, g, blue)
            }
            _ => {
                // Rainbow Fade
                let h = (color_cycle + b_norm * 0.6) % 1.0;
                hsv_to_rgb(h, 0.95, b)
            }
        }
    }

    fn draw_laser_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: Rgb) {
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

        for _ in 0..=steps {
            let xi = cx.round() as i32;
            let yi = cy.round() as i32;

            if xi >= 0 && xi < WIDTH as i32 && yi >= 0 && yi < HEIGHT as i32 {
                let px = &mut self.decay_buffer[yi as usize][xi as usize];
                px.r = px.r.saturating_add(color.r);
                px.g = px.g.saturating_add(color.g);
                px.b = px.b.saturating_add(color.b);
            }
            cx += x_inc;
            cy += y_inc;
        }
    }
}

impl Effect for LaserShowEffect {
    fn id(&self) -> &'static str {
        "laser_show"
    }

    fn name(&self) -> &'static str {
        "Club Laser Show"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "beams" => Some(self.values[1]),
            "pattern" => Some(self.values[2]),
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
            "beams" => self.values[1] = value,
            "pattern" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.scan_phase = 0.0;
        self.color_cycle = 0.0;
        self.strobe = 0.0;
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
            self.strobe = 1.0;
        } else {
            self.strobe = (self.strobe - dt * 4.0).max(0.0);
        }

        // Haze / smoke decay
        let decay_mult = 0.70f32.powf(dt * 30.0);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let px = &mut self.decay_buffer[y][x];
                px.r = (px.r as f32 * decay_mult) as u8;
                px.g = (px.g as f32 * decay_mult) as u8;
                px.b = (px.b as f32 * decay_mult) as u8;
            }
        }

        let speed = (self.values[0] / 30.0) * (1.0 + treble * 0.6);
        self.scan_phase = (self.scan_phase + dt * speed * 2.2) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.18) % 1.0;

        let pal = self.values[3] as usize;
        let beam_count = (self.values[1] as usize).clamp(3, 16);
        let pattern = self.values[2] as usize;

        let intensity = (0.75 + 0.25 * mid + self.strobe * 0.4).min(1.0);

        match pattern {
            0 => {
                // Fan Sweep from Top Center (15.5, 0.0)
                let origin_x = 15.5f32;
                let origin_y = 0.0f32;
                let sweep_center = (self.scan_phase).sin() * 8.0;
                let fan_width = (24.0 + bass * 12.0).min(31.0);

                for b in 0..beam_count {
                    let spread_t = (b as f32 / (beam_count - 1) as f32) - 0.5;
                    let target_x = 15.5 + sweep_center + spread_t * fan_width;
                    let target_y = 31.0f32;

                    let col = Self::laser_color(pal, self.color_cycle, intensity, b, beam_count);
                    self.draw_laser_line(origin_x, origin_y, target_x, target_y, col);
                }
            }
            1 => {
                // Tunnel Cone (Rotational 3D Cone from Center)
                let origin_x = 15.5f32;
                let origin_y = 15.5f32;
                let radius = 14.0 + bass * 4.0;

                for b in 0..beam_count {
                    let ang = self.scan_phase + (b as f32 / beam_count as f32) * 2.0 * PI;
                    let target_x = origin_x + ang.cos() * radius;
                    let target_y = origin_y + ang.sin() * radius;

                    let col = Self::laser_color(pal, self.color_cycle, intensity, b, beam_count);
                    self.draw_laser_line(origin_x, origin_y, target_x, target_y, col);
                }
            }
            2 => {
                // Cross Scanners (Dual Projectors in Corners)
                let p1_x = 0.0f32;
                let p1_y = 0.0f32;
                let p2_x = 31.0f32;
                let p2_y = 0.0f32;

                let half_beams = beam_count / 2;
                for b in 0..half_beams {
                    let swing = ((self.scan_phase * 1.5 + b as f32 * 0.4).sin() * 0.5 + 0.5) * 31.0;
                    let col1 = Self::laser_color(pal, self.color_cycle, intensity, b, half_beams);
                    self.draw_laser_line(p1_x, p1_y, swing, 31.0, col1);

                    let swing2 = ((self.scan_phase * 1.5 + b as f32 * 0.4 + PI).sin() * 0.5 + 0.5) * 31.0;
                    let col2 = Self::laser_color(pal, self.color_cycle, intensity, b + half_beams, half_beams);
                    self.draw_laser_line(p2_x, p2_y, swing2, 31.0, col2);
                }
            }
            _ => {
                // Liquid Sky (Horizontal Scanning Sheet Waves)
                let sheet_y = 14.0 + (self.scan_phase * 1.2).sin() * 8.0;
                let waviness = 2.0 + bass * 3.0;

                for x in 0..WIDTH {
                    let x_norm = x as f32 / WIDTH as f32;
                    let wave_y = sheet_y + (x_norm * 4.0 * PI + self.scan_phase * 3.0).sin() * waviness;
                    let yi = wave_y.round() as i32;
                    if yi >= 0 && yi < HEIGHT as i32 {
                        let col = Self::laser_color(pal, self.color_cycle, intensity, x / 4, 8);
                        let px = &mut self.decay_buffer[yi as usize][x];
                        px.r = px.r.saturating_add(col.r);
                        px.g = px.g.saturating_add(col.g);
                        px.b = px.b.saturating_add(col.b);
                    }
                }
            }
        }

        // Strobe flash overlay
        if self.strobe > 0.4 {
            let flash_byte = to_byte(self.strobe * 160.0);
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let cur = self.decay_buffer[y][x];
                    frame.set(
                        x,
                        y,
                        Rgb::new(
                            cur.r.saturating_add(flash_byte),
                            cur.g.saturating_add(flash_byte),
                            cur.b.saturating_add(flash_byte),
                        ),
                    );
                }
            }
        } else {
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    frame.set(x, y, self.decay_buffer[y][x]);
                }
            }
        }
    }
}
