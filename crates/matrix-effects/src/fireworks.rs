use crate::util::{alloc_boxed_2d, audio_bass, audio_mid, clamp01, hsv_to_rgb, to_byte, XorShift32};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const PALETTES: &[&str] = &["Carnival", "Golden Glow", "Neon Dream", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 6] = [
    ParameterDescriptor {
        id: "speed",
        label: "Speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 50.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "gravity",
        label: "Gravity",
        kind: ParameterKind::Number,
        min: 5.0,
        max: 50.0,
        step: 1.0,
        default: 20.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "sparks",
        label: "Sparks",
        kind: ParameterKind::Number,
        min: 16.0,
        max: 64.0,
        step: 1.0,
        default: 32.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Palette",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 3.0,
        step: 1.0,
        default: 3.0,
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
    ParameterDescriptor {
        id: "trail",
        label: "Trail length",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 90.0,
        step: 1.0,
        default: 50.0,
        options: &[],
        unit: "%",
    },
];

const MAX_SPARKS: usize = 64;

#[derive(Clone, Copy, Debug)]
struct Rocket {
    active: bool,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    target_y: f32,
    hue: f32,
}

#[derive(Clone, Copy, Debug)]
struct Spark {
    active: bool,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    decay: f32,
    hue: f32,
}

pub struct FireworksEffect {
    values: [f32; 6],
    rockets: [Rocket; 2],
    sparks: [Spark; MAX_SPARKS],
    buffer: Box<[[Rgb; WIDTH]; HEIGHT]>,
    rng: XorShift32,
    color_cycle: f32,
}

impl Default for FireworksEffect {
    fn default() -> Self {
        Self {
            values: [50.0, 20.0, 32.0, 3.0, 1.0, 50.0],
            rockets: [Rocket {
                active: false,
                x: 16.0,
                y: 31.0,
                vx: 0.0,
                vy: -20.0,
                target_y: 10.0,
                hue: 0.0,
            }; 2],
            sparks: [Spark {
                active: false,
                x: 0.0,
                y: 0.0,
                vx: 0.0,
                vy: 0.0,
                life: 0.0,
                decay: 1.0,
                hue: 0.0,
            }; MAX_SPARKS],
            buffer: alloc_boxed_2d(Rgb::BLACK),
            rng: XorShift32::new(777123),
            color_cycle: 0.0,
        }
    }
}

impl FireworksEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn spark_color(pal: usize, color_cycle: f32, hue: f32, life: f32) -> Rgb {
        let v = clamp01(life.sqrt());
        match pal {
            0 => {
                // Carnival
                hsv_to_rgb(hue, 0.9, v)
            }
            1 => {
                // Golden Glow
                let r = to_byte(255.0 * v);
                let g = to_byte(180.0 * v * life);
                let b = to_byte(40.0 * v * life * life);
                Rgb::new(r, g, b)
            }
            2 => {
                // Neon Dream
                let h = if life > 0.5 { 0.85 } else { 0.5 };
                hsv_to_rgb(h, 0.95, v)
            }
            _ => {
                // Rainbow Fade
                let dynamic_h = (hue + color_cycle) % 1.0;
                hsv_to_rgb(dynamic_h, 1.0, v)
            }
        }
    }

    fn launch_rocket(&mut self, audio_hue_offset: f32) {
        for rocket in &mut self.rockets {
            if !rocket.active {
                rocket.active = true;
                rocket.x = 4.0 + self.rng.next_f32() * 24.0;
                rocket.y = 31.0;
                rocket.vx = (self.rng.next_f32() - 0.5) * 6.0;
                let speed_scale = self.values[0] / 50.0;
                rocket.vy = -(18.0 + self.rng.next_f32() * 10.0) * speed_scale;
                rocket.target_y = 4.0 + self.rng.next_f32() * 14.0;
                rocket.hue = (self.color_cycle + audio_hue_offset + self.rng.next_f32() * 0.2) % 1.0;
                break;
            }
        }
    }

    fn explode(&mut self, x: f32, y: f32, base_hue: f32, bass_boost: f32) {
        let spark_count = (self.values[2] as usize).min(MAX_SPARKS);
        let speed_scale = (self.values[0] / 50.0) * (1.0 + bass_boost * 0.6);
        let mut launched = 0;

        for spark in &mut self.sparks {
            if !spark.active {
                spark.active = true;
                spark.x = x;
                spark.y = y;
                let angle = self.rng.next_f32() * std::f32::consts::TAU;
                let speed = (3.0 + self.rng.next_f32() * 12.0) * speed_scale;
                spark.vx = angle.cos() * speed;
                spark.vy = angle.sin() * speed;
                spark.life = 1.0;
                spark.decay = 0.8 + self.rng.next_f32() * 0.8;
                spark.hue = (base_hue + (self.rng.next_f32() - 0.5) * 0.15 + 1.0) % 1.0;
                launched += 1;
                if launched >= spark_count {
                    break;
                }
            }
        }
    }
}

