use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const MODES: &[&str] = &["Wig-Wag Quad Strobe", "Rotating Dual Beacon", "Split Blinder", "Audio Beat Blast"];
const PALETTES: &[&str] = &["Red & Blue Police", "Amber & White", "Magenta & Cyan", "Rainbow Strobe"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Flash frequency",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 50.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "mode",
        label: "Strobe pattern",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 3.0,
        step: 1.0,
        default: 0.0,
        options: MODES,
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Beacon colors",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 3.0,
        step: 1.0,
        default: 0.0,
        options: PALETTES,
        unit: "",
    },
    ParameterDescriptor {
        id: "persistence",
        label: "Phosphor decay",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 90.0,
        step: 1.0,
        default: 50.0,
        options: &[],
        unit: "%",
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

pub struct PoliceStrobeEffect {
    values: [f32; 5],
    timer: f32,
    beacon_angle: f32,
    flash_l: f32,
    flash_r: f32,
    beat_shock: f32,
    color_cycle: f32,
}

impl Default for PoliceStrobeEffect {
    fn default() -> Self {
        Self {
            values: [50.0, 0.0, 0.0, 50.0, 1.0],
            timer: 0.0,
            beacon_angle: 0.0,
            flash_l: 0.0,
            flash_r: 0.0,
            beat_shock: 0.0,
            color_cycle: 0.0,
        }
    }
}

impl PoliceStrobeEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn strobe_colors(pal: usize, color_cycle: f32) -> (Rgb, Rgb) {
        // (Color A, Color B)
        match pal {
            0 => (Rgb::new(255, 10, 20), Rgb::new(10, 80, 255)), // Red & Blue
            1 => (Rgb::new(255, 170, 0), Rgb::new(250, 250, 255)), // Amber & White
            2 => (Rgb::new(255, 20, 180), Rgb::new(0, 240, 255)), // Magenta & Cyan
            _ => (
                hsv_to_rgb(color_cycle, 1.0, 1.0),
                hsv_to_rgb((color_cycle + 0.5) % 1.0, 1.0, 1.0),
            ),
        }
    }
}

