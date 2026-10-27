use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Area 51", "Cyber Roswell", "Golden Eclipse", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "Flight speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 35.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "beam_width",
        label: "Tractor beam size",
        kind: ParameterKind::Number,
        min: 4.0,
        max: 12.0,
        step: 1.0,
        default: 8.0,
        options: &[],
        unit: "px",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Theme",
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

pub struct UfoAbductionEffect {
    values: [f32; 4],
    ufo_time: f32,
    abduction_y: f32,
    beam_pulse: f32,
    color_cycle: f32,
}

impl Default for UfoAbductionEffect {
    fn default() -> Self {
        Self {
            values: [35.0, 8.0, 0.0, 1.0],
            ufo_time: 0.0,
            abduction_y: 26.0,
            beam_pulse: 0.0,
            color_cycle: 0.0,
        }
    }
}

impl UfoAbductionEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn beam_color(pal: usize, brightness: f32, color_cycle: f32) -> Rgb {
        let b = clamp01(brightness);
        match pal {
            0 => {
                // Area 51 (Glowing radioactive lime/cyan beam)
                let r = to_byte(30.0 * b);
                let g = to_byte(255.0 * b);
                let blue = to_byte(120.0 * b);
                Rgb::new(r, g, blue)
            }
            1 => {
                // Cyber Roswell (Neon Magenta beam)
                let r = to_byte(255.0 * b);
                let g = to_byte(20.0 * b);
                let blue = to_byte(180.0 * b);
                Rgb::new(r, g, blue)
            }
            2 => {
                // Golden Eclipse (Warm Amber beam)
                let r = to_byte(255.0 * b);
                let g = to_byte(210.0 * b);
                let blue = to_byte(40.0 * b);
                Rgb::new(r, g, blue)
            }
            _ => {
                // Rainbow Beam
                let h = (color_cycle + b * 0.4) % 1.0;
                hsv_to_rgb(h, 0.9, b)
            }
        }
    }

    fn draw_ufo(frame: &mut Frame, cx: i32, cy: i32, light_phase: usize, ufo_c: Rgb) {
        // Saucer Dome (Width 5, Height 2 at cx - 2, cy - 2)
        let dome_c = Rgb::new(100, 220, 255); // Glowing cockpit glass
        for dx in 0..5i32 {
            let px = cx - 2 + dx;
            if px >= 0 && px < WIDTH as i32 {
                if (1..=3).contains(&dx) && (cy - 2) >= 0 {
                    frame.set(px as usize, (cy - 2) as usize, dome_c);
                }
                if (cy - 1) >= 0 {
                    frame.set(px as usize, (cy - 1) as usize, dome_c);
                }
            }
        }

        // Saucer Main Hull Disk (Width 11, Height 2 at cx - 5, cy)
        for dx in 0..11i32 {
            let px = cx - 5 + dx;
            if px >= 0 && px < WIDTH as i32 {
                if cy >= 0 && cy < HEIGHT as i32 {
                    frame.set(px as usize, cy as usize, ufo_c);
                }
                if (cy + 1) >= 0 && (cy + 1) < HEIGHT as i32 && (1..=9).contains(&dx) {
                    frame.set(px as usize, (cy + 1) as usize, ufo_c);
                }
            }
        }

        // Running Navigation Lights under Hull (at cy + 1, dx = 2, 4, 6, 8)
        let light_coords = [cx - 3, cx - 1, cx + 1, cx + 3];
        for (i, &lx) in light_coords.iter().enumerate() {
            if lx >= 0 && lx < WIDTH as i32 && (cy + 1) >= 0 && (cy + 1) < HEIGHT as i32 {
                #[allow(clippy::manual_is_multiple_of)]
                let is_on = (i + light_phase) % 2 == 0;
                let c = if is_on { Rgb::new(255, 230, 0) } else { Rgb::new(255, 40, 20) };
                frame.set(lx as usize, (cy + 1) as usize, c);
            }
        }
    }

    fn draw_cow(frame: &mut Frame, cx: i32, cy: i32) {
        // 4x3 pixel art cow
        let c_white = Rgb::WHITE;
        let c_black = Rgb::new(20, 20, 20);
        let c_pink = Rgb::new(255, 160, 180);

        // Head (right side)
        if cx + 2 >= 0 && cx + 2 < WIDTH as i32 && cy >= 0 && cy < HEIGHT as i32 {
            frame.set((cx + 2) as usize, cy as usize, c_white); // Head
            frame.set((cx + 2) as usize, (cy + 1) as usize, c_pink); // Muzzle
        }

        // Body (3x2 at cx - 1 ..= cx + 1)
        for dx in -1..=1i32 {
            let px = cx + dx;
            if px >= 0 && px < WIDTH as i32 {
                if cy >= 0 && cy < HEIGHT as i32 {
                    let is_spot = dx == 0;
                    frame.set(px as usize, cy as usize, if is_spot { c_black } else { c_white });
                }
                if (cy + 1) >= 0 && (cy + 1) < HEIGHT as i32 {
                    frame.set(px as usize, (cy + 1) as usize, c_white);
                }
            }
        }

        // Legs (y = cy + 2)
        if (cy + 2) >= 0 && (cy + 2) < HEIGHT as i32 {
            if cx > 0 && cx - 1 < WIDTH as i32 {
                frame.set((cx - 1) as usize, (cy + 2) as usize, c_black);
            }
            if cx + 1 >= 0 && cx + 1 < WIDTH as i32 {
                frame.set((cx + 1) as usize, (cy + 2) as usize, c_black);
            }
        }
    }
}

