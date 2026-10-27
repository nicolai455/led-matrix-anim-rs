use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Boombox Silver", "Subwoofer Carbon", "Neon Soundclash", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Idle groove",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 35.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "excursion",
        label: "Cone excursion",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 70.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "shockwaves",
        label: "Air pressure rings",
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
        label: "Cabinet style",
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

pub struct BouncingSpeakersEffect {
    values: [f32; 5],
    smooth_pump: f32,
    color_cycle: f32,
    wave_radius: f32,
    phase: f32,
}

impl Default for BouncingSpeakersEffect {
    fn default() -> Self {
        Self {
            values: [35.0, 70.0, 1.0, 3.0, 1.0],
            smooth_pump: 0.0,
            color_cycle: 0.0,
            wave_radius: 0.0,
            phase: 0.0,
        }
    }
}

impl BouncingSpeakersEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn cabinet_color(pal: usize, color_cycle: f32) -> (Rgb, Rgb, Rgb) {
        // Returns (cabinet_c, cone_c, dustcap_c)
        match pal {
            0 => {
                // Boombox Silver
                (Rgb::new(160, 160, 175), Rgb::new(45, 45, 55), Rgb::new(210, 210, 225))
            }
            1 => {
                // Subwoofer Carbon
                (Rgb::new(25, 25, 30), Rgb::new(15, 15, 18), Rgb::new(240, 60, 20))
            }
            2 => {
                // Neon Soundclash
                (Rgb::new(20, 10, 35), Rgb::new(255, 0, 140), Rgb::new(0, 240, 255))
            }
            _ => {
                // Rainbow
                let c1 = hsv_to_rgb(color_cycle, 0.8, 0.4);
                let c2 = hsv_to_rgb((color_cycle + 0.3) % 1.0, 0.9, 0.8);
                let c3 = hsv_to_rgb((color_cycle + 0.6) % 1.0, 0.95, 1.0);
                (c1, c2, c3)
            }
        }
    }

    fn draw_woofer(
        frame: &mut Frame,
        cx: f32,
        cy: f32,
        base_r: f32,
        excursion_boost: f32,
        cone_c: Rgb,
        dustcap_c: Rgb,
    ) {
        let cur_r = base_r + excursion_boost * 2.2;
        let r_sq = cur_r * cur_r;
        let cap_r = cur_r * 0.42;
        let cap_sq = cap_r * cap_r;

        let y_start = (cy - cur_r - 1.0).max(0.0) as usize;
        let y_end = (cy + cur_r + 1.0).min(HEIGHT as f32 - 1.0) as usize;
        let x_start = (cx - cur_r - 1.0).max(0.0) as usize;
        let x_end = (cx + cur_r + 1.0).min(WIDTH as f32 - 1.0) as usize;

        for y in y_start..=y_end {
            let dy = y as f32 - cy;
            for x in x_start..=x_end {
                let dx = x as f32 - cx;
                let d2 = dx * dx + dy * dy;

                if d2 <= r_sq {
                    let dist = d2.sqrt();
                    let norm = dist / cur_r;

                    if d2 <= cap_sq {
                        // Central dome / dust cap (3D convex highlight)
                        let dome_shade = 1.0 - (dist / cap_r) * 0.5;
                        let r = to_byte(dustcap_c.r as f32 * dome_shade);
                        let g = to_byte(dustcap_c.g as f32 * dome_shade);
                        let b = to_byte(dustcap_c.b as f32 * dome_shade);
                        frame.set(x, y, Rgb::new(r, g, b));
                    } else if norm > 0.86 {
                        // Rubber surround roll (dark outer ring)
                        frame.set(x, y, Rgb::new(20, 20, 25));
                    } else {
                        // Conical diaphragm (concave cone depth gradient)
                        let cone_shade = 0.5 + 0.5 * norm;
                        let r = to_byte(cone_c.r as f32 * cone_shade);
                        let g = to_byte(cone_c.g as f32 * cone_shade);
                        let b = to_byte(cone_c.b as f32 * cone_shade);
                        frame.set(x, y, Rgb::new(r, g, b));
                    }
                }
            }
        }
    }
}

