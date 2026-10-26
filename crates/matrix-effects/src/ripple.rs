use crate::util::{audio_bass, clamp01, to_byte, XorShift32};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Ocean", "Sunset", "Neon"];

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
        id: "ripple_count",
        label: "Ripples",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 8.0,
        step: 1.0,
        default: 3.0,
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
        id: "audio_reactive",
        label: "Audio reactive",
        kind: ParameterKind::Boolean,
        min: 0.0,
        max: 1.0,
        step: 1.0,
        default: 0.0,
        options: &[],
        unit: "",
    },
];

#[derive(Clone, Copy)]
struct Ripple {
    cx: f32,
    cy: f32,
    radius: f32,
    max_radius: f32,
    active: bool,
}

pub struct RippleEffect {
    values: [f32; 4],
    ripples: [Ripple; 8],
    rng: XorShift32,
    spawn_timer: f32,
}

impl Default for RippleEffect {
    fn default() -> Self {
        Self {
            values: [40.0, 3.0, 0.0, 0.0],
            ripples: [Ripple {
                cx: 15.5,
                cy: 15.5,
                radius: 0.0,
                max_radius: 20.0,
                active: false,
            }; 8],
            rng: XorShift32::new(0x1337BEEF),
            spawn_timer: 0.35,
        }
    }
}

impl RippleEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn find_inactive_slot(&self) -> Option<usize> {
        self.ripples.iter().position(|r| !r.active)
    }

    fn spawn_ripple(&mut self, cx: f32, cy: f32) {
        let slot = if let Some(s) = self.find_inactive_slot() {
            s
        } else {
            // Find slot with largest radius
            let mut best = 0;
            for i in 1..8 {
                if self.ripples[i].radius > self.ripples[best].radius {
                    best = i;
                }
            }
            best
        };

        self.ripples[slot].cx = cx;
        self.ripples[slot].cy = cy;
        self.ripples[slot].radius = 0.0;
        self.ripples[slot].max_radius = 20.0 + self.rng.next_f32() * 12.0;
        self.ripples[slot].active = true;
    }

    fn color_from_palette(&self, mut t: f32, alpha: f32) -> (f32, f32, f32) {
        t = t - t.floor();
        let pal = self.values[2] as usize;
        let two_pi = 2.0 * PI;
        let (fr, fg, fb) = if pal == 1 {
            // Sunset
            (
                255.0 * (0.7 + 0.3 * (t * two_pi).sin()),
                255.0 * (0.2 + 0.3 * (t * two_pi + 1.2).sin()),
                255.0 * (0.1 + 0.4 * (t * two_pi + 2.5).sin()),
            )
        } else if pal == 2 {
            // Neon
            (
                255.0 * (0.4 + 0.6 * (t * two_pi).sin()),
                255.0 * (0.4 + 0.6 * (t * two_pi + 2.094).sin()),
                255.0 * (0.4 + 0.6 * (t * two_pi + 4.189).sin()),
            )
        } else {
            // Ocean
            (
                255.0 * (0.05 + 0.2 * (t * two_pi).sin()),
                255.0 * (0.2 + 0.4 * (t * two_pi + 1.0).sin()),
                255.0 * (0.5 + 0.5 * (t * two_pi + 0.5).sin()),
            )
        };
        (fr * alpha, fg * alpha, fb * alpha)
    }
}

impl Effect for RippleEffect {
    fn id(&self) -> &'static str {
        "ripple"
    }

    fn name(&self) -> &'static str {
        "Ripple"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "ripple_count" => Some(self.values[1]),
            "palette" => Some(self.values[2]),
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
            "speed" => self.values[0] = value,
            "ripple_count" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        for r in self.ripples.iter_mut() {
            r.active = false;
        }
        self.spawn_timer = 0.35;
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let audio_on = self.values[3] != 0.0;
        let mut speed_mul = 1.0;
        let mut bright_mul = 1.0;

        if audio_on && context.audio.available {
            let bass = audio_bass(context.audio);
            let level = clamp01(context.audio.level);
            speed_mul += bass * 1.5;
            bright_mul += level * 0.5;
            if context.audio.beat {
                self.spawn_ripple(15.5, 15.5);
            }
        }

        let dt = context.delta_seconds.clamp(0.0, 1.0);
        let speed = self.values[0] * 0.3 * speed_mul;

        // Advance ripples
        for r in self.ripples.iter_mut() {
            if !r.active {
                continue;
            }
            r.radius += speed * dt;
            if r.radius > r.max_radius {
                r.active = false;
            }
        }

        // Spawn timer
        self.spawn_timer += dt;
        let max_ripples = (self.values[1] as usize).clamp(1, 8);
        let active_count = self.ripples.iter().filter(|r| r.active).count();

        if self.spawn_timer > 0.4 && active_count < max_ripples {
            self.spawn_timer = 0.0;
            let cx = 4.0 + self.rng.next_f32() * 24.0;
            let cy = 4.0 + self.rng.next_f32() * 24.0;
            self.spawn_ripple(cx, cy);
        }

        frame.clear(Rgb::BLACK);

        for (i, r) in self.ripples.iter().enumerate() {
            if !r.active {
                continue;
            }
            let fade_zone = r.max_radius * 0.3;

            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let dx = x as f32 - r.cx;
                    let dy = y as f32 - r.cy;
                    let dist = (dx * dx + dy * dy).sqrt();

                    let ring_dist = (dist - r.radius).abs();
                    if ring_dist > 3.0 {
                        continue;
                    }

                    let ring_alpha = (-ring_dist * ring_dist * 0.5).exp();
                    let life_fade = if r.radius > r.max_radius - fade_zone {
                        (r.max_radius - r.radius) / fade_zone
                    } else {
                        1.0
                    };

                    let alpha = clamp01(ring_alpha * life_fade * bright_mul);
                    if alpha < 0.01 {
                        continue;
                    }

                    let (cr, cg, cb) = self.color_from_palette(dist * 0.05 + i as f32 * 0.3, alpha);
                    let curr = frame.get(x, y);
                    let nr = curr.r.saturating_add(to_byte(cr));
                    let ng = curr.g.saturating_add(to_byte(cg));
                    let nb = curr.b.saturating_add(to_byte(cb));
                    frame.set(x, y, Rgb::new(nr, ng, nb));
                }
            }
        }
    }
}
