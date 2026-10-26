use crate::util::{alloc_boxed_3d, audio_bass, clamp01, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Cosmos", "Fire", "Ice"];

const PARAMETERS: [ParameterDescriptor; 6] = [
    ParameterDescriptor {
        id: "orbit_speed",
        label: "Orbit speed",
        kind: ParameterKind::Number,
        min: 0.0,
        max: 180.0,
        step: 1.0,
        default: 30.0,
        options: &[],
        unit: "deg/s",
    },
    ParameterDescriptor {
        id: "particle_count",
        label: "Particles",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 32.0,
        step: 1.0,
        default: 12.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "trail_length",
        label: "Trail length",
        kind: ParameterKind::Number,
        min: 0.0,
        max: 100.0,
        step: 1.0,
        default: 70.0,
        options: &[],
        unit: "%",
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
        id: "audio_enabled",
        label: "Audio reactive",
        kind: ParameterKind::Boolean,
        min: 0.0,
        max: 1.0,
        step: 1.0,
        default: 0.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "audio_amount",
        label: "Audio amount",
        kind: ParameterKind::Number,
        min: 0.0,
        max: 100.0,
        step: 1.0,
        default: 50.0,
        options: &[],
        unit: "%",
    },
];

pub struct BlackHoleEffect {
    values: [f32; 6],
    trails: Box<[[[f32; 3]; WIDTH]; HEIGHT]>,
    phase: f32,
    pulse: f32,
    orbital_phases: [f32; 32],
    accumulator: f32,
    started: bool,
}

impl Default for BlackHoleEffect {
    fn default() -> Self {
        Self {
            values: [30.0, 12.0, 70.0, 0.0, 0.0, 50.0],
            trails: alloc_boxed_3d(0.0),
            phase: 0.0,
            pulse: 0.0,
            orbital_phases: [0.0; 32],
            accumulator: 0.0,
            started: false,
        }
    }
}

impl BlackHoleEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn color(&self, t: f32) -> (f32, f32, f32) {
        let t_norm = 0.5 + 0.5 * (t).sin();
        let palette = self.values[3] as usize;
        if palette == 1 {
            // Fire
            (255.0, 35.0 + 165.0 * t_norm, 4.0 + 45.0 * t_norm)
        } else if palette == 2 {
            // Ice
            (25.0 + 110.0 * t_norm, 130.0 + 100.0 * t_norm, 255.0)
        } else {
            // Cosmos
            (100.0 + 135.0 * t_norm, 25.0 + 115.0 * t_norm, 255.0)
        }
    }

    fn splat(&mut self, x: f32, y: f32, r: f32, g: f32, b: f32, energy: f32) {
        let left = x.floor() as i32;
        let top = y.floor() as i32;
        for dy in 0..2 {
            for dx in 0..2 {
                let xx = left + dx;
                let yy = top + dy;
                if xx < 0 || xx >= WIDTH as i32 || yy < 0 || yy >= HEIGHT as i32 {
                    continue;
                }
                let wx = if dx == 1 {
                    x - left as f32
                } else {
                    1.0 - (x - left as f32)
                };
                let wy = if dy == 1 {
                    y - top as f32
                } else {
                    1.0 - (y - top as f32)
                };
                let weight = wx * wy * energy;
                let pixel = &mut self.trails[yy as usize][xx as usize];
                pixel[0] = (pixel[0] + r * weight).min(255.0);
                pixel[1] = (pixel[1] + g * weight).min(255.0);
                pixel[2] = (pixel[2] + b * weight).min(255.0);
            }
        }
    }

    fn tick(&mut self, drive: f32) {
        let dt = 0.016;
        let speed_rad = self.values[0] * (std::f32::consts::PI / 180.0);
        self.phase = (self.phase + speed_rad * dt * (1.0 + drive)) % (2.0 * PI);
        self.pulse *= (-dt * 4.0f32).exp();

        let lifetime = 0.025 + self.values[2] * 0.012;
        let decay = (-dt / lifetime).exp();

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                for c in 0..3 {
                    self.trails[y][x][c] *= decay;
                }
            }
        }

        let particles = (self.values[1] as usize).clamp(1, 32);
        for i in 0..32 {
            let radius = 4.0 + 10.3 * (0.5 + 0.5 * (i as f32 * 1.713).sin());
            self.orbital_phases[i] = (self.orbital_phases[i]
                + speed_rad * dt * (1.0 + drive) * (0.65 + 5.0 / radius))
                % (2.0 * PI);
        }

        let cx = 15.5;
        let cy = 15.5;

        for i in 0..particles {
            let base_radius = 4.0 + 10.3 * (0.5 + 0.5 * (i as f32 * 1.713).sin());
            let angle = self.orbital_phases[i];
            let wobble = 0.8 * (angle * 3.0 + i as f32).sin();
            let r = (base_radius + wobble + self.pulse * 1.5).max(1.0);
            let px = cx + r * angle.cos();
            let py = cy + r * angle.sin();

            let (cr, cg, cb) = self.color(angle + i as f32 * 0.3);
            let energy = 0.8 + 0.4 * (angle * 2.0).sin().abs() + drive * 0.5;
            self.splat(px, py, cr, cg, cb, energy);
        }
    }
}

impl Effect for BlackHoleEffect {
    fn id(&self) -> &'static str {
        "black_hole"
    }

    fn name(&self) -> &'static str {
        "Black Hole"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "orbit_speed" => Some(self.values[0]),
            "particle_count" => Some(self.values[1]),
            "trail_length" => Some(self.values[2]),
            "palette" => Some(self.values[3]),
            "audio_enabled" => Some(self.values[4]),
            "audio_amount" => Some(self.values[5]),
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
            "orbit_speed" => self.values[0] = value,
            "particle_count" => self.values[1] = value,
            "trail_length" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_enabled" => self.values[4] = value,
            "audio_amount" => self.values[5] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.trails.as_mut().fill([[0.0; 3]; WIDTH]);
        self.phase = 0.0;
        self.pulse = 0.0;
        self.orbital_phases = [0.0; 32];
        self.accumulator = 0.0;
        self.started = false;
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let mut drive = 0.0;
        if self.values[4] != 0.0 && context.audio.available {
            let amount = self.values[5] * 0.01;
            let level = clamp01(context.audio.level);
            let bass = audio_bass(context.audio);
            drive = amount * (level * 0.4 + bass * 0.6);
            if amount > 0.0 && context.audio.beat {
                self.pulse = amount;
            }
        }

        let dt = context.delta_seconds.clamp(0.0, 1.0);
        self.accumulator += dt;

        if !self.started {
            self.tick(drive);
            self.started = true;
        }

        let fixed_step = 0.016;
        let mut steps = 0;
        while self.accumulator >= fixed_step && steps < 10 {
            self.tick(drive);
            self.accumulator -= fixed_step;
            steps += 1;
        }
        if self.accumulator >= fixed_step {
            self.accumulator = 0.0;
        }

        // Draw event horizon (dark center) and accretion disk trails
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let dx = x as f32 - 15.5;
                let dy = y as f32 - 15.5;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist < 2.5 {
                    frame.set(x, y, Rgb::BLACK);
                } else {
                    let p = self.trails[y][x];
                    let r = to_byte(p[0]);
                    let g = to_byte(p[1]);
                    let b = to_byte(p[2]);
                    frame.set(x, y, Rgb::new(r, g, b));
                }
            }
        }
    }
}
