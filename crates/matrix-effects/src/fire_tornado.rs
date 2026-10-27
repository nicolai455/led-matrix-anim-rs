use crate::util::{alloc_boxed_2d, audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte, XorShift32};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Inferno Flame", "Hellfire Violet", "Chemical Green", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Swirl speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 35.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "vortex_twist",
        label: "Funnel twist",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 8.0,
        step: 1.0,
        default: 4.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "width",
        label: "Vortex width",
        kind: ParameterKind::Number,
        min: 4.0,
        max: 14.0,
        step: 1.0,
        default: 8.0,
        options: &[],
        unit: "px",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Flame color",
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
];

#[derive(Clone, Copy, Debug)]
struct Ember {
    x: f32,
    y: f32,
    ang: f32,
    dist: f32,
    vy: f32,
    vang: f32,
    life: f32,
}

pub struct FireTornadoEffect {
    values: [f32; 5],
    swirl_phase: f32,
    color_cycle: f32,
    flame_surge: f32,
    embers: [Ember; 24],
    heat_buffer: Box<[[f32; WIDTH]; HEIGHT]>,
    rng: XorShift32,
}

impl Default for FireTornadoEffect {
    fn default() -> Self {
        Self {
            values: [35.0, 4.0, 8.0, 3.0, 1.0],
            swirl_phase: 0.0,
            color_cycle: 0.0,
            flame_surge: 0.0,
            embers: [Ember { x: 15.5, y: 31.0, ang: 0.0, dist: 2.0, vy: -12.0, vang: 4.0, life: 0.0 }; 24],
            heat_buffer: alloc_boxed_2d(0.0),
            rng: XorShift32::new(13579),
        }
    }
}

impl FireTornadoEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn flame_color(pal: usize, heat: f32, color_cycle: f32, y_norm: f32) -> Rgb {
        let h = clamp01(heat);
        match pal {
            0 => {
                // Inferno Flame: Black -> Deep Crimson -> Fiery Orange -> Gold -> Incandescent White Hot
                if h > 0.85 {
                    let w = (h - 0.85) / 0.15;
                    Rgb::new(255, to_byte(220.0 + 35.0 * w), to_byte(80.0 + 175.0 * w))
                } else if h > 0.45 {
                    let t = (h - 0.45) / 0.40;
                    Rgb::new(255, to_byte(40.0 + 180.0 * t), 10)
                } else {
                    let r = to_byte(255.0 * (h / 0.45).powi(2));
                    Rgb::new(r, 0, 0)
                }
            }
            1 => {
                // Hellfire Violet: Dark Indigo -> Neon Magenta -> Laser Cyan
                if h > 0.8 {
                    let w = (h - 0.8) / 0.2;
                    Rgb::new(to_byte(180.0 + 75.0 * w), 255, 255)
                } else if h > 0.4 {
                    let t = (h - 0.4) / 0.4;
                    Rgb::new(to_byte(140.0 + 115.0 * t), to_byte(20.0 * t), 240)
                } else {
                    let b = to_byte(240.0 * (h / 0.4));
                    Rgb::new(to_byte(60.0 * (h / 0.4)), 0, b)
                }
            }
            2 => {
                // Chemical Green: Deep Emerald -> Neon Lime -> Electric White Core
                if h > 0.85 {
                    let w = (h - 0.85) / 0.15;
                    Rgb::new(to_byte(140.0 + 115.0 * w), 255, to_byte(100.0 + 155.0 * w))
                } else if h > 0.4 {
                    let t = (h - 0.4) / 0.45;
                    Rgb::new(to_byte(100.0 * t), 255, 30)
                } else {
                    let g = to_byte(255.0 * (h / 0.4));
                    Rgb::new(0, g, 20)
                }
            }
            _ => {
                // Rainbow Vortex
                let hue = (color_cycle + y_norm * 0.4 + h * 0.3) % 1.0;
                hsv_to_rgb(hue, 1.0 - h * 0.4, h)
            }
        }
    }
}

