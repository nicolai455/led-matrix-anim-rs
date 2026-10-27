use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Festival RGB", "Cyan & Gold", "Neon Violet & Lime", "Rainbow Prism"];
const GOBOS: &[&str] = &["Solid Beam", "Split Beams", "Strobe Gobo", "Pulsing Iris"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Pan & tilt speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 40.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "beam_width",
        label: "Cone angle",
        kind: ParameterKind::Number,
        min: 2.0,
        max: 10.0,
        step: 0.5,
        default: 5.0,
        options: &[],
        unit: "deg",
    },
    ParameterDescriptor {
        id: "gobo",
        label: "Gobo style",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 3.0,
        step: 1.0,
        default: 0.0,
        options: GOBOS,
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Color scheme",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 3.0,
        step: 1.0,
        default: 0.0,
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

pub struct MovingHeadsEffect {
    values: [f32; 5],
    timer: f32,
    strobe_pulse: f32,
    color_cycle: f32,
}

impl Default for MovingHeadsEffect {
    fn default() -> Self {
        Self {
            values: [40.0, 5.0, 0.0, 0.0, 1.0],
            timer: 0.0,
            strobe_pulse: 0.0,
            color_cycle: 0.0,
        }
    }
}

impl MovingHeadsEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn head_color(head_idx: usize, pal: usize, color_cycle: f32) -> Rgb {
        match pal {
            0 => {
                // Festival RGB (Red, Green, Blue, Amber)
                match head_idx % 4 {
                    0 => Rgb::new(255, 30, 40),
                    1 => Rgb::new(30, 240, 60),
                    2 => Rgb::new(20, 120, 255),
                    _ => Rgb::new(255, 180, 20),
                }
            }
            1 => {
                // Cyan & Gold
                #[allow(clippy::manual_is_multiple_of)]
                if head_idx % 2 == 0 {
                    Rgb::new(0, 240, 255)
                } else {
                    Rgb::new(255, 200, 30)
                }
            }
            2 => {
                // Neon Violet & Lime
                #[allow(clippy::manual_is_multiple_of)]
                if head_idx % 2 == 0 {
                    Rgb::new(200, 30, 255)
                } else {
                    Rgb::new(80, 255, 50)
                }
            }
            _ => {
                // Rainbow Prism
                let h = (color_cycle + (head_idx as f32) * 0.25) % 1.0;
                hsv_to_rgb(h, 0.95, 1.0)
            }
        }
    }
}

