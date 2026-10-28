use crate::util::{audio_bass, audio_mid, audio_treble, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Miami 1986", "Cyber Neon", "Golden Hour", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Drive speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 40.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "curve_motion",
        label: "Road winding",
        kind: ParameterKind::Number,
        min: 0.0,
        max: 10.0,
        step: 1.0,
        default: 5.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "exhaust_boost",
        label: "Nitro flames",
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

pub struct OutrunDriveEffect {
    values: [f32; 5],
    road_scroll: f32,
    curve_phase: f32,
    color_cycle: f32,
    nitro_boost: f32,
}

impl Default for OutrunDriveEffect {
    fn default() -> Self {
        Self {
            values: [40.0, 5.0, 1.0, 3.0, 1.0],
            road_scroll: 0.0,
            curve_phase: 0.0,
            color_cycle: 0.0,
            nitro_boost: 0.0,
        }
    }
}

impl OutrunDriveEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn synth_colors(pal: usize, color_cycle: f32) -> (Rgb, Rgb, Rgb) {
        // Returns (sun_c, curb_c, car_body_c)
        match pal {
            0 => {
                // Miami 1986 (Sunset Orange/Yellow, Hot Magenta curbs, Brilliant Red sports car)
                (Rgb::new(255, 170, 20), Rgb::new(255, 20, 160), Rgb::new(230, 25, 30))
            }
            1 => {
                // Cyber Neon (Cyan/White sun, Electric Violet curbs, Neon Cyan car)
                (Rgb::new(0, 240, 255), Rgb::new(190, 30, 255), Rgb::new(20, 200, 240))
            }
            2 => {
                // Golden Hour (Deep Gold sun, Amber curbs, Onyx Black car)
                (Rgb::new(255, 210, 40), Rgb::new(240, 120, 10), Rgb::new(30, 30, 35))
            }
            _ => {
                // Rainbow Fade
                let c1 = hsv_to_rgb(color_cycle, 0.95, 1.0);
                let c2 = hsv_to_rgb((color_cycle + 0.35) % 1.0, 0.9, 1.0);
                let c3 = hsv_to_rgb((color_cycle + 0.7) % 1.0, 0.9, 0.9);
                (c1, c2, c3)
            }
        }
    }
}