impl Effect for FireTornadoEffect {
    fn id(&self) -> &'static str {
        "fire_tornado"
    }

    fn name(&self) -> &'static str {
        "Fire Tornado"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "vortex_twist" => Some(self.values[1]),
            "width" => Some(self.values[2]),
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
            "vortex_twist" => self.values[1] = value,
            "width" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.swirl_phase = 0.0;
        self.color_cycle = 0.0;
        self.flame_surge = 0.0;
        self.heat_buffer = alloc_boxed_2d(0.0);
        for ember in &mut self.embers {
            ember.life = 0.0;
        }
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
            self.flame_surge = 1.0;
        } else {
            self.flame_surge = (self.flame_surge - dt * 2.8).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.7);
        self.swirl_phase = (self.swirl_phase + dt * speed * 3.5) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let pal = self.values[3] as usize;
        let twist = self.values[1];
        let base_w = self.values[2] * (1.0 + bass * 0.5 + self.flame_surge * 0.4);

        let center_x = 15.5f32;

        // 1. Calculate Tornado Funnel Geometry & Heat
        for y in 0..HEIGHT {
            let y_norm = (31.0 - y as f32) / 31.0; // 0=bottom base, 1=top sky

            // Funnel width: wide base, narrower neck in middle, flares out at top
            let neck = 0.6 + 0.4 * (y_norm - 0.4).abs();
            let cur_w = base_w * neck;

            // Spiral twist displacement
            let twist_ang = self.swirl_phase + y_norm * twist * PI;
            let center_offset = (twist_ang).sin() * 2.5 * (1.0 - y_norm * 0.5);

            for x in 0..WIDTH {
                let dx = (x as f32 - (center_x + center_offset)).abs();
                let r_norm = dx / cur_w.max(1.0);

                // Vortex walls are hotter than dead-center eye
                let vortex_lip = (-((r_norm - 0.65).powi(2) * 8.0)).exp();
                let core = (-(r_norm.powi(2) * 5.0)).exp() * 0.85;

                // Vertical convection falloff
                let height_factor = (1.0 - y_norm * 0.75).max(0.1);
                let turbulence = ((x as f32 * 0.4 + y as f32 * 0.5 - self.swirl_phase * 2.0).sin() * (0.15 + mid * 0.1)).max(0.0);

                let raw_heat = (vortex_lip * 1.1 + core + turbulence + self.flame_surge * 0.4) * height_factor;
                self.heat_buffer[y][x] = raw_heat.clamp(0.0, 1.2);
            }
        }

        // 2. Spawn and Update Embers rising through the updraft
        for ember in &mut self.embers {
            if ember.life <= 0.0 {
                // Respawn at bottom
                ember.life = 0.6 + self.rng.next_f32() * 0.6;
                ember.y = 31.0;
                ember.ang = self.rng.next_f32() * 2.0 * PI;
                ember.dist = 1.0 + self.rng.next_f32() * (base_w * 0.8);
                ember.vy = -(15.0 + self.rng.next_f32() * 20.0 + bass * 15.0);
                ember.vang = 5.0 + self.rng.next_f32() * 8.0;
            } else {
                ember.life -= dt;
                ember.y += ember.vy * dt;
                ember.ang += ember.vang * dt;

                let y_norm = (31.0 - ember.y) / 31.0;
                let cur_dist = ember.dist * (0.6 + 0.5 * y_norm);
                ember.x = center_x + ember.ang.cos() * cur_dist;

                let ex = ember.x.round() as i32;
                let ey = ember.y.round() as i32;

                if ex >= 0 && ex < WIDTH as i32 && ey >= 0 && ey < HEIGHT as i32 {
                    self.heat_buffer[ey as usize][ex as usize] += 0.8 * ember.life;
                }
            }
        }

        // 3. Render Heat Buffer to Frame
        for y in 0..HEIGHT {
            let y_norm = (31.0 - y as f32) / 31.0;
            for x in 0..WIDTH {
                let heat = self.heat_buffer[y][x];
                if heat > 0.06 {
                    let col = Self::flame_color(pal, heat, self.color_cycle, y_norm);
                    frame.set(x, y, col);
                }
            }
        }

        // Swirling spark flashes from treble
        if audio_active && treble > 0.35 {
            for i in 0..4 {
                let spark_ang = self.swirl_phase * 2.0 + i as f32 * 1.5;
                let sx = (center_x + spark_ang.cos() * base_w * 0.7).round() as i32;
                let sy = (20.0 - (spark_ang * 2.0).sin() * 8.0).round() as i32;
                if sx >= 0 && sx < WIDTH as i32 && sy >= 0 && sy < HEIGHT as i32 {
                    frame.set(sx as usize, sy as usize, Rgb::WHITE);
                }
            }
        }
    }
}
