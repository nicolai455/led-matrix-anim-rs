use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PATTERNS: &[&str] = &["Beat Drop Blast", "Stage Sweep", "Cross Chaser", "Audio Matrix"];
const PALETTES: &[&str] = &["Warm Halogen", "Atomic Cool", "Neon Purple", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Chase tempo",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 40.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "pattern",
        label: "Blinder chase",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 3.0,
        step: 1.0,
        default: 0.0,
        options: PATTERNS,
        unit: "",
    },
    ParameterDescriptor {
        id: "halogen_decay",
        label: "Filament warmth",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 90.0,
        step: 1.0,
        default: 65.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Color temperature",
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

pub struct StrobeBlinderEffect {
    values: [f32; 5],
    lamp_heat: Box<[[f32; 4]; 4]>, // 4x4 array of stadium blinder lamps
    phase: f32,
    color_cycle: f32,
    flash_all: f32,
}

impl Default for StrobeBlinderEffect {
    fn default() -> Self {
        Self {
            values: [40.0, 0.0, 65.0, 0.0, 1.0],
            lamp_heat: Box::new([[0.0; 4]; 4]),
            phase: 0.0,
            color_cycle: 0.0,
            flash_all: 0.0,
        }
    }
}

impl StrobeBlinderEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn blinder_color(pal: usize, heat: f32, color_cycle: f32, lamp_x: usize, lamp_y: usize) -> Rgb {
        let h = clamp01(heat);
        match pal {
            0 => {
                // Warm Halogen: Black -> Deep Crimson -> Golden Amber -> Blinding White Hot
                if h > 0.85 {
                    let white_mix = (h - 0.85) / 0.15;
                    Rgb::new(
                        255,
                        to_byte(210.0 + 45.0 * white_mix),
                        to_byte(90.0 + 165.0 * white_mix),
                    )
                } else if h > 0.4 {
                    let yellow_mix = (h - 0.4) / 0.45;
                    Rgb::new(255, to_byte(50.0 + 160.0 * yellow_mix), 10)
                } else {
                    let red_mix = h / 0.4;
                    Rgb::new(to_byte(255.0 * red_mix), 0, 0)
                }
            }
            1 => {
                // Atomic Cool: Deep Navy -> Electric Cyan -> Bright Arc White
                if h > 0.8 {
                    let w = (h - 0.8) / 0.2;
                    Rgb::new(to_byte(150.0 + 105.0 * w), 255, 255)
                } else {
                    Rgb::new(0, to_byte(240.0 * h), to_byte(255.0 * h))
                }
            }
            2 => {
                // Neon Purple: Deep Indigo -> Hot Magenta -> Pure Optical White
                if h > 0.8 {
                    let w = (h - 0.8) / 0.2;
                    Rgb::new(255, to_byte(100.0 + 155.0 * w), 255)
                } else {
                    Rgb::new(to_byte(255.0 * h), 0, to_byte(220.0 * h))
                }
            }
            _ => {
                // Rainbow
                let lamp_norm = (lamp_x * 4 + lamp_y) as f32 / 16.0;
                let hue = (color_cycle + lamp_norm * 0.5) % 1.0;
                hsv_to_rgb(hue, 1.0 - h * 0.5, h)
            }
        }
    }
}