impl Effect for FireworksEffect {
    fn id(&self) -> &'static str {
        "fireworks"
    }

    fn name(&self) -> &'static str {
        "Particle Fireworks"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "gravity" => Some(self.values[1]),
            "sparks" => Some(self.values[2]),
            "palette" => Some(self.values[3]),
            "audio_reactive" => Some(self.values[4]),
            "trail" => Some(self.values[5]),
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
            "gravity" => self.values[1] = value,
            "sparks" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            "trail" => self.values[5] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        for rocket in &mut self.rockets {
            rocket.active = false;
        }
        for spark in &mut self.sparks {
            spark.active = false;
        }
        self.buffer = alloc_boxed_2d(Rgb::BLACK);
        self.color_cycle = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[4] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        // Fade existing buffer for phosphor trails
        let trail_decay = (self.values[5] / 100.0).clamp(0.1, 0.95);
        let decay_mult = trail_decay.powf(dt * 30.0);

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let px = &mut self.buffer[y][x];
                px.r = (px.r as f32 * decay_mult) as u8;
                px.g = (px.g as f32 * decay_mult) as u8;
                px.b = (px.b as f32 * decay_mult) as u8;
            }
        }

        // Auto launch or beat launch
        let active_rockets = self.rockets.iter().filter(|r| r.active).count();
        if beat || active_rockets == 0 || (active_rockets < 2 && self.rng.next_f32() < dt * 1.5) {
            self.launch_rocket(mid);
        }

        let gravity = self.values[1] * 0.5;

        // Update rockets
        for i in 0..self.rockets.len() {
            if !self.rockets[i].active {
                continue;
            }
            let rocket = &mut self.rockets[i];
            rocket.x += rocket.vx * dt;
            rocket.y += rocket.vy * dt;
            rocket.vy += gravity * 0.4 * dt;

            // Rocket spark
            let rx = rocket.x.round() as i32;
            let ry = rocket.y.round() as i32;
            if rx >= 0 && rx < WIDTH as i32 && ry >= 0 && ry < HEIGHT as i32 {
                self.buffer[ry as usize][rx as usize] = Rgb::WHITE;
            }

            // Detonate at apex or target_y or on beat
            if rocket.vy >= -2.0 || rocket.y <= rocket.target_y || (beat && rocket.y < 24.0) {
                rocket.active = false;
                let rx = rocket.x;
                let ry = rocket.y;
                let hue = rocket.hue;
                self.explode(rx, ry, hue, bass);
            }
        }

        // Update sparks
        let drag = 0.94f32.powf(dt * 30.0);
        let pal = self.values[3] as usize;
        let color_cycle = self.color_cycle;
        for spark in &mut self.sparks {
            if !spark.active {
                continue;
            }
            spark.x += spark.vx * dt;
            spark.y += spark.vy * dt;
            spark.vx *= drag;
            spark.vy = spark.vy * drag + gravity * dt;
            spark.life -= spark.decay * dt;

            if spark.life <= 0.0 || spark.x < 0.0 || spark.x >= WIDTH as f32 || spark.y < 0.0 || spark.y >= HEIGHT as f32 {
                spark.active = false;
                continue;
            }

            let sx = spark.x as usize;
            let sy = spark.y as usize;
            if sx < WIDTH && sy < HEIGHT {
                let color = Self::spark_color(pal, color_cycle, spark.hue, spark.life);
                let current = &mut self.buffer[sy][sx];
                current.r = current.r.saturating_add(color.r);
                current.g = current.g.saturating_add(color.g);
                current.b = current.b.saturating_add(color.b);
            }
        }

        // Copy buffer to frame
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                frame.set(x, y, self.buffer[y][x]);
            }
        }
    }
}
