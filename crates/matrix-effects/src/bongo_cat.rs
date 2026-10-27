use crate::util::{audio_bass, audio_mid, audio_treble, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const PALETTES: &[&str] = &["Classic White", "Calico", "Midnight Cat", "Rainbow Rave"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "Drum tempo",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 50.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "bongo_glow",
        label: "Impact glow",
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
        label: "Cat fur style",
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

pub struct BongoCatEffect {
    values: [f32; 4],
    drum_timer: f32,
    l_paw_down: bool,
    r_paw_down: bool,
    l_impact: f32,
    r_impact: f32,
    color_cycle: f32,
}

impl Default for BongoCatEffect {
    fn default() -> Self {
        Self {
            values: [50.0, 1.0, 0.0, 1.0],
            drum_timer: 0.0,
            l_paw_down: true,
            r_paw_down: false,
            l_impact: 0.0,
            r_impact: 0.0,
            color_cycle: 0.0,
        }
    }
}

impl BongoCatEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn cat_colors(pal: usize, color_cycle: f32) -> (Rgb, Rgb, Rgb) {
        // (Fur, InnerEar/Nose, Outline)
        match pal {
            0 => (Rgb::WHITE, Rgb::new(255, 170, 190), Rgb::new(40, 40, 50)),
            1 => (Rgb::new(245, 170, 70), Rgb::new(255, 150, 170), Rgb::new(60, 30, 10)),
            2 => (Rgb::new(45, 45, 55), Rgb::new(120, 80, 140), Rgb::new(15, 15, 20)),
            _ => (
                hsv_to_rgb(color_cycle, 0.75, 1.0),
                Rgb::new(255, 255, 255),
                Rgb::new(20, 20, 30),
            ),
        }
    }

    fn draw_musical_note(frame: &mut Frame, x: usize, y: usize, col: Rgb) {
        if x + 2 < WIDTH && y + 3 < HEIGHT {
            // Note stem & flag
            frame.set(x + 2, y, col);
            frame.set(x + 2, y + 1, col);
            frame.set(x + 2, y + 2, col);
            frame.set(x + 1, y + 2, col);
            frame.set(x, y + 2, col); // Note head
            frame.set(x + 1, y, col); // Flag
        }
    }
}

impl Effect for BongoCatEffect {
    fn id(&self) -> &'static str {
        "bongo_cat"
    }

    fn name(&self) -> &'static str {
        "Bongo Cat"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "bongo_glow" => Some(self.values[1]),
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
            "bongo_glow" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.drum_timer = 0.0;
        self.l_paw_down = true;
        self.r_paw_down = false;
        self.l_impact = 0.0;
        self.r_impact = 0.0;
        self.color_cycle = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::new(12, 14, 28)); // Studio room background

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[3] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.9);
        self.drum_timer += dt * speed * 3.5;
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        // Paw state update: alternating left/right drumming
        if beat {
            // Beat triggers rapid double hit or switch
            self.l_paw_down = true;
            self.r_paw_down = true;
            self.l_impact = 1.0;
            self.r_impact = 1.0;
        } else {
            let cycle = (self.drum_timer % 2.0).floor() as usize;
            let prev_l = self.l_paw_down;
            let prev_r = self.r_paw_down;
            self.l_paw_down = cycle == 0;
            self.r_paw_down = cycle == 1;

            if self.l_paw_down && !prev_l {
                self.l_impact = 1.0;
            }
            if self.r_paw_down && !prev_r {
                self.r_impact = 1.0;
            }
        }

        self.l_impact = (self.l_impact - dt * 5.0).max(0.0);
        self.r_impact = (self.r_impact - dt * 5.0).max(0.0);

        let pal = self.values[2] as usize;
        let (fur_c, ear_c, out_c) = Self::cat_colors(pal, self.color_cycle);

        // 1. Studio Desk Table (y: 22..31)
        let desk_c = Rgb::new(60, 40, 30);
        let desk_rim = Rgb::new(90, 60, 45);
        for x in 0..WIDTH {
            frame.set(x, 22, desk_rim);
            for y in 23..HEIGHT {
                frame.set(x, y, desk_c);
            }
        }

        // 2. Bongo Drums:
        // Left Bongo: cx = 9, cy = 24, r = 4.5
        // Right Bongo: cx = 22, cy = 24, r = 4.5
        let bongo_rim_c = Rgb::new(190, 150, 80);
        let bongo_head_c = Rgb::new(235, 220, 190);
        let glow_active = self.values[1] > 0.5;

        for &(bcx, bimpact) in &[(9.0f32, self.l_impact), (22.0f32, self.r_impact)] {
            for dy in -2..=5i32 {
                let py = 24 + dy;
                if py >= HEIGHT as i32 {
                    continue;
                }
                for dx in -4..=4i32 {
                    let px = (bcx + dx as f32).round() as i32;
                    if px >= 0 && px < WIDTH as i32 {
                        let d_norm = (dx as f32).abs() / 4.5;
                        if d_norm <= 1.0 {
                            let mut c = if dy == -2 || dy == -1 {
                                // Drum head skin
                                if glow_active && bimpact > 0.1 {
                                    let boost = to_byte(bimpact * 200.0);
                                    Rgb::new(
                                        bongo_head_c.r.saturating_add(boost),
                                        bongo_head_c.g.saturating_add(boost / 2),
                                        bongo_head_c.b,
                                    )
                                } else {
                                    bongo_head_c
                                }
                            } else {
                                // Wooden drum shell
                                bongo_rim_c
                            };
                            if d_norm > 0.85 {
                                c = out_c; // Outline
                            }
                            frame.set(px as usize, py as usize, c);
                        }
                    }
                }
            }
        }

        // 3. Cat Head & Ears (cx = 15.5, cy = 13.0)
        let head_cx = 15.5f32;
        let head_cy = 13.0f32;

        // Cat Body / Chest (y: 16..22, x: 9..22)
        for y in 16..22 {
            for x in 9..=22 {
                frame.set(x, y, fur_c);
            }
        }

        // Cat Head Circle (radius 6.5)
        for dy in -6..=5i32 {
            let py = (head_cy + dy as f32).round() as i32;
            if py < 0 || py >= HEIGHT as i32 {
                continue;
            }
            for dx in -7..=7i32 {
                let px = (head_cx + dx as f32).round() as i32;
                if px >= 0 && px < WIDTH as i32 {
                    let d2 = (dx * dx + dy * dy) as f32;
                    if d2 <= 42.0 {
                        let c = if d2 >= 33.0 { out_c } else { fur_c };
                        frame.set(px as usize, py as usize, c);
                    }
                }
            }
        }

        // Pointy Cat Ears
        // Left Ear: tip at (8, 4), base at (6..11, 8)
        let l_ear_pts = [(8, 4), (7, 5), (8, 5), (9, 5), (6, 6), (7, 6), (8, 6), (9, 6), (10, 6)];
        for &(ex, ey) in &l_ear_pts {
            frame.set(ex, ey, if ex == 8 && ey == 5 { ear_c } else { fur_c });
        }
        // Right Ear: tip at (23, 4), base at (20..25, 8)
        let r_ear_pts = [(23, 4), (22, 5), (23, 5), (24, 5), (21, 6), (22, 6), (23, 6), (24, 6), (25, 6)];
        for &(ex, ey) in &r_ear_pts {
            frame.set(ex, ey, if ex == 23 && ey == 5 { ear_c } else { fur_c });
        }

        // Happy Cat Eyes (^ ^)
        // Left Eye at (12, 12), Right Eye at (19, 12)
        let eye_c = out_c;
        frame.set(11, 12, eye_c);
        frame.set(12, 11, eye_c);
        frame.set(13, 12, eye_c);

        frame.set(18, 12, eye_c);
        frame.set(19, 11, eye_c);
        frame.set(20, 12, eye_c);

        // Cute Pink Nose & Mouth at (15, 14) and (16, 14)
        frame.set(15, 14, ear_c);
        frame.set(16, 14, ear_c);
        frame.set(15, 15, eye_c);
        frame.set(16, 15, eye_c);

        // 4. Cat Paws:
        // Left Paw:
        // If DOWN: reaches down to (x: 8..11, y: 22..24)
        // If UP: raised up to (x: 8..11, y: 17..19)
        let l_paw_y = if self.l_paw_down { 22i32 } else { 17i32 };
        for dy in 0..=2i32 {
            for dx in 0..=3i32 {
                let px = 8 + dx;
                let py = l_paw_y + dy;
                if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                    let is_paw_edge = dx == 0 || dx == 3 || dy == 2;
                    frame.set(px as usize, py as usize, if is_paw_edge { out_c } else { fur_c });
                }
            }
        }

        // Right Paw:
        // If DOWN: reaches down to (x: 20..23, y: 22..24)
        // If UP: raised up to (x: 20..23, y: 17..19)
        let r_paw_y = if self.r_paw_down { 22i32 } else { 17i32 };
        for dy in 0..=2i32 {
            for dx in 0..=3i32 {
                let px = 20 + dx;
                let py = r_paw_y + dy;
                if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                    let is_paw_edge = dx == 0 || dx == 3 || dy == 2;
                    frame.set(px as usize, py as usize, if is_paw_edge { out_c } else { fur_c });
                }
            }
        }

        // 5. Sound Waves & Floating Musical Notes
        if self.l_impact > 0.3 {
            let note_y = (18.0 - self.l_impact * 8.0).round() as usize;
            Self::draw_musical_note(frame, 4, note_y, Rgb::new(255, 230, 80));
        }
        if self.r_impact > 0.3 {
            let note_y = (18.0 - self.r_impact * 8.0).round() as usize;
            Self::draw_musical_note(frame, 25, note_y, Rgb::new(100, 230, 255));
        }

        // Background equalizer bars on treble / mid
        if audio_active && mid > 0.25 {
            for bar in 0..6 {
                let bx = 3 + bar * 5;
                let bh = ((context.audio.bands[bar * 2] * 7.0).round() as usize).min(6);
                for by in 0..bh {
                    if (8 - by) < HEIGHT {
                        let c = if treble > 0.5 { Rgb::new(80, 200, 255) } else { Rgb::new(40, 120, 200) };
                        frame.set(bx, 8 - by, c);
                    }
                }
            }
        }
    }
}
