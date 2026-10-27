use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const CHASE_MODES: &[&str] = &["Outward Cannon Burst", "Inward Implosion", "Dual Alternating", "Audio Pulse Wave"];
const PALETTES: &[&str] = &["Warm Halogen", "RGB Prism", "Red & Blue Police", "Cyberpunk Neon"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Chase tempo",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 45.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "mode",
        label: "Chase pattern",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 3.0,
        step: 1.0,
        default: 0.0,
        options: CHASE_MODES,
        unit: "",
    },
    ParameterDescriptor {
        id: "filament_decay",
        label: "Filament warmth decay",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 90.0,
        step: 1.0,
        default: 60.0,
        options: &[],
        unit: "%",
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

pub struct ChaserCannonsEffect {
    values: [f32; 5],
    phase: f32,
    ring_energy: [f32; 6],
    strobe_pulse: f32,
    color_cycle: f32,
}

impl Default for ChaserCannonsEffect {
    fn default() -> Self {
        Self {
            values: [45.0, 0.0, 60.0, 0.0, 1.0],
            phase: 0.0,
            ring_energy: [0.0; 6],
            strobe_pulse: 0.0,
            color_cycle: 0.0,
        }
    }
}

impl ChaserCannonsEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn blinder_color(pal: usize, ring_idx: usize, intensity: f32, color_cycle: f32) -> Rgb {
        let b = clamp01(intensity);
        match pal {
            0 => {
                // Warm Halogen: White -> Gold -> Deep Amber -> Red as it cools
                let r = to_byte(255.0 * b.sqrt());
                let g = to_byte(220.0 * b.powi(2) * 0.85);
                let blue = to_byte(50.0 * b.powi(4));
                Rgb::new(r, g, blue)
            }
            1 => {
                // RGB Prism (Each ring has its own rainbow hue)
                let h = ((ring_idx as f32 / 6.0) + color_cycle) % 1.0;
                hsv_to_rgb(h, 0.95, b)
            }
            2 => {
                // Red & Blue Police
                #[allow(clippy::manual_is_multiple_of)]
                let is_red = ring_idx % 2 == 0;
                if is_red {
                    Rgb::new(to_byte(255.0 * b), to_byte(20.0 * b), to_byte(30.0 * b))
                } else {
                    Rgb::new(to_byte(15.0 * b), to_byte(100.0 * b), to_byte(255.0 * b))
                }
            }
            _ => {
                // Cyberpunk Neon (Hot Pink / Electric Cyan)
                #[allow(clippy::manual_is_multiple_of)]
                let is_pink = ring_idx % 2 == 0;
                if is_pink {
                    Rgb::new(to_byte(255.0 * b), to_byte(10.0 * b), to_byte(180.0 * b))
                } else {
                    Rgb::new(0, to_byte(240.0 * b), to_byte(255.0 * b))
                }
            }
        }
    }
}

