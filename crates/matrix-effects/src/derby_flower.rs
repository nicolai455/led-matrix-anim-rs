use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Classic Derby RGBY", "Red & Blue Strobe", "Cyberpunk Violet", "Rainbow Flower"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Tumble speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 45.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "clusters",
        label: "Flower clusters",
        kind: ParameterKind::Number,
        min: 2.0,
        max: 6.0,
        step: 1.0,
        default: 4.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "spread",
        label: "Spot radius spread",
        kind: ParameterKind::Number,
        min: 6.0,
        max: 16.0,
        step: 1.0,
        default: 11.0,
        options: &[],
        unit: "px",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Color array",
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

pub struct DerbyFlowerEffect {
    values: [f32; 5],
    tumble_a: f32,
    tumble_b: f32,
    strobe_pulse: f32,
    direction: f32,
    previous_beat: bool,
    color_cycle: f32,
}

impl Default for DerbyFlowerEffect {
    fn default() -> Self {
        Self {
            values: [45.0, 4.0, 11.0, 0.0, 1.0],
            tumble_a: 0.0,
            tumble_b: 0.0,
            strobe_pulse: 0.0,
            direction: 1.0,
            previous_beat: false,
            color_cycle: 0.0,
        }
    }
}

impl DerbyFlowerEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn derby_color(spot_idx: usize, pal: usize, color_cycle: f32) -> Rgb {
        match pal {
            0 => {
                // Classic Derby RGBY (Red, Green, Blue, Yellow, White, Amber)
                match spot_idx % 6 {
                    0 => Rgb::new(255, 20, 30),
                    1 => Rgb::new(20, 240, 50),
                    2 => Rgb::new(20, 110, 255),
                    3 => Rgb::new(255, 230, 20),
                    4 => Rgb::WHITE,
                    _ => Rgb::new(255, 120, 10),
                }
            }
            1 => {
                // Red & Blue Strobe
                #[allow(clippy::manual_is_multiple_of)]
                if spot_idx % 2 == 0 {
                    Rgb::new(255, 15, 30)
                } else {
                    Rgb::new(15, 90, 255)
                }
            }
            2 => {
                // Cyberpunk Violet (Hot Magenta, Electric Cyan, Neon Lime)
                match spot_idx % 3 {
                    0 => Rgb::new(255, 20, 180),
                    1 => Rgb::new(0, 240, 255),
                    _ => Rgb::new(60, 255, 70),
                }
            }
            _ => {
                // Rainbow Flower
                let h = ((spot_idx as f32 / 12.0) + color_cycle) % 1.0;
                hsv_to_rgb(h, 0.95, 1.0)
            }
        }
    }
}