impl Effect for BouncingSpeakersEffect {
    fn id(&self) -> &'static str {
        "bouncing_speakers"
    }

    fn name(&self) -> &'static str {
        "Bouncing Woofers"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "excursion" => Some(self.values[1]),
            "shockwaves" => Some(self.values[2]),
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
            "excursion" => self.values[1] = value,
            "shockwaves" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.smooth_pump = 0.0;
        self.color_cycle = 0.0;
        self.wave_radius = 0.0;
        self.phase = 0.0;
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

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.7);
        self.phase = (self.phase + dt * speed * 4.0) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.16) % 1.0;

        let excursion_scale = self.values[1] / 100.0;
        let target_pump = if audio_active {
            (bass * 1.5 + (if beat { 1.2 } else { 0.0 })).min(2.0)
        } else {
            0.5 + 0.5 * (self.phase * 2.0).sin()
        };

        // Smooth woofer spring return physics
        self.smooth_pump += (target_pump - self.smooth_pump) * (dt * 22.0).min(1.0);
        let pump = self.smooth_pump * excursion_scale;

        // Shockwave expansion
        if beat {
            self.wave_radius = 3.0;
        } else if self.wave_radius > 0.0 {
            self.wave_radius += dt * 35.0;
            if self.wave_radius > 28.0 {
                self.wave_radius = 0.0;
            }
        }

        let pal = self.values[3] as usize;
        let (cab_col, cone_col, cap_col) = Self::cabinet_color(pal, self.color_cycle);

        // 1. Draw Speaker Cabinet Frame (Outer box at x: 2..=29, y: 1..=30)
        frame.clear(Rgb::BLACK);

        for y in 1..=30 {
            for x in 2..=29 {
                let is_edge = x == 2 || x == 29 || y == 1 || y == 30;
                let c = if is_edge {
                    Rgb::new(to_byte(cab_col.r as f32 * 1.3), to_byte(cab_col.g as f32 * 1.3), to_byte(cab_col.b as f32 * 1.3))
                } else {
                    cab_col
                };
                frame.set(x, y, c);
            }
        }

        // 2. High-Frequency Tweeter Horn at Top (cx=15.5, cy=6.0, r=3.5)
        let tweeter_shake = if audio_active { treble * 1.2 } else { 0.0 };
        Self::draw_woofer(frame, 15.5, 6.0, 3.5, tweeter_shake, Rgb::new(20, 20, 25), cap_col);

        // 3. Dual Main Subwoofers:
        // Left Woofer at (cx=9.5, cy=19.5)
        // Right Woofer at (cx=21.5, cy=19.5)
        let main_r = 5.2f32;
        Self::draw_woofer(frame, 9.5, 19.5, main_r, pump, cone_col, cap_col);
        Self::draw_woofer(frame, 21.5, 19.5, main_r, pump, cone_col, cap_col);

        // 4. Center LED VU Meter bar between woofers (x=15..=16, y=12..=26)
        let vu_height = ((mid * 0.7 + bass * 0.3) * 14.0).round() as i32;
        for y in (13..=26).rev() {
            let row_idx = 26 - y;
            if row_idx <= vu_height {
                let vu_c = if row_idx > 10 {
                    Rgb::new(255, 30, 30) // Red peak
                } else if row_idx > 6 {
                    Rgb::new(255, 220, 0) // Amber mid
                } else {
                    Rgb::new(0, 240, 80) // Green base
                };
                frame.set(15, y as usize, vu_c);
                frame.set(16, y as usize, vu_c);
            }
        }

        // 5. Air Pressure Shockwave Ripples
        let show_shockwaves = self.values[2] > 0.5;
        if show_shockwaves && self.wave_radius > 0.0 {
            let wr = self.wave_radius;
            let fade = clamp01(1.0 - (wr / 28.0));
            let ring_byte = to_byte(255.0 * fade);
            let ring_col = Rgb::new(ring_byte, ring_byte, ring_byte);

            for y in 0..HEIGHT {
                let dy = y as f32 - 19.5;
                for x in 0..WIDTH {
                    let dx1 = x as f32 - 9.5;
                    let dx2 = x as f32 - 21.5;
                    let d1 = (dx1 * dx1 + dy * dy).sqrt();
                    let d2 = (dx2 * dx2 + dy * dy).sqrt();

                    if (d1 - wr).abs() < 1.0 || (d2 - wr).abs() < 1.0 {
                        let cur = frame.get(x, y);
                        frame.set(
                            x,
                            y,
                            Rgb::new(
                                cur.r.saturating_add(ring_col.r),
                                cur.g.saturating_add(ring_col.g),
                                cur.b.saturating_add(ring_col.b),
                            ),
                        );
                    }
                }
            }
        }
    }
}
