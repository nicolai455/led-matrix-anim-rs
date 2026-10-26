use crate::util::{clamp01, hsv_to_rgb, to_byte, XorShift32};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const MODES: &[&str] = &["Hybrid", "Beat Sync", "Fixed Rate"];
const COLORS: &[&str] = &["White", "Rainbow", "Police", "Random"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Flash rate",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 30.0,
        step: 1.0,
        default: 10.0,
        options: &[],
        unit: "Hz",
    },
    ParameterDescriptor {
        id: "mode",
        label: "Mode",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 2.0,
        step: 1.0,
        default: 0.0,
        options: MODES,
        unit: "",
    },
    ParameterDescriptor {
        id: "color",
        label: "Color",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 3.0,
        step: 1.0,
        default: 0.0,
        options: COLORS,
        unit: "",
    },
    ParameterDescriptor {
        id: "duration",
        label: "Flash duration",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 80.0,
        step: 1.0,
        default: 25.0,
        options: &[],
        unit: "ms",
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

pub struct StroboscopeEffect {
    values: [f32; 5],
    timer_ms: f32,
    standby_timer_ms: f32,
    flash_age_ms: f32,
    flash_peak: f32,
    flash_hue: f32,
    random_hue: f32,
    police_left_red: bool,
    rng: XorShift32,
}

impl Default for StroboscopeEffect {
    fn default() -> Self {
        Self {
            values: [10.0, 0.0, 0.0, 25.0, 1.0],
            timer_ms: 0.0,
            standby_timer_ms: 0.0,
            flash_age_ms: 99999.0,
            flash_peak: 0.0,
            flash_hue: 0.0,
            random_hue: 0.0,
            police_left_red: false,
            rng: XorShift32::new(0x1337BEEF),
        }
    }
}

impl StroboscopeEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn trigger_flash(&mut self, boost: f32) {
        self.flash_age_ms = 0.0;
        self.flash_peak = 1.0 + boost;
        self.flash_hue = (self.flash_hue + 0.15) % 1.0;
        self.police_left_red = !self.police_left_red;
        self.random_hue = self.rng.next_f32();
    }
}

impl Effect for StroboscopeEffect {
    fn id(&self) -> &'static str {
        "stroboscope"
    }

    fn name(&self) -> &'static str {
        "Stroboscope"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "mode" => Some(self.values[1]),
            "color" => Some(self.values[2]),
            "duration" => Some(self.values[3]),
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
            "color" => self.values[2] = value,
            "duration" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.timer_ms = 0.0;
        self.standby_timer_ms = 0.0;
        self.flash_age_ms = 99999.0;
        self.flash_peak = 0.0;
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let dt_ms = (context.delta_seconds * 1000.0).clamp(0.0, 1000.0);
        let speed = self.values[0].max(1.0);
        let mode = self.values[1] as usize;
        let interval_ms = 1000.0 / speed;

        self.timer_ms += dt_ms;
        self.standby_timer_ms += dt_ms;
        self.flash_age_ms += dt_ms;

        let audio_on = self.values[4] != 0.0;
        let mut audio_boost = 0.0;
        let beat = if audio_on && context.audio.available {
            audio_boost = clamp01(context.audio.level) * 0.5;
            context.audio.beat
        } else {
            false
        };

        let mut should_flash = false;
        if mode == 2 {
            // Fixed rate
            if self.timer_ms >= interval_ms {
                self.timer_ms %= interval_ms;
                should_flash = true;
            }
        } else if mode == 1 {
            // Beat Sync
            if beat {
                should_flash = true;
                self.timer_ms = 0.0;
                self.standby_timer_ms = 0.0;
            } else if self.standby_timer_ms >= 1000.0 {
                self.standby_timer_ms %= 1000.0;
                should_flash = true;
            }
        } else {
            // Hybrid
            if beat {
                should_flash = true;
                self.timer_ms = 0.0;
                self.standby_timer_ms = 0.0;
            } else if self.timer_ms >= interval_ms {
                self.timer_ms %= interval_ms;
                should_flash = true;
            }
        }

        if should_flash {
            self.trigger_flash(audio_boost);
        }

        let duration = self.values[3].max(1.0);
        if self.flash_age_ms >= duration {
            frame.clear(Rgb::BLACK);
            return;
        }

        let decay = 1.0 - (self.flash_age_ms / duration);
        let brightness = clamp01(self.flash_peak * decay);
        let bri = to_byte(brightness * 255.0);

        let color_mode = self.values[2] as usize;
        if color_mode == 1 {
            // Rainbow
            let col = hsv_to_rgb(self.flash_hue, 1.0, brightness);
            frame.clear(col);
        } else if color_mode == 2 {
            // Police: left/right alternating
            let left_col = if self.police_left_red {
                Rgb::new(bri, 0, 0)
            } else {
                Rgb::new(0, 0, bri)
            };
            let right_col = if self.police_left_red {
                Rgb::new(0, 0, bri)
            } else {
                Rgb::new(bri, 0, 0)
            };
            for y in 0..HEIGHT {
                for x in 0..(WIDTH / 2) {
                    frame.set(x, y, left_col);
                }
                for x in (WIDTH / 2)..WIDTH {
                    frame.set(x, y, right_col);
                }
            }
        } else if color_mode == 3 {
            // Random
            let col = hsv_to_rgb(self.random_hue, 1.0, brightness);
            frame.clear(col);
        } else {
            // White
            frame.clear(Rgb::new(bri, bri, bri));
        }
    }
}
