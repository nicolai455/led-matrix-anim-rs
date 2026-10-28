use crate::util::{audio_bass, audio_mid, audio_treble, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Cyber Duo", "Retro 80s", "Gold & Silver", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "Dance tempo",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 40.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "dancers",
        label: "Dancer count",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 3.0,
        step: 1.0,
        default: 2.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Outfits & Lights",
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

pub struct PartyDancersEffect {
    values: [f32; 4],
    beat_step: f32,
    color_cycle: f32,
    move_style: usize,
    spin_angle: f32,
}

impl Default for PartyDancersEffect {
    fn default() -> Self {
        Self {
            values: [40.0, 2.0, 3.0, 1.0],
            beat_step: 0.0,
            color_cycle: 0.0,
            move_style: 0,
            spin_angle: 0.0,
        }
    }
}

impl PartyDancersEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn outfit_colors(pal: usize, dancer_idx: usize, color_cycle: f32) -> (Rgb, Rgb, Rgb) {
        // Returns (hair, shirt, pants)
        match pal {
            0 => {
                // Cyber Duo
                if dancer_idx == 0 {
                    (Rgb::new(255, 0, 180), Rgb::new(0, 240, 255), Rgb::new(20, 20, 40))
                } else {
                    (Rgb::new(0, 255, 140), Rgb::new(255, 30, 100), Rgb::new(30, 20, 50))
                }
            }
            1 => {
                // Retro 80s
                if dancer_idx == 0 {
                    (Rgb::new(240, 220, 40), Rgb::new(255, 80, 20), Rgb::new(20, 120, 240))
                } else {
                    (Rgb::new(180, 40, 20), Rgb::new(240, 240, 240), Rgb::new(255, 20, 140))
                }
            }
            2 => {
                // Gold & Silver
                if dancer_idx == 0 {
                    (Rgb::new(40, 40, 40), Rgb::new(255, 215, 0), Rgb::new(30, 30, 30))
                } else {
                    (Rgb::new(220, 220, 240), Rgb::new(192, 192, 210), Rgb::new(20, 20, 30))
                }
            }
            _ => {
                // Rainbow
                let h1 = (color_cycle + dancer_idx as f32 * 0.4) % 1.0;
                let h2 = (h1 + 0.33) % 1.0;
                let h3 = (h1 + 0.66) % 1.0;
                (hsv_to_rgb(h1, 0.9, 1.0), hsv_to_rgb(h2, 0.9, 1.0), hsv_to_rgb(h3, 0.8, 0.9))
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_dancer(
        frame: &mut Frame,
        cx: i32,
        ground_y: i32,
        step: f32,
        move_style: usize,
        hair_c: Rgb,
        shirt_c: Rgb,
        pants_c: Rgb,
        skin_c: Rgb,
    ) {
        let step_phase = (step * 2.0 * PI).sin();
        let bounce = (step_phase.abs() * 2.5).round() as i32;

        let base_y = ground_y - bounce;

        // Head position
        let head_y = base_y - 12;
        let head_tilt = (step_phase * 1.2).round() as i32;

        // 1. Head & Face (3x3 pixels at cx - 1 + head_tilt, head_y)
        let hx = cx - 1 + head_tilt;
        for dy in 0..3 {
            for dx in 0..3 {
                let px = hx + dx;
                let py = head_y + dy;
                if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                    let is_hair = dy == 0 || (dy == 1 && dx == 0);
                    let is_eye = dy == 1 && dx == 2;
                    let col = if is_hair {
                        hair_c
                    } else if is_eye {
                        Rgb::BLACK
                    } else {
                        skin_c
                    };
                    frame.set(px as usize, py as usize, col);
                }
            }
        }

        // 2. Torso (3x4 pixels at cx - 1, base_y - 9)
        let tx = cx - 1;
        let ty = base_y - 9;
        for dy in 0..4 {
            for dx in 0..3 {
                let px = tx + dx;
                let py = ty + dy;
                if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                    frame.set(px as usize, py as usize, shirt_c);
                }
            }
        }

        // 3. Arms (animated based on move_style and step)
        let arm_l_y;
        let arm_r_y;
        let arm_l_x = cx - 2;
        let arm_r_x = cx + 2;

        match move_style % 3 {
            0 => {
                // Wave Arms (Alternate Up/Down)
                if step_phase > 0.0 {
                    arm_l_y = ty;
                    arm_r_y = ty + 2;
                } else {
                    arm_l_y = ty + 2;
                    arm_r_y = ty;
                }
            }
            1 => {
                // Hands in the air!
                arm_l_y = ty - 1;
                arm_r_y = ty - 1;
            }
            _ => {
                // Disco Point (diagonal)
                arm_l_y = ty + 2;
                arm_r_y = ty - 2;
            }
        }

        for &(ax, ay) in &[(arm_l_x, arm_l_y), (arm_r_x, arm_r_y)] {
            if ax >= 0 && ax < WIDTH as i32 && ay >= 0 && ay < HEIGHT as i32 {
                frame.set(ax as usize, ay as usize, skin_c);
            }
        }

        // 4. Legs (Pants)
        let leg_y = base_y - 5;
        let leg_spread = if step_phase.abs() > 0.5 { 1 } else { 0 };

        for dy in 0..4 {
            let py = leg_y + dy;
            let lx1 = cx - 1 - if dy > 1 { leg_spread } else { 0 };
            let lx2 = cx + 1 + if dy > 1 { leg_spread } else { 0 };

            if py >= 0 && py < HEIGHT as i32 {
                if lx1 >= 0 && lx1 < WIDTH as i32 {
                    frame.set(lx1 as usize, py as usize, pants_c);
                }
                if lx2 >= 0 && lx2 < WIDTH as i32 {
                    frame.set(lx2 as usize, py as usize, pants_c);
                }
            }
        }

        // 5. Shoes (Bottom line)
        let shoe_y = base_y - 1;
        if shoe_y >= 0 && shoe_y < HEIGHT as i32 {
            let sx1 = cx - 1 - leg_spread;
            let sx2 = cx + 1 + leg_spread;
            if sx1 >= 0 && sx1 < WIDTH as i32 {
                frame.set(sx1 as usize, shoe_y as usize, Rgb::WHITE);
            }
            if sx2 >= 0 && sx2 < WIDTH as i32 {
                frame.set(sx2 as usize, shoe_y as usize, Rgb::WHITE);
            }
        }
    }
}

impl Effect for PartyDancersEffect {
    fn id(&self) -> &'static str {
        "party_dancers"
    }

    fn name(&self) -> &'static str {
        "Party Pixel Dancers"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "dancers" => Some(self.values[1]),
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
            "dancers" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.beat_step = 0.0;
        self.color_cycle = 0.0;
        self.move_style = 0;
        self.spin_angle = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::BLACK);

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[3] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let _treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.move_style = (self.move_style + 1) % 3;
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.8);
        self.beat_step = (self.beat_step + dt * speed * 2.0) % 1.0;
        self.spin_angle = (self.spin_angle + dt * 2.5) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let pal = self.values[2] as usize;
        let dancer_count = (self.values[1].round() as usize).clamp(1, 3);

        // 1. Moving Club Spotlights in Background
        let spot_x1 = 15.5 + (self.spin_angle).sin() * 12.0;
        let spot_x2 = 15.5 - (self.spin_angle * 1.3).cos() * 12.0;

        for y in 0..HEIGHT {
            let yf = y as f32;
            let cone_w = (yf * 0.45).max(1.0);
            for x in 0..WIDTH {
                let xf = x as f32;
                let in_cone1 = (xf - spot_x1).abs() < cone_w;
                let in_cone2 = (xf - spot_x2).abs() < cone_w;

                if in_cone1 || in_cone2 {
                    let spot_bright = to_byte((yf / 32.0) * (40.0 + mid * 40.0));
                    frame.set(x, y, Rgb::new(spot_bright / 2, spot_bright / 3, spot_bright));
                }
            }
        }

        // 2. Stage Floor at y >= 27
        for y in 27..HEIGHT {
            for x in 0..WIDTH {
                let is_front = y == 27;
                let col = if is_front {
                    let r = to_byte(255.0 * (0.5 + 0.5 * (x as f32 * 0.5 + self.spin_angle * 3.0).sin()));
                    let g = to_byte(255.0 * (0.5 + 0.5 * (x as f32 * 0.5 + self.spin_angle * 3.0 + 2.0).sin()));
                    let b = to_byte(255.0 * (0.5 + 0.5 * (x as f32 * 0.5 + self.spin_angle * 3.0 + 4.0).sin()));
                    Rgb::new(r, g, b)
                } else {
                    let tile_even = ((x / 4) + (y / 2)) % 2 == 0;
                    if tile_even { Rgb::new(20, 20, 30) } else { Rgb::new(10, 10, 15) }
                };
                frame.set(x, y, col);
            }
        }

        // 3. Render Dancers
        let skin = Rgb::new(255, 205, 175);
        let dancer_positions: &[i32] = match dancer_count {
            1 => &[16],
            2 => &[10, 22],
            _ => &[7, 16, 25],
        };

        for (idx, &cx) in dancer_positions.iter().enumerate() {
            let (hair, shirt, pants) = Self::outfit_colors(pal, idx, self.color_cycle);
            let dancer_step = (self.beat_step + idx as f32 * 0.5) % 1.0;
            Self::draw_dancer(
                frame,
                cx,
                27,
                dancer_step,
                self.move_style,
                hair,
                shirt,
                pants,
                skin,
            );
        }
    }
}
