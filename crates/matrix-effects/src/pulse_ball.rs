use crate::util::{audio_bass, clamp01, hsv_to_rgb};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, HEIGHT, WIDTH,
};

const SOURCES: &[&str] = &["Microphone", "Demo"];
const COLOR_MODES: &[&str] = &["Rainbow", "Solid", "White"];

const PARAMETERS: [ParameterDescriptor; 7] = [
    ParameterDescriptor {
        id: "source",
        label: "Pulse source",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 1.0,
        step: 1.0,
        default: 0.0,
        options: SOURCES,
        unit: "",
    },
    ParameterDescriptor {
        id: "color_mode",
        label: "Color mode",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 2.0,
        step: 1.0,
        default: 0.0,
        options: COLOR_MODES,
        unit: "",
    },
    ParameterDescriptor {
        id: "hue",
        label: "Solid hue",
        kind: ParameterKind::Number,
        min: 0.0,
        max: 360.0,
        step: 1.0,
        default: 0.0,
        options: &[],
        unit: "degrees",
    },
    ParameterDescriptor {
        id: "color_speed",
        label: "Rainbow speed",
        kind: ParameterKind::Number,
        min: 0.0,
        max: 100.0,
        step: 1.0,
        default: 25.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "reactivity",
        label: "Reactivity",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 200.0,
        step: 1.0,
        default: 100.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "decay",
        label: "Decay",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 50.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "bpm",
        label: "Demo tempo",
        kind: ParameterKind::Number,
        min: 30.0,
        max: 240.0,
        step: 1.0,
        default: 120.0,
        options: &[],
        unit: "BPM",
    },
];

pub struct PulseBallEffect {
    values: [f32; 7],
    pulse: f32,
    hue: f32,
    demo_phase: f32,
}

impl Default for PulseBallEffect {
    fn default() -> Self {
        Self {
            values: [0.0, 0.0, 0.0, 25.0, 100.0, 50.0, 120.0],
            pulse: 0.0,
            hue: 0.0,
            demo_phase: 0.0,
        }
    }
}

impl PulseBallEffect {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for PulseBallEffect {
    fn id(&self) -> &'static str {
        "pulse_ball"
    }

    fn name(&self) -> &'static str {
        "Pulse Ball"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "source" => Some(self.values[0]),
            "color_mode" => Some(self.values[1]),
            "hue" => Some(self.values[2]),
            "color_speed" => Some(self.values[3]),
            "reactivity" => Some(self.values[4]),
            "decay" => Some(self.values[5]),
            "bpm" => Some(self.values[6]),
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
            "source" => {
                if self.values[0] != value {
                    self.pulse = 0.0;
                    self.demo_phase = 0.0;
                }
                self.values[0] = value;
            }
            "color_mode" => self.values[1] = value,
            "hue" => self.values[2] = value,
            "color_speed" => self.values[3] = value,
            "reactivity" => self.values[4] = value,
            "decay" => self.values[5] = value,
            "bpm" => self.values[6] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.pulse = 0.0;
        self.hue = 0.0;
        self.demo_phase = 0.0;
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let dt = context.delta_seconds.clamp(0.0, 1.0);
        let demo = self.values[0] == 1.0;
        let amount = self.values[4] / 100.0;

        let mut beat = false;
        if demo {
            let bpm = self.values[6].max(1.0);
            let beats_per_sec = bpm / 60.0;
            self.demo_phase += dt * beats_per_sec;
            if self.demo_phase >= 1.0 {
                self.demo_phase -= self.demo_phase.floor();
                beat = true;
            }
        } else if context.audio.available && context.audio.beat {
            beat = true;
        }

        let drive = if !demo && context.audio.available {
            let bass = audio_bass(context.audio);
            let level = clamp01(context.audio.level);
            (bass * 0.8 + level * 0.2) * amount
        } else {
            0.0
        };

        let release_sec = (80.0 + self.values[5] * 7.0) / 1000.0;
        self.pulse *= (-dt / release_sec).exp();
        self.pulse = self.pulse.max(drive);
        if beat {
            self.pulse = self.pulse.max(amount.clamp(0.0, 1.0));
        }

        self.hue = (self.hue + dt * (self.values[3] / 100.0) * 0.5) % 1.0;

        let radius = 4.0 + 10.0 * self.pulse;
        let hue = if self.values[1] == 0.0 {
            self.hue
        } else {
            self.values[2] / 360.0
        };
        let saturation = if self.values[1] == 2.0 { 0.0 } else { 1.0 };

        frame.clear(matrix_core::Rgb::BLACK);

        let cx = 15.5;
        let cy = 15.5;

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let distance = (dx * dx + dy * dy).sqrt();

                let coverage = clamp01(radius + 0.5 - distance);
                if coverage <= 0.0 {
                    continue;
                }

                let depth = clamp01(1.0 - (distance * distance) / (radius * radius)).sqrt();
                let brightness = coverage * (0.3 + 0.7 * depth) * (0.65 + 0.35 * self.pulse);

                frame.set(x, y, hsv_to_rgb(hue, saturation, brightness));
            }
        }
    }
}