impl Effect for MovingHeadsEffect {
    fn id(&self) -> &'static str {
        "moving_heads"
    }

    fn name(&self) -> &'static str {
        "Moving Head Spotlights"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "beam_width" => Some(self.values[1]),
            "gobo" => Some(self.values[2]),
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
            "beam_width" => self.values[1] = value,
            "gobo" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.timer = 0.0;
        self.strobe_pulse = 0.0;
        self.color_cycle = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        // Concert stage hazing: deep dark atmospheric hall
        frame.clear(Rgb::new(8, 8, 14));

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[4] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.strobe_pulse = 1.0;
        } else {
            self.strobe_pulse = (self.strobe_pulse - dt * 4.5).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.8);
        self.timer = (self.timer + dt * speed * 2.0) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let pal = self.values[3] as usize;
        let gobo = self.values[2] as usize;
        let base_cone_w = self.values[1] + bass * 3.0;

        // 4 Ceiling Moving Head Mounts along y = 0
        let head_xs = [4.0f32, 11.5f32, 19.5f32, 27.0f32];

        // Draw Truss Rig along top row (y = 0)
        let truss_c = Rgb::new(60, 65, 75);
        for x in 0..WIDTH {
            frame.set(x, 0, truss_c);
        }

        // Each moving head projects down onto the floor with oscillating pan/tilt angles
        for (i, &hx) in head_xs.iter().enumerate() {
            let col = Self::head_color(i, pal, self.color_cycle);

            // Motorized pan angle sweeps left/right with harmonic offset
            let pan_phase = self.timer + (i as f32) * (PI * 0.5);
            let pan_amp = 0.55 + (if i % 2 == 0 { 0.15 } else { -0.15 });
            let beam_angle = (pan_phase.sin()) * pan_amp; // angle in radians from vertical down

            let h_int = if gobo == 2 && self.strobe_pulse < 0.2 {
                // Strobe shutter gobo: only flashes on strobe/beat
                0.2
            } else {
                1.0
            };

            // Volumetric light beam cone ray-march from (hx, 1) down to y = 31
            for y in 1..HEIGHT {
                let dy = y as f32 - 1.0;
                let beam_center_x = hx + dy * beam_angle.tan();
                let current_cone_half_w = 0.5 + (dy / 30.0) * base_cone_w;

                let min_x = (beam_center_x - current_cone_half_w).floor() as i32;
                let max_x = (beam_center_x + current_cone_half_w).ceil() as i32;

                for px in min_x..=max_x {
                    if px >= 0 && px < WIDTH as i32 {
                        let dist_from_center = (px as f32 - beam_center_x).abs();
                        if dist_from_center <= current_cone_half_w {
                            let rel_dist = dist_from_center / current_cone_half_w.max(0.1);
                            let falloff = (1.0 - rel_dist * rel_dist).max(0.0);

                            // Gobo modulation
                            let gobo_mult = match gobo {
                                1 => {
                                    // Split twin beams
                                    (rel_dist * 4.0).sin().abs()
                                }
                                3 => {
                                    // Pulsing iris
                                    0.5 + 0.5 * (self.timer * 6.0 - dy * 0.3).sin()
                                }
                                _ => 1.0,
                            };

                            // Beam haze intensity with distance and core highlight
                            let depth_factor = 0.25 + 0.75 * (1.0 - dy / 45.0);
                            let core_boost = if rel_dist < 0.25 { 0.4 } else { 0.0 };
                            let intensity = (falloff * depth_factor * gobo_mult * h_int + core_boost + self.strobe_pulse * 0.35)
                                .clamp(0.0, 1.0);

                            let cur = frame.get(px as usize, y);
                            let r_add = to_byte(col.r as f32 * intensity);
                            let g_add = to_byte(col.g as f32 * intensity);
                            let b_add = to_byte(col.b as f32 * intensity);

                            frame.set(
                                px as usize,
                                y,
                                Rgb::new(
                                    cur.r.saturating_add(r_add),
                                    cur.g.saturating_add(g_add),
                                    cur.b.saturating_add(b_add),
                                ),
                            );
                        }
                    }
                }
            }

            // Floor Hotspot Pool at y = 31
            let floor_center_x = (hx + 30.0 * beam_angle.tan()).round() as i32;
            let pool_rad = (base_cone_w * 0.8).round() as i32;
            for dx in -pool_rad..=pool_rad {
                let px = floor_center_x + dx;
                if px >= 0 && px < WIDTH as i32 {
                    let d = (dx as f32).abs() / pool_rad.max(1) as f32;
                    let bright = clamp01((1.0 - d * d) * (0.8 + bass * 0.6 + self.strobe_pulse * 0.6));
                    let cur = frame.get(px as usize, 31);
                    frame.set(
                        px as usize,
                        31,
                        Rgb::new(
                            cur.r.saturating_add(to_byte(col.r as f32 * bright)),
                            cur.g.saturating_add(to_byte(col.g as f32 * bright)),
                            cur.b.saturating_add(to_byte(col.b as f32 * bright)),
                        ),
                    );
                }
            }

            // Fixture Head Lens at (hx, 1)
            let fix_x = hx.round() as usize;
            if fix_x < WIDTH {
                frame.set(fix_x, 1, Rgb::WHITE);
                if fix_x > 0 { frame.set(fix_x - 1, 1, col); }
                if fix_x + 1 < WIDTH { frame.set(fix_x + 1, 1, col); }
            }
        }

        // Haze sparkle dust motes on treble / mid
        if audio_active && treble > 0.35 {
            for s in 0..4 {
                let sx = ((s * 9 + 4) as f32 + (self.timer * 5.0).sin() * 3.0).round() as usize % WIDTH;
                let sy = ((s * 7 + 10) as f32 + (self.timer * 4.0).cos() * 2.0).round() as usize % HEIGHT;
                let cur = frame.get(sx, sy);
                frame.set(
                    sx,
                    sy,
                    Rgb::new(
                        cur.r.saturating_add(180),
                        cur.g.saturating_add(220),
                        cur.b.saturating_add(255),
                    ),
                );
            }
        }
        if audio_active && mid > 0.4 {
            let cur = frame.get(15, 15);
            frame.set(15, 15, Rgb::new(cur.r.saturating_add(90), cur.g.saturating_add(90), cur.b.saturating_add(90)));
        }
    }
}