impl Effect for UfoAbductionEffect {
    fn id(&self) -> &'static str {
        "ufo_abduction"
    }

    fn name(&self) -> &'static str {
        "Retro UFO Abduction"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "beam_width" => Some(self.values[1]),
            "palette" => Some(self.values[2]),
            "audio_reactive" => Some(self.values[3]),
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
            "palette" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.ufo_time = 0.0;
        self.abduction_y = 26.0;
        self.beam_pulse = 0.0;
        self.color_cycle = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::new(4, 6, 20)); // Midnight sky

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[3] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.beam_pulse = 1.0;
        } else {
            self.beam_pulse = (self.beam_pulse - dt * 3.0).max(0.0);
        }

        let speed = (self.values[0] / 35.0) * (1.0 + bass * 0.7);
        self.ufo_time = (self.ufo_time + dt * speed) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let pal = self.values[2] as usize;
        let base_beam_w = self.values[1] + bass * 3.0;

        // 1. Starfield background
        for s in 0..12 {
            let sx = (s * 7 + 3) % WIDTH;
            let sy = (s * 5 + 1) % 16;
            let twinkle = 0.4 + 0.6 * ((s as f32 * 1.5 + self.ufo_time * 3.0).sin() * 0.5 + 0.5);
            let b = to_byte(255.0 * twinkle);
            frame.set(sx, sy, Rgb::new(b, b, b));
        }

        // 2. Rolling Green Hills at Bottom (y: 27..=31)
        for x in 0..WIDTH {
            let hill1 = 28.0 + (x as f32 * 0.25).sin() * 1.5;
            let h1 = hill1.round() as usize;
            for y in h1..HEIGHT {
                let hill_col = if pal == 1 {
                    Rgb::new(10, 30, 60)
                } else {
                    Rgb::new(15, 75, 25)
                };
                frame.set(x, y, hill_col);
            }
        }

        // UFO Coordinates
        let ufo_cx = (15.5 + (self.ufo_time).sin() * 7.0).round() as i32;
        let ufo_cy = (7.0 + (self.ufo_time * 2.0).cos() * 1.5).round() as i32;

        // 3. Conical Tractor Beam projecting from UFO base to ground
        let beam_top_y = ufo_cy + 2;
        let beam_bot_y = 29i32;

        for y in beam_top_y..=beam_bot_y {
            let progress = (y - beam_top_y) as f32 / (beam_bot_y - beam_top_y) as f32; // 0=ufo, 1=ground
            let current_half_w = 1.0 + progress * base_beam_w;
            let hw = current_half_w.round() as i32;

            for dx in -hw..=hw {
                let px = ufo_cx + dx;
                if px >= 0 && px < WIDTH as i32 {
                    let d_norm = (dx as f32).abs() / current_half_w.max(1.0);
                    let edge_falloff = (1.0 - d_norm.powi(2)).max(0.0);
                    let pulse_wave = ((progress * 10.0 - self.ufo_time * 8.0).sin() * 0.5 + 0.5) * 0.35;

                    let brightness = (edge_falloff * (0.45 + pulse_wave) + self.beam_pulse * 0.45 + mid * 0.15).min(1.0);
                    if brightness > 0.05 {
                        let col = Self::beam_color(pal, brightness, self.color_cycle);
                        let cur = frame.get(px as usize, y as usize);
                        frame.set(
                            px as usize,
                            y as usize,
                            Rgb::new(
                                cur.r.saturating_add(col.r),
                                cur.g.saturating_add(col.g),
                                cur.b.saturating_add(col.b),
                            ),
                        );
                    }
                }
            }
        }

        // 4. Abduction Animation (Cow floating up towards UFO)
        // Levitate up when beam is active
        if self.beam_pulse > 0.2 || bass > 0.4 {
            self.abduction_y = (self.abduction_y - dt * 20.0).max((beam_top_y + 3) as f32);
        } else {
            self.abduction_y = (self.abduction_y + dt * 8.0).min(26.0);
        }

        let cow_x = ufo_cx;
        let cow_y = self.abduction_y.round() as i32;
        Self::draw_cow(frame, cow_x, cow_y);

        // 5. Render Saucer Hull
        let ufo_hull_c = if pal == 1 {
            Rgb::new(0, 220, 240)
        } else if pal == 2 {
            Rgb::new(45, 45, 50)
        } else {
            Rgb::new(170, 175, 190) // Silver saucer
        };

        let light_phase = (self.ufo_time * 6.0).floor() as usize;
        Self::draw_ufo(frame, ufo_cx, ufo_cy, light_phase, ufo_hull_c);

        // Sparkle beam particles from treble
        if audio_active && treble > 0.4 {
            for i in 0..3 {
                let py = (beam_top_y + 3 + i * 5) as usize;
                let px = (ufo_cx + if i % 2 == 0 { -2 } else { 2 }) as usize;
                if px < WIDTH && py < HEIGHT {
                    frame.set(px, py, Rgb::WHITE);
                }
            }
        }
    }
}