impl Effect for DerbyFlowerEffect {
    fn id(&self) -> &'static str {
        "derby_flower"
    }

    fn name(&self) -> &'static str {
        "Derby Gobo Flower"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "clusters" => Some(self.values[1]),
            "spread" => Some(self.values[2]),
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
            "clusters" => self.values[1] = value,
            "spread" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.tumble_a = 0.0;
        self.tumble_b = 0.0;
        self.strobe_pulse = 0.0;
        self.direction = 1.0;
        self.previous_beat = false;
        self.color_cycle = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::BLACK); // Dark haze background

        // Integrate the bounded interval once, then render the final state.
        let dt = if context.delta_seconds.is_finite() {
            context.delta_seconds.clamp(0.0, 0.5)
        } else {
            0.0
        };
        let audio_active = self.values[4] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.strobe_pulse = 1.0;
            if !self.previous_beat {
                self.direction = -self.direction;
            }
        } else {
            self.strobe_pulse = (self.strobe_pulse - dt * 4.0).max(0.0);
        }

        self.previous_beat = beat;

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.9);
        self.tumble_a = (self.tumble_a + dt * speed * 2.5 * self.direction) % (2.0 * PI);
        self.tumble_b = (self.tumble_b - dt * speed * 2.1 * self.direction) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.16) % 1.0;

        let pal = self.values[3] as usize;
        let num_clusters = self.values[1].round() as usize;
        let base_spread = self.values[2] + bass * 3.5;

        let cx = 15.5f32;
        let cy = 15.5f32;

        // Generate multiple tumbling clusters of gobo dots
        let mut spot_counter = 0usize;

        for c_idx in 0..num_clusters {
            let is_inner_wheel = c_idx % 2 == 0;
            let tumble = if is_inner_wheel { self.tumble_a } else { self.tumble_b };

            // Cluster center orbits around the display
            let cluster_dist = if is_inner_wheel {
                base_spread * 0.45
            } else {
                base_spread * 0.85
            };
            let cluster_ang = tumble + (c_idx as f32) * (2.0 * PI / num_clusters as f32);

            let cluster_cx = cx + cluster_ang.cos() * cluster_dist;
            let cluster_cy = cy + cluster_ang.sin() * cluster_dist;

            // 6 gobo petal dots per cluster
            for dot_idx in 0..6 {
                let dot_ang = (dot_idx as f32) * (2.0 * PI / 6.0) - tumble * 2.0;
                let dot_dist = 2.5 + (if dot_idx % 2 == 0 { 0.8 } else { -0.8 }) + mid * 1.5;

                let spot_x = cluster_cx + dot_ang.cos() * dot_dist;
                let spot_y = cluster_cy + dot_ang.sin() * dot_dist;

                let col = Self::derby_color(spot_counter, pal, self.color_cycle);
                spot_counter += 1;

                // Render circular gobo spot with soft edge
                let sx_i = spot_x.round() as i32;
                let sy_i = spot_y.round() as i32;

                for dy in -1..=1i32 {
                    for dx in -1..=1i32 {
                        let px = sx_i + dx;
                        let py = sy_i + dy;
                        if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                            let d2 = (dx * dx + dy * dy) as f32;
                            if d2 <= 2.0 {
                                let edge = 1.0 - (d2 / 2.0) * 0.5;
                                let is_center_dot = dx == 0 && dy == 0;
                                let hot = if is_center_dot && self.strobe_pulse > 0.2 { 0.5 } else { 0.0 };

                                let intensity = clamp01(edge * (0.85 + bass * 0.35) + hot);

                                let r_add = to_byte(col.r as f32 * intensity);
                                let g_add = to_byte(col.g as f32 * intensity);
                                let b_add = to_byte(col.b as f32 * intensity);

                                frame.add_rgb(px as usize, py as usize, r_add, g_add, b_add);
                            }
                        }
                    }
                }
            }
        }

        // Treble sparkles on high frequencies
        if audio_active && treble > 0.4 {
            for i in 0..4 {
                let sx = (i * 9 + 4) % WIDTH;
                let sy = (i * 7 + 2) % HEIGHT;
                frame.set(sx, sy, Rgb::WHITE);
            }
        }

        // Strobe blast flash
        if self.strobe_pulse > 0.3 {
            let blast = to_byte(self.strobe_pulse * 120.0);
            for x in (0..WIDTH).step_by(3) {
                frame.set(x, 0, Rgb::new(blast, blast, blast));
                frame.set(x, 31, Rgb::new(blast, blast, blast));
            }
        }
    }
}

#[cfg(test)]
mod beat_regression {
    use super::*;
    use matrix_core::AudioSnapshot;

    #[test]
    fn held_beat_reverses_once_and_rearms() {
        let mut effect = DerbyFlowerEffect::default();
        let mut frame = Frame::default();
        let mut audio = AudioSnapshot::default();
        audio.available = true;
        audio.beat = true;
        let render = |effect: &mut DerbyFlowerEffect, frame: &mut Frame, audio: &AudioSnapshot| {
            effect.render(frame, &RenderContext { now_ms: 0, delta_seconds: 0.016, audio, color: Rgb::WHITE, text: "", scroll_speed: 1.0 });
        };
        render(&mut effect, &mut frame, &audio);
        assert_eq!(effect.direction, -1.0);
        render(&mut effect, &mut frame, &audio);
        assert_eq!(effect.direction, -1.0);
        audio.beat = false;
        render(&mut effect, &mut frame, &audio);
        audio.beat = true;
        render(&mut effect, &mut frame, &audio);
        assert_eq!(effect.direction, 1.0);
    }
}
