use crate::util::{audio_bass, audio_mid, audio_treble, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

const PALETTES: &[&str] = &["Bone White", "Radioactive Green", "Cyber Neon", "Rainbow Bones"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "Dance speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 45.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "rib_rattle",
        label: "Rib rattle amp",
        kind: ParameterKind::Number,
        min: 1.0,
        max: 5.0,
        step: 0.5,
        default: 2.5,
        options: &[],
        unit: "px",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Skeleton style",
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

pub struct DancingSkeletonEffect {
    values: [f32; 4],
    dance_time: f32,
    strobe_pulse: f32,
    color_cycle: f32,
}

impl Default for DancingSkeletonEffect {
    fn default() -> Self {
        Self {
            values: [45.0, 2.5, 0.0, 1.0],
            dance_time: 0.0,
            strobe_pulse: 0.0,
            color_cycle: 0.0,
        }
    }
}

impl DancingSkeletonEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn bone_colors(pal: usize, color_cycle: f32) -> (Rgb, Rgb) {
        // (Bone, EyeGlow)
        match pal {
            0 => (Rgb::new(240, 240, 230), Rgb::new(255, 30, 20)),
            1 => (Rgb::new(80, 255, 60), Rgb::new(255, 230, 0)),
            2 => (Rgb::new(0, 240, 255), Rgb::new(255, 0, 180)),
            _ => (
                hsv_to_rgb(color_cycle, 0.85, 1.0),
                hsv_to_rgb((color_cycle + 0.5) % 1.0, 0.9, 1.0),
            ),
        }
    }
}

impl Effect for DancingSkeletonEffect {
    fn id(&self) -> &'static str {
        "dancing_skeleton"
    }

    fn name(&self) -> &'static str {
        "Dancing Skeleton"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "rib_rattle" => Some(self.values[1]),
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
            "rib_rattle" => self.values[1] = value,
            "palette" => self.values[2] = value,
            "audio_reactive" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.dance_time = 0.0;
        self.strobe_pulse = 0.0;
        self.color_cycle = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        // Graveyard midnight atmosphere
        frame.clear(Rgb::new(15, 8, 25));

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[3] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        if beat {
            self.strobe_pulse = 1.0;
        } else {
            self.strobe_pulse = (self.strobe_pulse - dt * 4.0).max(0.0);
        }

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.8 + self.strobe_pulse * 0.5);
        self.dance_time = (self.dance_time + dt * speed * 2.2) % (2.0 * PI);
        self.color_cycle = (self.color_cycle + dt * 0.16) % 1.0;

        let pal = self.values[2] as usize;
        let (bone_c, eye_c) = Self::bone_colors(pal, self.color_cycle);

        // Funky dance cycle angles:
        let sway = self.dance_time.sin();
        let bounce = (self.dance_time * 2.0).sin().abs() * (2.0 + bass * 2.0);
        let rib_amp = self.values[1] + bass * 1.5;

        let root_x = 15.5f32 + sway * 2.5;
        let root_y = 15.0f32 - bounce;

        // 1. Skull (cx: root_x, cy: root_y - 8)
        let skull_x = root_x + sway * 1.2;
        let skull_y = root_y - 8.0;

        for dy in -4..=2i32 {
            let py = (skull_y + dy as f32).round() as i32;
            if py < 0 || py >= HEIGHT as i32 {
                continue;
            }
            let half_w = if dy == 2 {
                2.0f32 // Jaw
            } else if dy >= 0 {
                3.5f32
            } else {
                4.2f32 // Cranium
            };

            let hw = half_w.round() as i32;
            for dx in -hw..=hw {
                let px = (skull_x + dx as f32).round() as i32;
                if px >= 0 && px < WIDTH as i32 {
                    frame.set(px as usize, py as usize, bone_c);
                }
            }
        }

        // Glowing Eye Sockets & Nose cavity
        let eye_py = (skull_y - 0.5).round() as usize;
        let eye_lx = (skull_x - 1.8).round() as usize;
        let eye_rx = (skull_x + 1.8).round() as usize;
        if eye_py < HEIGHT {
            if eye_lx < WIDTH { frame.set(eye_lx, eye_py, eye_c); }
            if eye_rx < WIDTH { frame.set(eye_rx, eye_py, eye_c); }
        }

        let nose_py = (skull_y + 1.0).round() as usize;
        let nose_px = skull_x.round() as usize;
        if nose_py < HEIGHT && nose_px < WIDTH {
            frame.set(nose_px, nose_py, Rgb::BLACK);
        }

        // 2. Spine & Ribcage
        let spine_top = (skull_y + 3.0).round() as usize;
        let spine_bot = (root_y + 3.0).round() as usize;

        // Spine column
        for y in spine_top..=spine_bot {
            if y < HEIGHT {
                let x = root_x.round() as usize;
                if x < WIDTH {
                    frame.set(x, y, bone_c);
                }
            }
        }

        // Ribs (3 pairs of ribs radiating outwards)
        for rib_idx in 0..3 {
            let ry = (skull_y + 4.5 + rib_idx as f32 * 1.8).round() as usize;
            if ry >= HEIGHT {
                continue;
            }
            let rib_w = (rib_amp + (2 - rib_idx) as f32 * 1.2 + (self.dance_time * 4.0 + rib_idx as f32).sin() * 0.8).round() as i32;

            for dx in -rib_w..=rib_w {
                if dx == 0 {
                    continue;
                }
                let px = (root_x + dx as f32).round() as i32;
                if px >= 0 && px < WIDTH as i32 {
                    frame.set(px as usize, ry, bone_c);
                }
            }
        }

        // 3. Pelvis Bone at (root_x, root_y + 3)
        let pelv_y = (root_y + 3.0).round() as usize;
        if pelv_y < HEIGHT {
            for dx in -2..=2i32 {
                let px = (root_x + dx as f32).round() as i32;
                if px >= 0 && px < WIDTH as i32 {
                    frame.set(px as usize, pelv_y, bone_c);
                }
            }
        }

        // 4. Funky Jazz Arms:
        // Left arm waves high or low
        let arm_phase = self.dance_time;
        let l_elbow_x = root_x - 5.0 - (arm_phase).sin() * 2.0;
        let l_elbow_y = root_y - 3.0 + (arm_phase).cos() * 2.0;
        let l_hand_x = l_elbow_x - 3.0;
        let l_hand_y = l_elbow_y + (arm_phase * 1.5).sin() * 4.0;

        let r_elbow_x = root_x + 5.0 + (arm_phase).sin() * 2.0;
        let r_elbow_y = root_y - 3.0 - (arm_phase).cos() * 2.0;
        let r_hand_x = r_elbow_x + 3.0;
        let r_hand_y = r_elbow_y - (arm_phase * 1.5).sin() * 4.0;

        let arm_points = [
            (root_x - 2.0, root_y - 2.0),
            (l_elbow_x, l_elbow_y),
            (l_hand_x, l_hand_y),
            (root_x + 2.0, root_y - 2.0),
            (r_elbow_x, r_elbow_y),
            (r_hand_x, r_hand_y),
        ];

        for &(ax, ay) in &arm_points {
            let px = ax.round() as i32;
            let py = ay.round() as i32;
            if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                frame.set(px as usize, py as usize, bone_c);
            }
        }

        // 5. Dancing Legs & Feet:
        // Left Leg: Hip -> Knee -> Foot
        let l_knee_x = root_x - 3.0 + (sway * 3.0).max(-2.0);
        let l_knee_y = root_y + 7.0;
        let l_foot_x = root_x - 4.0 - sway * 4.0;
        let l_foot_y = 28.0;

        // Right Leg: Hip -> Knee -> Foot
        let r_knee_x = root_x + 3.0 + (sway * 3.0).min(2.0);
        let r_knee_y = root_y + 7.0;
        let r_foot_x = root_x + 4.0 - sway * 4.0;
        let r_foot_y = 28.0;

        let leg_points = [
            (l_knee_x, l_knee_y),
            (l_foot_x, l_foot_y),
            (r_knee_x, r_knee_y),
            (r_foot_x, r_foot_y),
        ];

        for &(lx, ly) in &leg_points {
            let px = lx.round() as i32;
            let py = ly.round() as i32;
            if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                frame.set(px as usize, py as usize, bone_c);
                if px + 1 < WIDTH as i32 {
                    frame.set((px + 1) as usize, py as usize, bone_c);
                }
            }
        }

        // 6. Audio strobe flash & graveyard mist
        if self.strobe_pulse > 0.1 {
            let flash_c = to_byte(self.strobe_pulse * 120.0);
            for x in 0..WIDTH {
                let cur = frame.get(x, 31);
                frame.set(x, 31, Rgb::new(cur.r.saturating_add(flash_c), cur.g, cur.b.saturating_add(flash_c)));
            }
        }

        // Sparkle magic dust on treble
        if audio_active && treble > 0.4 {
            for i in 0..3 {
                let sx = (i * 9 + 4) % WIDTH;
                let sy = (i * 6 + 3) % 12;
                frame.set(sx, sy, Rgb::new(180, 240, 255));
            }
        }
        if audio_active && mid > 0.3 {
            frame.set(29, 29, Rgb::new(100, 255, 100));
        }
    }
}