impl Effect for StrobeBlinderEffect {
    fn id(&self) -> &'static str {
        "strobe_blinder"
    }

    fn name(&self) -> &'static str {
        "Club Strobe Blinders"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "pattern" => Some(self.values[1]),
            "halogen_decay" => Some(self.values[2]),
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
            "pattern" => self.values[1] = value,
            "halogen_decay" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.lamp_heat = Box::new([[0.0; 4]; 4]);
        self.phase = 0.0;
        self.color_cycle = 0.0;
        self.flash_all = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[4] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let _mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let _treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        // Beat triggers all blinders 100% white hot!
        if beat {
            self.flash_all = 1.0;
            for ly in 0..4 {
                for lx in 0..4 {
                    self.lamp_heat[ly][lx] = 1.0;
                }
            }
        } else {
            self.flash_all = (self.flash_all - dt * 3.5).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.8);
        self.phase = (self.phase + dt * speed * 3.0) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let pal = self.values[3] as usize;
        let pattern_mode = self.values[1] as usize;

        // Natural halogen thermal cooling rate
        let cool_rate = (100.0 - self.values[2]).clamp(10.0, 90.0) * 0.08;

        for ly in 0..4 {
            for lx in 0..4 {
                let heat = &mut self.lamp_heat[ly][lx];
                // Decay
                *heat = (*heat - dt * cool_rate).max(0.0);

                let trigger = match pattern_mode {
                    0 => {
                        // Beat Drop Blast (Pulsing ring outward from center)
                        let dist = ((lx as f32 - 1.5).powi(2) + (ly as f32 - 1.5).powi(2)).sqrt();
                        let pulse = ((dist * 2.5 - self.phase * 2.0).sin() * 0.5 + 0.5).powf(3.0);
                        pulse > 0.65
                    }
                    1 => {
                        // Stage Sweep (Column chaser)
                        let col_phase = (self.phase * 2.0).floor() as usize % 4;
                        lx == col_phase
                    }
                    2 => {
                        // Cross Chaser (X-pattern alternating)
                        let is_diag1 = lx == ly;
                        let is_diag2 = (lx + ly) == 3;
                        let alt = (self.phase * 1.5).sin() > 0.0;
                        if alt { is_diag1 } else { is_diag2 }
                    }
                    _ => {
                        // Audio Matrix (Mapped to 16 bands)
                        let band_idx = ly * 4 + lx;
                        let val = if audio_active {
                            context.audio.bands[band_idx]
                        } else {
                            ((self.phase + band_idx as f32 * 0.4).sin() * 0.5 + 0.5).powf(2.0)
                        };
                        val > 0.6
                    }
                };

                if trigger {
                    *heat = (*heat + dt * 15.0).min(1.0);
                }
            }
        }

        // Render 4x4 Blinder Lamps onto 32x32 Display (Each lamp is 6x6 pixels with 2px borders)
        const LAMP_SIZE: usize = 6;
        const STRIDE: usize = 8; // 6px lamp + 2px border

        for ly in 0..4 {
            for lx in 0..4 {
                let heat = self.lamp_heat[ly][lx];
                let color = Self::blinder_color(pal, heat, self.color_cycle, lx, ly);

                let start_x = lx * STRIDE + 1;
                let start_y = ly * STRIDE + 1;

                // Circular reflector shape for each blinder lamp
                for py in 0..LAMP_SIZE {
                    let y = start_y + py;
                    let dy = py as f32 - 2.5;
                    for px in 0..LAMP_SIZE {
                        let x = start_x + px;
                        let dx = px as f32 - 2.5;
                        let d2 = dx * dx + dy * dy;

                        if d2 <= 7.0 {
                            // Lamp center is hotter / brighter
                            let center_boost = if d2 <= 2.0 { 1.25 } else { 0.9 };
                            let r = to_byte(color.r as f32 * center_boost);
                            let g = to_byte(color.g as f32 * center_boost);
                            let b = to_byte(color.b as f32 * center_boost);
                            frame.set(x, y, Rgb::new(r, g, b));
                        } else {
                            // Dark metal bezel
                            frame.set(x, y, Rgb::new(12, 12, 16));
                        }
                    }
                }
            }
        }

        // Full blinder whiteout flash on sudden beat
        if self.flash_all > 0.5 {
            let f_byte = to_byte(self.flash_all * 180.0);
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let cur = frame.get(x, y);
                    frame.set(
                        x,
                        y,
                        Rgb::new(
                            cur.r.saturating_add(f_byte),
                            cur.g.saturating_add(f_byte),
                            cur.b.saturating_add(f_byte),
                        ),
                    );
                }
            }
        }
    }
}
