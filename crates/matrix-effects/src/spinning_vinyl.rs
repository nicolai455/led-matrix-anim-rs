use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Classic Black", "Neon Collector", "Gold Master", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "RPM speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 33.0, // 33 RPM classic
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "scratch_react",
        label: "Scratch reactivity",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 60.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Vinyl style",
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

pub struct SpinningVinylEffect {
    values: [f32; 4],
    rotation: f32,
    color_cycle: f32,
    scratch_vel: f32,
}

impl Default for SpinningVinylEffect {
    fn default() -> Self {
        Self {
            values: [33.0, 60.0, 3.0, 1.0],
            rotation: 0.0,
            color_cycle: 0.0,
            scratch_vel: 0.0,
        }
    }
}

impl SpinningVinylEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn vinyl_colors(pal: usize, color_cycle: f32, r_norm: f32, sheen: f32) -> (Rgb, Rgb) {
        // Returns (groove_color, label_color)
        let s = clamp01(sheen);
        match pal {
            0 => {
                // Classic Black Vinyl
                let g_val = to_byte(25.0 + 80.0 * s);
                let groove = Rgb::new(g_val, g_val, g_val);
                let label = Rgb::new(220, 30, 40); // Red retro label
                (groove, label)
            }
            1 => {
                // Neon Collector (Translucent Cyan with Hot Pink Label)
                let r = to_byte(20.0 + 50.0 * s);
                let g = to_byte(140.0 + 115.0 * s);
                let b = to_byte(200.0 + 55.0 * s);
                (Rgb::new(r, g, b), Rgb::new(255, 20, 160))
            }
            2 => {
                // Gold Master (Heavy gold metallic shimmer)
                let r = to_byte(180.0 + 75.0 * s);
                let g = to_byte(130.0 + 85.0 * s);
                let b = to_byte(30.0 + 40.0 * s);
                (Rgb::new(r, g, b), Rgb::new(40, 40, 40))
            }
            _ => {
                // Rainbow Vinyl
                let h = (color_cycle + r_norm * 0.4) % 1.0;
                let groove = hsv_to_rgb(h, 0.9, 0.25 + 0.75 * s);
                let label = hsv_to_rgb((h + 0.5) % 1.0, 0.95, 1.0);
                (groove, label)
            }
        }
    }
}

impl Effect for SpinningVinylEffect {
    fn id(&self) -> &'static str {
        "spinning_vinyl"
    }

    fn name(&self) -> &'static str {
        "Spinning DJ Vinyl"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "scratch_react" => Some(self.values[1]),
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
            "scratch_react" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.rotation = 0.0;
        self.color_cycle = 0.0;
        self.scratch_vel = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::new(18, 18, 22)); // Turntable deck body

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[3] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        // Beat scratch impulse
        if beat {
            let scratch_power = self.values[1] / 50.0;
            self.scratch_vel = -8.0 * scratch_power;
        } else {
            self.scratch_vel = (self.scratch_vel + dt * 15.0).min(0.0);
        }

        let base_speed = (self.values[0] / 33.0) * 3.5;
        let total_speed = base_speed + self.scratch_vel + bass * 1.5;
        self.rotation = (self.rotation + dt * total_speed) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.15) % 1.0;

        let pal = self.values[2] as usize;
        let cx = 14.5f32;
        let cy = 15.5f32;
        let vinyl_r = 13.5f32;
        let label_r = 5.0f32;
        let spindle_r = 1.2f32;

        let r_sq = vinyl_r * vinyl_r;
        let l_sq = label_r * label_r;
        let sp_sq = spindle_r * spindle_r;

        // Render Vinyl disc pixels
        for y in 0..HEIGHT {
            let dy = y as f32 - cy;
            for x in 0..WIDTH {
                let dx = x as f32 - cx;
                let d2 = dx * dx + dy * dy;

                if d2 <= r_sq {
                    let dist = d2.sqrt();
                    let theta = dy.atan2(dx);
                    let rel_theta = theta - self.rotation;

                    if d2 <= sp_sq {
                        // Spindle metallic brass/silver
                        frame.set(x, y, Rgb::WHITE);
                    } else if d2 <= l_sq {
                        // Center record label
                        let (_, label_col) = Self::vinyl_colors(pal, self.color_cycle, dist / vinyl_r, 0.0);
                        // Rotating label text/design marker
                        let marker = (rel_theta * 2.0).sin() > 0.8;
                        let col = if marker { Rgb::WHITE } else { label_col };
                        frame.set(x, y, col);
                    } else {
                        // Vinyl grooves with double anisotropic reflection sheen (bow-tie light reflection)
                        let sheen_angle = (theta * 2.0 - 0.7).cos().abs().powi(4);
                        let groove_ripple = (dist * 4.0).sin() * 0.2;
                        let sheen = (sheen_angle + groove_ripple + treble * 0.35).clamp(0.0, 1.0);

                        let (groove_col, _) = Self::vinyl_colors(pal, self.color_cycle, dist / vinyl_r, sheen);
                        frame.set(x, y, groove_col);
                    }
                }
            }
        }

        // Render DJ Tonearm on upper right (Pivot at (29, 3), Needle head at (18, 14))
        let pivot_x = 29i32;
        let pivot_y = 3i32;
        let head_x = 18i32;
        let head_y = 14i32 + (bass * 1.5).round() as i32;

        // Arm shaft
        let dx = head_x - pivot_x;
        let dy = head_y - pivot_y;
        let steps = dx.abs().max(dy.abs()) as usize;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let ax = (pivot_x as f32 + t * dx as f32).round() as i32;
            let ay = (pivot_y as f32 + t * dy as f32).round() as i32;
            if ax >= 0 && ax < WIDTH as i32 && ay >= 0 && ay < HEIGHT as i32 {
                frame.set(ax as usize, ay as usize, Rgb::new(190, 190, 205)); // Brushed aluminum arm
            }
        }

        // Needle cartridge (Red / Silver head)
        if head_x >= 0 && head_x < WIDTH as i32 && head_y >= 0 && head_y < HEIGHT as i32 {
            let cart_c = Rgb::new(255, 30, 40);
            frame.set(head_x as usize, head_y as usize, cart_c);
            if head_x + 1 < WIDTH as i32 {
                frame.set((head_x + 1) as usize, head_y as usize, cart_c);
            }
        }

        // Tonearm pivot base (Circular metallic housing)
        for py in -1..=1 {
            for px in -1..=1 {
                let bx = pivot_x + px;
                let by = pivot_y + py;
                if bx >= 0 && bx < WIDTH as i32 && by >= 0 && by < HEIGHT as i32 {
                    frame.set(bx as usize, by as usize, Rgb::new(120, 120, 130));
                }
            }
        }

        // Deck Power strobe / Speed dots along left edge (x=0, y=10..22)
        for y in 10..=22 {
            #[allow(clippy::manual_is_multiple_of)]
            if (y + (self.rotation * 4.0) as usize) % 2 == 0 {
                let strobe_c = Rgb::new(to_byte(40.0 + mid * 120.0), to_byte(220.0 + treble * 35.0), 255);
                frame.set(0, y, strobe_c);
            }
        }
    }
}