impl Effect for OutrunDriveEffect {
    fn id(&self) -> &'static str {
        "outrun_drive"
    }

    fn name(&self) -> &'static str {
        "Outrun Sunset Drive"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "curve_motion" => Some(self.values[1]),
            "exhaust_boost" => Some(self.values[2]),
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
            "curve_motion" => self.values[1] = value,
            "exhaust_boost" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.road_scroll = 0.0;
        self.curve_phase = 0.0;
        self.color_cycle = 0.0;
        self.nitro_boost = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::new(12, 5, 25)); // Deep twilight sky

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[4] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let _treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.nitro_boost = 1.0;
        } else {
            self.nitro_boost = (self.nitro_boost - dt * 3.5).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.8 + self.nitro_boost * 0.5);
        self.road_scroll = (self.road_scroll + dt * speed * 25.0) % 200.0;
        self.curve_phase = (self.curve_phase + dt * speed * 0.8) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.14) % 1.0;

        let pal = self.values[3] as usize;
        let (sun_col, curb_col, car_col) = Self::synth_colors(pal, self.color_cycle);

        // 1. Synthwave Sun at Horizon (cx=15.5, cy=13.0, r=6.5)
        let sun_cx = 15.5f32;
        let sun_cy = 13.0f32;
        let sun_r = 6.5f32;
        let sun_r_sq = sun_r * sun_r;

        for y in 0..15 {
            let yf = y as f32;
            let dy = yf - sun_cy;

            // Retro horizontal scanline cuts through the sun
            let is_blind = yf >= 10.0 && y % 2 == 1;

            for x in 0..WIDTH {
                let dx = x as f32 - sun_cx;
                let d2 = dx * dx + dy * dy;

                if d2 <= sun_r_sq && !is_blind {
                    let t = 1.0 - (d2 / sun_r_sq).sqrt();
                    let r = to_byte(sun_col.r as f32 * (0.8 + 0.2 * t));
                    let g = to_byte(sun_col.g as f32 * (0.8 + 0.2 * t));
                    let b = to_byte(sun_col.b as f32 * (0.8 + 0.2 * t));
                    frame.set(x, y, Rgb::new(r, g, b));
                }
            }
        }

        // Horizon at y = 14
        let horizon_y = 14i32;

        // 2. Perspective Road from y = 15 to 31
        let curve_amount = self.values[1] * (self.curve_phase).sin();

        for y in (horizon_y + 1)..HEIGHT as i32 {
            let y_norm = (y - horizon_y) as f32 / (HEIGHT as i32 - horizon_y) as f32; // 0=horizon, 1=foreground
            let road_half_w = 2.0 + y_norm.powi(2) * 14.0;

            let curve_offset = (1.0 - (1.0 - y_norm).powi(2)) * curve_amount;
            let road_center_x = 15.5 + curve_offset;

            // Perspective speed stripes (scrolling dashed lines)
            let z_depth = 8.0 / y_norm.max(0.05);
            let stripe_on = ((z_depth - self.road_scroll) * 0.4).sin() > 0.0;

            let left_curb = (road_center_x - road_half_w).round() as i32;
            let right_curb = (road_center_x + road_half_w).round() as i32;
            let center_line = road_center_x.round() as i32;

            for x in 0..WIDTH as i32 {
                if x < left_curb || x > right_curb {
                    // Out-of-bounds terrain: glowing perspective grid
                    let terrain_stripe = (x % 4 == 0) || stripe_on;
                    if terrain_stripe && y % 3 == 0 {
                        let t_c = to_byte(25.0 + mid * 35.0);
                        frame.set(x as usize, y as usize, Rgb::new(t_c / 2, 0, t_c));
                    }
                } else if x == left_curb || x == right_curb {
                    // Neon road curbs
                    frame.set(x as usize, y as usize, curb_col);
                } else if (x == center_line || x == center_line + 1) && stripe_on {
                    // Dashed center highway line
                    frame.set(x as usize, y as usize, Rgb::WHITE);
                } else {
                    // Dark asphalt
                    let d_c = to_byte(12.0 + y_norm * 18.0);
                    frame.set(x as usize, y as usize, Rgb::new(d_c, d_c, to_byte(d_c as f32 * 1.3)));
                }
            }
        }

        // 3. Pixel Art Sports Car in Foreground (Position: cx=15.5 + slight steering sway, y: 24..=29)
        let car_sway = (curve_amount * 0.45).round() as i32;
        let car_cx = 15 + car_sway;
        let car_base_y = 25i32;

        // Car Roof / Rear Window (Width 5, Height 2 at car_cx - 2, car_base_y)
        for dy in 0..2i32 {
            for dx in 0..5i32 {
                let px = car_cx - 2 + dx;
                let py = car_base_y + dy;
                if px >= 0 && px < WIDTH as i32 && py < HEIGHT as i32 {
                    let is_window = dy == 1 && (1..=3).contains(&dx);
                    let c = if is_window {
                        Rgb::new(20, 60, 100) // Tinted glass
                    } else {
                        car_col
                    };
                    frame.set(px as usize, py as usize, c);
                }
            }
        }

        // Car Lower Body (Width 9, Height 3 at car_cx - 4, car_base_y + 2)
        for dy in 0..3i32 {
            for dx in 0..9i32 {
                let px = car_cx - 4 + dx;
                let py = car_base_y + 2 + dy;
                if px >= 0 && px < WIDTH as i32 && py < HEIGHT as i32 {
                    let is_taillight_left = dy == 0 && (dx == 1 || dx == 2);
                    let is_taillight_right = dy == 0 && (dx == 6 || dx == 7);
                    let is_tire = dy == 2 && (dx == 0 || dx == 8);

                    if is_taillight_left || is_taillight_right {
                        // Glowing Red Tail Lights
                        let light_c = if beat {
                            Rgb::WHITE // Brake flash
                        } else {
                            Rgb::new(255, 30, 20)
                        };
                        frame.set(px as usize, py as usize, light_c);
                    } else if is_tire {
                        frame.set(px as usize, py as usize, Rgb::new(10, 10, 10));
                    } else {
                        frame.set(px as usize, py as usize, car_col);
                    }
                }
            }
        }

        // 4. Nitro Exhaust Flames on Bass/Beat
        let show_flames = self.values[2] > 0.5;
        if show_flames && (self.nitro_boost > 0.2 || bass > 0.4) {
            let flame_len = (2.0 + bass * 3.0 + self.nitro_boost * 2.0).round() as i32;
            let exhaust_left = car_cx - 3;
            let exhaust_right = car_cx + 3;
            let flame_y = car_base_y + 5;

            for f in 0..flame_len {
                let fy = flame_y + f;
                if fy < HEIGHT as i32 {
                    let flame_c = if f == 0 {
                        Rgb::new(0, 240, 255) // Hot cyan core
                    } else {
                        Rgb::new(255, to_byte(150.0 - f as f32 * 30.0), 20)
                    };
                    if exhaust_left >= 0 && exhaust_left < WIDTH as i32 {
                        frame.set(exhaust_left as usize, fy as usize, flame_c);
                    }
                    if exhaust_right >= 0 && exhaust_right < WIDTH as i32 {
                        frame.set(exhaust_right as usize, fy as usize, flame_c);
                    }
                }
            }
        }
    }
}
