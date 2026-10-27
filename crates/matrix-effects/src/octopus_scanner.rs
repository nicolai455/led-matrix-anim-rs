use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Club Rainbow", "Red & Blue Police", "Gold & Cyan", "UV Rave"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Scan velocity",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 45.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "beam_length",
        label: "Reach distance",
        kind: ParameterKind::Number,
        min: 6.0,
        max: 18.0,
        step: 1.0,
        default: 13.0,
        options: &[],
        unit: "px",
    },
    ParameterDescriptor {
        id: "barrel_wobble",
        label: "Tumble oscillation",
        kind: ParameterKind::Number,
        min: 0.0,
        max: 10.0,
        step: 1.0,
        default: 5.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Beam palette",
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

pub struct OctopusScannerEffect {
    values: [f32; 5],
    rotation: f32,
    direction: f32,
    strobe_pulse: f32,
    color_cycle: f32,
}

impl Default for OctopusScannerEffect {
    fn default() -> Self {
        Self {
            values: [45.0, 13.0, 5.0, 0.0, 1.0],
            rotation: 0.0,
            direction: 1.0,
            strobe_pulse: 0.0,
            color_cycle: 0.0,
        }
    }
}

impl OctopusScannerEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn beam_color(beam_idx: usize, pal: usize, color_cycle: f32) -> Rgb {
        match pal {
            0 => {
                // Club Rainbow (8 distinct saturated colors)
                let h = ((beam_idx as f32 / 8.0) + color_cycle) % 1.0;
                hsv_to_rgb(h, 1.0, 1.0)
            }
            1 => {
                // Red & Blue Police
                #[allow(clippy::manual_is_multiple_of)]
                if beam_idx % 2 == 0 {
                    Rgb::new(255, 15, 30)
                } else {
                    Rgb::new(15, 100, 255)
                }
            }
            2 => {
                // Gold & Cyan
                #[allow(clippy::manual_is_multiple_of)]
                if beam_idx % 2 == 0 {
                    Rgb::new(255, 210, 20)
                } else {
                    Rgb::new(0, 230, 255)
                }
            }
            _ => {
                // UV Rave (Deep Purple, Neon Pink, White, Electric Indigo)
                match beam_idx % 4 {
                    0 => Rgb::new(180, 20, 255),
                    1 => Rgb::new(255, 30, 160),
                    2 => Rgb::new(250, 250, 255),
                    _ => Rgb::new(80, 0, 240),
                }
            }
        }
    }
}

impl Effect for OctopusScannerEffect {
    fn id(&self) -> &'static str {
        "octopus_scanner"
    }

    fn name(&self) -> &'static str {
        "Octopus Scanner"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "beam_length" => Some(self.values[1]),
            "barrel_wobble" => Some(self.values[2]),
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
            "beam_length" => self.values[1] = value,
            "barrel_wobble" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.rotation = 0.0;
        self.direction = 1.0;
        self.strobe_pulse = 0.0;
        self.color_cycle = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        // Dark nightclub fog
        frame.clear(Rgb::new(6, 6, 12));

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[4] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.strobe_pulse = 1.0;
            self.direction = -self.direction; // Direction reversal on drop!
        } else {
            self.strobe_pulse = (self.strobe_pulse - dt * 4.0).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.9);
        self.rotation = (self.rotation + dt * speed * 2.8 * self.direction) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let pal = self.values[3] as usize;
        let max_reach = self.values[1];
        let wobble_amp = self.values[2] * 0.06;

        let cx = 15.5f32;
        let cy = 15.5f32;

        // 8 motorized rotating scanner barrels
        for i in 0..8 {
            let col = Self::beam_color(i, pal, self.color_cycle);

            // Frequency band modulation
            let band_val = if audio_active { context.audio.bands[i * 2] } else { 0.5 };
            let reach = max_reach * (0.65 + band_val * 0.7);

            // Independent barrel tumbling nutation
            let wobble = (self.rotation * 3.0 + (i as f32) * 1.5).sin() * wobble_amp;
            let ang = self.rotation + (i as f32) * (2.0 * PI / 8.0) + wobble;

            let cos_a = ang.cos();
            let sin_a = ang.sin();

            // Trace volumetric ray from center out to beam tip
            let steps = (reach * 2.0).ceil() as usize;
            for s in 2..=steps {
                let dist = s as f32 * 0.5;
                let px = (cx + cos_a * dist).round() as i32;
                let py = (cy + sin_a * dist).round() as i32;

                if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                    let d_norm = dist / reach.max(1.0);
                    let beam_falloff = 1.0 - d_norm.powi(2) * 0.7;

                    let intensity = (beam_falloff * (0.7 + band_val * 0.5) + self.strobe_pulse * 0.35 + mid * 0.15)
                        .clamp(0.0, 1.0);

                    let r_add = to_byte(col.r as f32 * intensity);
                    let g_add = to_byte(col.g as f32 * intensity);
                    let b_add = to_byte(col.b as f32 * intensity);

                    frame.add_rgb(px as usize, py as usize, r_add, g_add, b_add);
                }
            }

            // Beam End Spot (high-intensity illuminated spot on club wall/floor)
            let tip_x = (cx + cos_a * reach).round() as i32;
            let tip_y = (cy + sin_a * reach).round() as i32;

            if tip_x >= 0 && tip_x < WIDTH as i32 && tip_y >= 0 && tip_y < HEIGHT as i32 {
                let spot_b = clamp01(0.9 + band_val * 0.5 + self.strobe_pulse * 0.5);
                frame.add_rgb(
                    tip_x as usize,
                    tip_y as usize,
                    to_byte(col.r as f32 * spot_b),
                    to_byte(col.g as f32 * spot_b),
                    to_byte(col.b as f32 * spot_b),
                );
            }
        }

        // Center Chrome Chandelier Fixture (4x4 hub at cx, cy)
        let hub_rim = Rgb::new(120, 125, 140);
        let hub_core = if beat { Rgb::WHITE } else { Rgb::new(200, 210, 230) };

        for dy in -1..=1i32 {
            for dx in -1..=1i32 {
                let px = (cx + dx as f32).round() as usize;
                let py = (cy + dy as f32).round() as usize;
                if px < WIDTH && py < HEIGHT {
                    let c = if dx == 0 && dy == 0 { hub_core } else { hub_rim };
                    frame.set(px, py, c);
                }
            }
        }

        // Sparkle motes on treble
        if audio_active && treble > 0.4 {
            for i in 0..4 {
                let sx = (i * 9 + 4) % WIDTH;
                let sy = (i * 7 + 3) % HEIGHT;
                frame.set(sx, sy, Rgb::new(255, 255, 150));
            }
        }
    }
}