impl Effect for PoliceStrobeEffect {
    fn id(&self) -> &'static str {
        "police_strobe"
    }

    fn name(&self) -> &'static str {
        "Police Strobe Beacon"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "mode" => Some(self.values[1]),
            "palette" => Some(self.values[2]),
            "persistence" => Some(self.values[3]),
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
            "mode" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "persistence" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.timer = 0.0;
        self.beacon_angle = 0.0;
        self.flash_l = 0.0;
        self.flash_r = 0.0;
        self.beat_shock = 0.0;
        self.color_cycle = 0.0;
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
            self.beat_shock = 1.0;
        } else {
            self.beat_shock = (self.beat_shock - dt * 4.0).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.9);
        self.timer += dt * speed * 3.5;
        self.beacon_angle = (self.beacon_angle + dt * speed * 8.0) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let mode = self.values[1] as usize;
        let pal = self.values[2] as usize;
        let (color_a, color_b) = Self::strobe_colors(pal, self.color_cycle);

        let decay_rate = 1.0 - (self.values[3] / 100.0) * 0.8;

        match mode {
            0 => {
                // Wig-Wag Quad Strobe:
                // Period = 1.0 sec: Left side does 3 quick pulses, then Right side does 3 quick pulses
                let cycle = (self.timer % 1.2) / 1.2; // 0..1
                let (target_l, target_r) = if cycle < 0.45 {
                    // Left 3 strobe flashes
                    let sub = cycle / 0.45;
                    #[allow(clippy::manual_is_multiple_of)]
                    let is_flash = ((sub * 6.0).floor() as usize) % 2 == 0;
                    (if is_flash { 1.0 } else { 0.0 }, 0.0)
                } else if cycle < 0.9 {
                    // Right 3 strobe flashes
                    let sub = (cycle - 0.45) / 0.45;
                    #[allow(clippy::manual_is_multiple_of)]
                    let is_flash = ((sub * 6.0).floor() as usize) % 2 == 0;
                    (0.0, if is_flash { 1.0 } else { 0.0 })
                } else {
                    (0.0, 0.0)
                };

                // Fast attack, configurable phosphor decay
                if target_l > self.flash_l { self.flash_l = target_l; } else { self.flash_l = (self.flash_l - dt * 14.0 * decay_rate).max(0.0); }
                if target_r > self.flash_r { self.flash_r = target_r; } else { self.flash_r = (self.flash_r - dt * 14.0 * decay_rate).max(0.0); }

                // Audio beat strobe boost
                if beat {
                    self.flash_l = 1.0;
                    self.flash_r = 1.0;
                }

                // Render Left split (x: 0..16) and Right split (x: 16..32)
                for y in 0..HEIGHT {
                    let y_fade = 1.0 - (y as f32 - 15.5).abs() / 16.0 * 0.3;
                    for x in 0..WIDTH {
                        let (int, col) = if x < 16 {
                            (self.flash_l * y_fade, color_a)
                        } else {
                            (self.flash_r * y_fade, color_b)
                        };

                        if int > 0.02 {
                            let b = clamp01(int);
                            // Super bright core in center of each pod
                            let pod_cx = if x < 16 { 7.5f32 } else { 23.5f32 };
                            let d = ((x as f32 - pod_cx).powi(2) + (y as f32 - 15.5).powi(2)).sqrt();
                            let is_core = d < 4.0 && b > 0.6;

                            let pixel_c = if is_core {
                                Rgb::WHITE
                            } else {
                                Rgb::new(to_byte(col.r as f32 * b), to_byte(col.g as f32 * b), to_byte(col.b as f32 * b))
                            };
                            frame.set(x, y, pixel_c);
                        }
                    }
                }
            }
            1 => {
                // Rotating Dual Beacon:
                // Red beacon on one side, Blue beacon opposite (PI offset), sweeping 360 deg
                let cx = 15.5f32;
                let cy = 15.5f32;
                let ang1 = self.beacon_angle;
                let ang2 = (self.beacon_angle + PI) % (2.0 * PI);

                for y in 0..HEIGHT {
                    let dy = y as f32 - cy;
                    for x in 0..WIDTH {
                        let dx = x as f32 - cx;
                        let r = (dx * dx + dy * dy).sqrt();
                        if r < 0.5 {
                            frame.set(x, y, Rgb::WHITE);
                            continue;
                        }

                        let p_ang = dy.atan2(dx);

                        // Angular distance to beam 1 (Color A)
                        let mut diff1 = (p_ang - ang1).abs();
                        if diff1 > PI { diff1 = 2.0 * PI - diff1; }

                        // Angular distance to beam 2 (Color B)
                        let mut diff2 = (p_ang - ang2).abs();
                        if diff2 > PI { diff2 = 2.0 * PI - diff2; }

                        let beam_w = 0.55 + bass * 0.25;

                        let mut acc_r = 0u16;
                        let mut acc_g = 0u16;
                        let mut acc_b = 0u16;

                        if diff1 < beam_w {
                            let b = (1.0 - diff1 / beam_w).powi(2) * (1.0 - r / 20.0).max(0.1);
                            acc_r = acc_r.saturating_add((color_a.r as f32 * b) as u16);
                            acc_g = acc_g.saturating_add((color_a.g as f32 * b) as u16);
                            acc_b = acc_b.saturating_add((color_a.b as f32 * b) as u16);
                        }

                        if diff2 < beam_w {
                            let b = (1.0 - diff2 / beam_w).powi(2) * (1.0 - r / 20.0).max(0.1);
                            acc_r = acc_r.saturating_add((color_b.r as f32 * b) as u16);
                            acc_g = acc_g.saturating_add((color_b.g as f32 * b) as u16);
                            acc_b = acc_b.saturating_add((color_b.b as f32 * b) as u16);
                        }

                        if self.beat_shock > 0.1 {
                            let shock = to_byte(self.beat_shock * 150.0);
                            acc_r = acc_r.saturating_add(shock as u16);
                            acc_g = acc_g.saturating_add(shock as u16);
                            acc_b = acc_b.saturating_add(shock as u16);
                        }

                        frame.set(
                            x,
                            y,
                            Rgb::new(
                                acc_r.min(255) as u8,
                                acc_g.min(255) as u8,
                                acc_b.min(255) as u8,
                            ),
                        );
                    }
                }
            }
            2 => {
                // Split Blinder (Top Red, Bottom Blue rapid alternations with center blinders)
                #[allow(clippy::manual_is_multiple_of)]
                let alt = ((self.timer * 4.0).floor() as usize) % 2 == 0;
                let (top_c, bot_c) = if alt { (color_a, color_b) } else { (color_b, color_a) };

                for y in 0..HEIGHT {
                    let is_top = y < 16;
                    let c = if is_top { top_c } else { bot_c };
                    let row_boost = ((y as f32 - 15.5).abs() / 16.0 * (1.0 + bass * 0.5)).min(1.0);

                    for x in 0..WIDTH {
                        let strobe_col = (x % 4 == 0) || (y % 4 == 0);
                        let b = if strobe_col { 1.0 } else { 0.35 * row_boost };
                        frame.set(x, y, Rgb::new(to_byte(c.r as f32 * b), to_byte(c.g as f32 * b), to_byte(c.b as f32 * b)));
                    }
                }
            }
            _ => {
                // Audio Beat Blast: Full panel explodes with Red/Blue/White strobing strobe rays on audio level
                let blast_int = (context.audio.level * 1.5 + bass * 1.2 + self.beat_shock * 1.8).clamp(0.0, 1.0);
                for y in 0..HEIGHT {
                    for x in 0..WIDTH {
                        let checker = ((x / 4) + (y / 4)) % 2 == 0;
                        let col = if checker { color_a } else { color_b };
                        let b = blast_int * (0.4 + 0.6 * ((x as f32 * 0.8 + y as f32 * 0.8 - self.timer * 8.0).sin() * 0.5 + 0.5));
                        frame.set(x, y, Rgb::new(to_byte(col.r as f32 * b), to_byte(col.g as f32 * b), to_byte(col.b as f32 * b)));
                    }
                }
            }
        }

        // Treble sparkles on high frequencies
        if audio_active && treble > 0.4 {
            for i in 0..4 {
                let sx = (i * 9 + 3) % WIDTH;
                let sy = (i * 7 + 2) % HEIGHT;
                frame.set(sx, sy, Rgb::WHITE);
            }
        }
    }
}
