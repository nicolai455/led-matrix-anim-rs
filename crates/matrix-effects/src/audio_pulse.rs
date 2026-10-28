use crate::util::{audio_bass, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const PALETTES: &[&str] = &["Rainbow", "Electric", "Lava"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "Speed",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 100.0,
        step: 1.0,
        default: 40.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Palette",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 2.0,
        step: 1.0,
        default: 0.0,
        options: PALETTES,
        unit: "",
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
];

#[derive(Clone, Copy, Default)]
struct Ring {
    radius: f32,
    intensity: f32,
    active: bool,
}

pub struct AudioPulseEffect {
    values: [f32; 4],
    rings: [Ring; 8],
    phase: f32,
    core_pulse: f32,
    last_level: f32,
}

impl Default for AudioPulseEffect {
    fn default() -> Self {
        Self {
            values: [40.0, 0.0, 100.0, 50.0],
            rings: [Ring::default(); 8],
            phase: 0.0,
            core_pulse: 0.0,
            last_level: 0.0,
        }
    }
}

impl AudioPulseEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn spawn_ring(&mut self, intensity: f32) {
        let slot = self
            .rings
            .iter()
            .position(|r| !r.active)
            .unwrap_or_else(|| {
                let mut max_idx = 0;
                for i in 1..8 {
                    if self.rings[i].radius > self.rings[max_idx].radius {
                        max_idx = i;
                    }
                }
                max_idx
            });

        self.rings[slot].radius = 0.5;
        self.rings[slot].intensity = intensity;
        self.rings[slot].active = true;
    }

    fn map_color(&self, dist: f32, intensity: f32) -> Rgb {
        let pal = self.values[1] as usize;
        let val = clamp01(intensity);

        if pal == 1 {
            // Electric: cyan, magenta, violet
            let t = clamp01(dist / 22.0);
            let r = to_byte(val * (120.0 + 135.0 * t));
            let g = to_byte(val * (240.0 * (1.0 - t * 0.7)));
            let b = to_byte(val * 255.0);
            Rgb::new(r, g, b)
        } else if pal == 2 {
            // Lava: deep red to fiery orange to bright white center
            if val < 0.35 {
                let r = to_byte((val / 0.35) * 220.0);
                Rgb::new(r, 0, 0)
            } else if val < 0.75 {
                let g = to_byte(((val - 0.35) / 0.40) * 200.0);
                Rgb::new(255, g, 0)
            } else {
                let g = to_byte(200.0 + ((val - 0.75) / 0.25) * 55.0);
                let b = to_byte(((val - 0.75) / 0.25) * 255.0);
                Rgb::new(255, g, b)
            }
        } else {
            // Rainbow
            let hue = (dist * 0.05) - (self.phase * 0.05);
            hsv_to_rgb(hue, 1.0, val)
        }
    }
}

impl Effect for AudioPulseEffect {
    fn id(&self) -> &'static str {
        "audio_pulse"
    }

    fn name(&self) -> &'static str {
        "Audio Pulse"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "palette" => Some(self.values[1]),
            "reactivity" => Some(self.values[2]),
            "decay" => Some(self.values[3]),
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
            "palette" => self.values[1] = value,
            "reactivity" => self.values[2] = value,
            "decay" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.rings = [Ring::default(); 8];
        self.phase = 0.0;
        self.core_pulse = 0.0;
        self.last_level = 0.0;
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let reactivity = self.values[2] / 100.0;
        let speed = self.values[0] * 0.35;
        let decay_rate = 1.0 + (self.values[3] / 100.0) * 3.0;

        let dt = context.delta_seconds.clamp(0.0, 1.0);
        self.phase = (self.phase + speed * dt * 2.5) % (std::f32::consts::TAU * 10.0);

        let has_audio = context.audio.available && (context.audio.level > 0.0001);
        let level = if has_audio {
            context.audio.level * reactivity
        } else {
            0.0
        };
        let bass = if has_audio {
            audio_bass(context.audio) * reactivity
        } else {
            0.0
        };
        let beat = has_audio && context.audio.beat;

        self.core_pulse *= (-dt * decay_rate).exp();
        if beat {
            self.spawn_ring(1.0);
            self.core_pulse = 1.0;
        } else if bass > 0.6 && (bass - self.last_level) > 0.25 {
            self.spawn_ring(0.7);
        }
        self.last_level = level;

        // Advance rings
        for r in self.rings.iter_mut() {
            if !r.active {
                continue;
            }
            r.radius += speed * dt;
            r.intensity *= (-dt * decay_rate * 0.6).exp();
            if r.radius > 25.0 || r.intensity < 0.02 {
                r.active = false;
            }
        }

        let cx = 15.5;
        let cy = 15.5;

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt();

                // Base ambient rolling wave
                let wave = 0.35 + 0.35 * (dist * 0.8 - self.phase).sin();

                // Central pulsing core
                let core = (-dist * 0.45).exp() * (0.2 + 0.8 * (self.core_pulse + level));

                // Expanding shockwaves
                let mut ring_sum = 0.0f32;
                for r in self.rings.iter() {
                    if !r.active {
                        continue;
                    }
                    let dr = dist - r.radius;
                    let ring_w = (-dr * dr * 0.8).exp() * r.intensity;
                    ring_sum += ring_w;
                }

                let total = if has_audio {
                    core + ring_sum * 1.2 + wave * 0.25
                } else {
                    wave * 0.8 + (-dist * 0.3).exp() * 0.3
                };

                let color = self.map_color(dist, total);
                frame.set(x, y, color);
            }
        }
    }
}