impl Effect for ChaserCannonsEffect {
    fn id(&self) -> &'static str {
        "chaser_cannons"
    }

    fn name(&self) -> &'static str {
        "Chaser Cannons"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "mode" => Some(self.values[1]),
            "filament_decay" => Some(self.values[2]),
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
            "mode" => self.values[1] = value,
            "filament_decay" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.ring_energy = [0.0; 6];
        self.strobe_pulse = 0.0;
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
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.strobe_pulse = 1.0;
        } else {
            self.strobe_pulse = (self.strobe_pulse - dt * 4.0).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.9);
        self.phase = (self.phase + dt * speed * 4.0) % 6.0;
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let mode = self.values[1] as usize;
        let pal = self.values[3] as usize;
        let decay_mult = 1.0 - (self.values[2] / 100.0) * 0.8;

        // Update target energy for each ring
        for r_idx in 0..6 {
            let target_energy = match mode {
                0 => {
                    // Outward Cannon Burst: 0 -> 1 -> 2 -> 3 -> 4 -> 5
                    let diff = (self.phase - r_idx as f32).abs();
                    if diff < 0.6 { 1.0 - (diff / 0.6) } else { 0.0 }
                }
                1 => {
                    // Inward Implosion: 5 -> 4 -> 3 -> 2 -> 1 -> 0
                    let inv_phase = 5.0 - self.phase;
                    let diff = (inv_phase - r_idx as f32).abs();
                    if diff < 0.6 { 1.0 - (diff / 0.6) } else { 0.0 }
                }
                2 => {
                    // Dual Alternating: Even rings fire, then Odd rings fire
                    #[allow(clippy::manual_is_multiple_of)]
                    let is_even_cycle = (self.phase.floor() as usize) % 2 == 0;
                    #[allow(clippy::manual_is_multiple_of)]
                    let is_even_ring = r_idx % 2 == 0;
                    if is_even_cycle == is_even_ring { 1.0 } else { 0.0 }
                }
                _ => {
                    // Audio Pulse Wave
                    let band_val = if audio_active { context.audio.bands[r_idx * 2] } else { 0.0 };
                    (band_val * 1.5 + bass * 0.6).min(1.0)
                }
            };

            // Fast attack, filament halogen decay
            if target_energy > self.ring_energy[r_idx] {
                self.ring_energy[r_idx] = target_energy;
            } else {
                self.ring_energy[r_idx] = (self.ring_energy[r_idx] - dt * 8.0 * decay_mult).max(0.0);
            }

            if beat && (r_idx == 0 || r_idx == 5) {
                self.ring_energy[r_idx] = 1.0;
            }
        }

        let cx = 15.5f32;
        let cy = 15.5f32;
        let ring_radii = [2.2f32, 4.8f32, 7.5f32, 10.2f32, 12.8f32, 15.2f32];

        // Draw concentric chasing blinder rings
        for y in 0..HEIGHT {
            let dy = y as f32 - cy;
            for x in 0..WIDTH {
                let dx = x as f32 - cx;
                let r = (dx * dx + dy * dy).sqrt();

                // Find closest ring
                for (r_idx, &rad) in ring_radii.iter().enumerate() {
                    let diff = (r - rad).abs();
                    if diff < 1.1 {
                        let energy = self.ring_energy[r_idx];
                        if energy > 0.04 {
                            let ring_falloff = 1.0 - (diff / 1.1).powi(2);
                            // Angle-based LED discretization (individual lamps in ring)
                            let num_lamps = (r_idx + 1) * 6;
                            let ang = dy.atan2(dx) + PI;
                            let lamp_phase = (ang * num_lamps as f32 / (2.0 * PI)) % 1.0;
                            let lamp_cut = (lamp_phase - 0.5).abs();
                            let in_lamp = lamp_cut < 0.38;

                            if in_lamp {
                                let lamp_bright = (1.0 - lamp_cut / 0.4) * ring_falloff * energy;
                                let b = (lamp_bright + self.strobe_pulse * 0.25 + mid * 0.1).clamp(0.0, 1.0);
                                let col = Self::blinder_color(pal, r_idx, b, self.color_cycle);

                                let cur = frame.get(x, y);
                                frame.set(
                                    x,
                                    y,
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
            }
        }

        // Center Blinder Lamp Core at (15..16, 15..16)
        let core_energy = self.ring_energy[0] + self.strobe_pulse * 0.4;
        if core_energy > 0.1 {
            let c = Self::blinder_color(pal, 0, core_energy, self.color_cycle);
            frame.set(15, 15, c);
            frame.set(16, 15, c);
            frame.set(15, 16, c);
            frame.set(16, 16, c);
        }

        // Treble sparks on perimeter
        if audio_active && treble > 0.4 {
            for i in 0..4 {
                let sx = (i * 9 + 4) % WIDTH;
                let sy = (i * 7 + 3) % HEIGHT;
                frame.set(sx, sy, Rgb::WHITE);
            }
        }
    }
}
