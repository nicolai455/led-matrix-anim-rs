use crate::text::{draw_char, text_width, FONT_SPACING, FONT_WIDTH};
use crate::util::{audio_bass, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, WIDTH,
};
use std::f32::consts::PI;

const MODES: &[&str] = &[
    "Rainbow Wave",
    "Bounce Wave",
    "Neon Glow",
    "Matrix Drip",
    "Audio Bop",
];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "mode",
        label: "Text style",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 4.0,
        step: 1.0,
        default: 4.0,
        options: MODES,
        unit: "",
    },
    ParameterDescriptor {
        id: "scroll",
        label: "Scrolling",
        kind: ParameterKind::Boolean,
        min: 0.0,
        max: 1.0,
        step: 1.0,
        default: 1.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "speed",
        label: "Speed",
        kind: ParameterKind::Number,
        min: 5.0,
        max: 100.0,
        step: 1.0,
        default: 30.0,
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

pub struct TextEffectsEffect {
    values: [f32; 4],
    offset_x: f32,
    time_phase: f32,
    glow_phase: f32,
}

impl Default for TextEffectsEffect {
    fn default() -> Self {
        Self {
            values: [4.0, 1.0, 30.0, 1.0],
            offset_x: WIDTH as f32,
            time_phase: 0.0,
            glow_phase: 0.0,
        }
    }
}

impl TextEffectsEffect {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for TextEffectsEffect {
    fn id(&self) -> &'static str {
        "text_effects"
    }

    fn name(&self) -> &'static str {
        "Text Effects"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "mode" => Some(self.values[0]),
            "scroll" => Some(self.values[1]),
            "speed" => Some(self.values[2]),
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
            "mode" => self.values[0] = value,
            "scroll" => self.values[1] = value,
            "speed" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.offset_x = WIDTH as f32;
        self.time_phase = 0.0;
        self.glow_phase = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::BLACK);

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[3] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        let speed = self.values[2];
        self.time_phase = (self.time_phase + dt * (speed / 20.0)) % (2.0 * PI);
        self.glow_phase = (self.glow_phase + dt * 2.5) % (2.0 * PI);

        let text = if context.text.is_empty() {
            "MATRIX"
        } else {
            context.text
        };

        let total_w = text_width(text) as i32;
        let is_scrolling = self.values[1] > 0.5;

        let base_x = if is_scrolling {
            self.offset_x -= speed * dt;
            if self.offset_x < -(total_w as f32) {
                self.offset_x = WIDTH as f32;
            }
            self.offset_x.round() as i32
        } else {
            ((WIDTH as i32 - total_w) / 2).max(0)
        };

        let mode = self.values[0] as usize;
        let base_y = 12i32;

        let mut curr_x = base_x;
        for (char_idx, ch) in text.chars().enumerate() {
            if curr_x + (FONT_WIDTH as i32) > 0 && curr_x < WIDTH as i32 {
                let char_offset = char_idx as f32 * 0.4;

                let (y_pos, color) = match mode {
                    0 => {
                        // Rainbow Wave
                        let h = (self.time_phase / (2.0 * PI) + char_offset * 0.5) % 1.0;
                        let y_bounce = ((self.time_phase * 2.0 + char_offset).sin() * 2.0).round() as i32;
                        (base_y + y_bounce, hsv_to_rgb(h, 1.0, 1.0))
                    }
                    1 => {
                        // Bounce Wave
                        let y_bounce = ((self.time_phase * 3.0 + char_offset * 2.0).sin() * 4.5).round() as i32;
                        let col = if context.color == Rgb::BLACK { Rgb::WHITE } else { context.color };
                        (base_y + y_bounce, col)
                    }
                    2 => {
                        // Neon Glow (Glow border)
                        let pulse = 0.7 + 0.3 * (self.glow_phase).sin();
                        let neon = if context.color == Rgb::BLACK {
                            Rgb::new(to_byte(40.0 * pulse), to_byte(255.0 * pulse), to_byte(230.0 * pulse))
                        } else {
                            Rgb::new(
                                to_byte(context.color.r as f32 * pulse),
                                to_byte(context.color.g as f32 * pulse),
                                to_byte(context.color.b as f32 * pulse),
                            )
                        };
                        // Draw glow aura first
                        let aura = Rgb::new(neon.r / 3, neon.g / 3, neon.b / 3);
                        draw_char(frame, ch, curr_x - 1, base_y, aura);
                        draw_char(frame, ch, curr_x + 1, base_y, aura);
                        draw_char(frame, ch, curr_x, base_y - 1, aura);
                        draw_char(frame, ch, curr_x, base_y + 1, aura);
                        (base_y, neon)
                    }
                    3 => {
                        // Matrix Drip (Green digital rain drip on characters)
                        let drip_h = (char_idx * 7) as f32;
                        let drip_y = (((self.time_phase * 5.0 + drip_h) % 32.0) - 16.0).round() as i32;
                        let y = (base_y + drip_y).clamp(0, 24);
                        let g = 255;
                        let r = 30;
                        let b = 50;
                        (y, Rgb::new(r, g, b))
                    }
                    _ => {
                        // Audio Bop (Scale/bounce with bass & beats)
                        let bop = if beat { 5 } else { (bass * 4.0).round() as i32 };
                        let h = (self.time_phase / (2.0 * PI) + bass * 0.5) % 1.0;
                        (base_y - bop, hsv_to_rgb(h, 0.95, 1.0))
                    }
                };

                draw_char(frame, ch, curr_x, y_pos, color);
            }
            curr_x += (FONT_WIDTH + FONT_SPACING) as i32;
        }
    }
}
