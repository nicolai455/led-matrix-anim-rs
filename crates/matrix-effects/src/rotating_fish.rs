use crate::util::{alloc_boxed_2d, audio_bass, to_byte, XorShift32};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const AXES: &[&str] = &["Horizontal", "Vertical"];
const COLOR_MODES: &[&str] = &["Goldfish", "Rainbow", "Solid"];

const PARAMETERS: [ParameterDescriptor; 6] = [
    ParameterDescriptor {
        id: "axis",
        label: "Rotation axis",
        kind: ParameterKind::Select,
        min: 0.0,
        max: 1.0,
        step: 1.0,
        default: 0.0,
        options: AXES,
        unit: "",
    },
    ParameterDescriptor {
        id: "speed",
        label: "Spin speed",
        kind: ParameterKind::Number,
        min: 0.0,
        max: 180.0,
        step: 1.0,
        default: 60.0,
        options: &[],
        unit: "degrees/s",
    },
    ParameterDescriptor {
        id: "size",
        label: "Size",
        kind: ParameterKind::Number,
        min: 50.0,
        max: 100.0,
        step: 1.0,
        default: 100.0,
        options: &[],
        unit: "%",
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
        default: 35.0,
        options: &[],
        unit: "degrees",
    },
    ParameterDescriptor {
        id: "reactivity",
        label: "Reactivity",
        kind: ParameterKind::Number,
        min: 0.0,
        max: 200.0,
        step: 1.0,
        default: 100.0,
        options: &[],
        unit: "%",
    },
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Head,
    Back,
    Belly,
    Flank,
    Fin,
    Tail,
}

#[derive(Clone, Copy)]
struct FishVertex {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Clone, Copy)]
struct Triangle {
    v0: usize,
    v1: usize,
    v2: usize,
    part: Part,
}

#[derive(Clone, Copy, Default)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Clone, Copy, Default)]
struct Bubble {
    x: f32,
    y: f32,
    speed: f32,
    active: bool,
}

const VERTEX_COUNT: usize = 27;
const TRIANGLE_COUNT: usize = 44;
const BUBBLE_COUNT: usize = 6;

const BASE_VERTICES: [FishVertex; VERTEX_COUNT] = [
    // 0: Snout tip
    FishVertex { x: 1.15, y: -0.05, z: 0.0 },
    // Ring 1: Head / Snout (x = 0.65)
    FishVertex { x: 0.65, y: 0.32, z: 0.0 },
    FishVertex { x: 0.65, y: -0.24, z: 0.0 },
    FishVertex { x: 0.65, y: 0.02, z: 0.28 },
    FishVertex { x: 0.65, y: 0.02, z: -0.28 },
    // Ring 2: Mid-body (x = 0.05) - widest part
    FishVertex { x: 0.05, y: 0.52, z: 0.0 },
    FishVertex { x: 0.05, y: -0.40, z: 0.0 },
    FishVertex { x: 0.05, y: 0.05, z: 0.42 },
    FishVertex { x: 0.05, y: 0.05, z: -0.42 },
    // Ring 3: Rear body (x = -0.5)
    FishVertex { x: -0.5, y: 0.32, z: 0.0 },
    FishVertex { x: -0.5, y: -0.24, z: 0.0 },
    FishVertex { x: -0.5, y: 0.04, z: 0.22 },
    FishVertex { x: -0.5, y: 0.04, z: -0.22 },
    // Ring 4: Tail peduncle base (x = -0.85)
    FishVertex { x: -0.85, y: 0.12, z: 0.0 },
    FishVertex { x: -0.85, y: -0.1, z: 0.0 },
    FishVertex { x: -0.85, y: 0.0, z: 0.07 },
    FishVertex { x: -0.85, y: 0.0, z: -0.07 },
    // Tail fin (caudal fin)
    FishVertex { x: -1.38, y: 0.55, z: 0.0 },
    FishVertex { x: -1.08, y: 0.0, z: 0.0 },
    FishVertex { x: -1.38, y: -0.5, z: 0.0 },
    // Dorsal fin
    FishVertex { x: -0.1, y: 0.88, z: 0.0 },
    FishVertex { x: -0.45, y: 0.40, z: 0.0 },
    // Ventral / Anal fin
    FishVertex { x: -0.35, y: -0.55, z: 0.0 },
    // Pectoral fins (right and left)
    FishVertex { x: 0.2, y: -0.18, z: 0.65 },
    FishVertex { x: 0.2, y: -0.18, z: -0.65 },
    // Eyes
    FishVertex { x: 0.72, y: 0.14, z: 0.24 },
    FishVertex { x: 0.72, y: 0.14, z: -0.24 },
];

const TRIANGLES: [Triangle; TRIANGLE_COUNT] = [
    // Nose cone
    Triangle { v0: 0, v1: 1, v2: 3, part: Part::Head },
    Triangle { v0: 0, v1: 3, v2: 2, part: Part::Belly },
    Triangle { v0: 0, v1: 2, v2: 4, part: Part::Belly },
    Triangle { v0: 0, v1: 4, v2: 1, part: Part::Head },
    // Segment 1 (Head to Mid-body)
    Triangle { v0: 1, v1: 5, v2: 7, part: Part::Back },
    Triangle { v0: 1, v1: 7, v2: 3, part: Part::Flank },
    Triangle { v0: 3, v1: 7, v2: 6, part: Part::Belly },
    Triangle { v0: 3, v1: 6, v2: 2, part: Part::Belly },
    Triangle { v0: 2, v1: 6, v2: 8, part: Part::Belly },
    Triangle { v0: 2, v1: 8, v2: 4, part: Part::Belly },
    Triangle { v0: 4, v1: 8, v2: 5, part: Part::Flank },
    Triangle { v0: 4, v1: 5, v2: 1, part: Part::Back },
    // Segment 2 (Mid-body to Rear)
    Triangle { v0: 5, v1: 9, v2: 11, part: Part::Back },
    Triangle { v0: 5, v1: 11, v2: 7, part: Part::Flank },
    Triangle { v0: 7, v1: 11, v2: 10, part: Part::Belly },
    Triangle { v0: 7, v1: 10, v2: 6, part: Part::Belly },
    Triangle { v0: 6, v1: 10, v2: 12, part: Part::Belly },
    Triangle { v0: 6, v1: 12, v2: 8, part: Part::Belly },
    Triangle { v0: 8, v1: 12, v2: 9, part: Part::Flank },
    Triangle { v0: 8, v1: 9, v2: 5, part: Part::Back },
    // Segment 3 (Rear to Tail peduncle)
    Triangle { v0: 9, v1: 13, v2: 15, part: Part::Back },
    Triangle { v0: 9, v1: 15, v2: 11, part: Part::Flank },
    Triangle { v0: 11, v1: 15, v2: 14, part: Part::Belly },
    Triangle { v0: 11, v1: 14, v2: 10, part: Part::Belly },
    Triangle { v0: 10, v1: 14, v2: 16, part: Part::Belly },
    Triangle { v0: 10, v1: 16, v2: 12, part: Part::Belly },
    Triangle { v0: 12, v1: 16, v2: 13, part: Part::Flank },
    Triangle { v0: 12, v1: 13, v2: 9, part: Part::Back },
    // Dorsal fin (two-sided)
    Triangle { v0: 5, v1: 20, v2: 21, part: Part::Fin },
    Triangle { v0: 5, v1: 21, v2: 9, part: Part::Fin },
    Triangle { v0: 5, v1: 21, v2: 20, part: Part::Fin },
    Triangle { v0: 5, v1: 9, v2: 21, part: Part::Fin },
    // Ventral fin (two-sided)
    Triangle { v0: 6, v1: 22, v2: 10, part: Part::Fin },
    Triangle { v0: 6, v1: 10, v2: 22, part: Part::Fin },
    // Tail fin (two-sided)
    Triangle { v0: 13, v1: 17, v2: 18, part: Part::Tail },
    Triangle { v0: 13, v1: 18, v2: 14, part: Part::Tail },
    Triangle { v0: 14, v1: 18, v2: 19, part: Part::Tail },
    Triangle { v0: 13, v1: 18, v2: 17, part: Part::Tail },
    Triangle { v0: 13, v1: 14, v2: 18, part: Part::Tail },
    Triangle { v0: 14, v1: 19, v2: 18, part: Part::Tail },
    // Pectoral fins (two-sided)
    Triangle { v0: 3, v1: 23, v2: 7, part: Part::Fin },
    Triangle { v0: 3, v1: 7, v2: 23, part: Part::Fin },
    Triangle { v0: 4, v1: 8, v2: 24, part: Part::Fin },
    Triangle { v0: 4, v1: 24, v2: 8, part: Part::Fin },
];

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Rgb {
    let h = h - h.floor();
    let c = v * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let hp = h * 6.0;
    let (r, g, b) = if hp < 1.0 {
        (c, x, 0.0)
    } else if hp < 2.0 {
        (x, c, 0.0)
    } else if hp < 3.0 {
        (0.0, c, x)
    } else if hp < 4.0 {
        (0.0, x, c)
    } else if hp < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    Rgb::new(
        to_byte(((r + m) * 255.0).clamp(0.0, 255.0)),
        to_byte(((g + m) * 255.0).clamp(0.0, 255.0)),
        to_byte(((b + m) * 255.0).clamp(0.0, 255.0)),
    )
}

fn get_part_color(part: Part, shade: f32, base_hue: f32, color_mode: usize, solid_hue: f32) -> Rgb {
    if color_mode == 0 {
        // Goldfish
        match part {
            Part::Head | Part::Flank => Rgb::new(
                to_byte((240.0 * shade).clamp(0.0, 255.0)),
                to_byte((130.0 * shade).clamp(0.0, 255.0)),
                to_byte((25.0 * shade).clamp(0.0, 255.0)),
            ),
            Part::Back => Rgb::new(
                to_byte((220.0 * shade).clamp(0.0, 255.0)),
                to_byte((80.0 * shade).clamp(0.0, 255.0)),
                to_byte((10.0 * shade).clamp(0.0, 255.0)),
            ),
            Part::Belly => Rgb::new(
                to_byte((255.0 * shade).clamp(0.0, 255.0)),
                to_byte((200.0 * shade).clamp(0.0, 255.0)),
                to_byte((120.0 * shade).clamp(0.0, 255.0)),
            ),
            Part::Fin | Part::Tail => Rgb::new(
                to_byte((255.0 * shade).clamp(0.0, 255.0)),
                to_byte((150.0 * shade).clamp(0.0, 255.0)),
                to_byte((40.0 * shade).clamp(0.0, 255.0)),
            ),
        }
    } else if color_mode == 1 {
        // Rainbow
        let mut h = base_hue;
        match part {
            Part::Head => h = (h + 0.15) % 1.0,
            Part::Back => h = (h + 0.30) % 1.0,
            Part::Belly => h = (h + 0.45) % 1.0,
            Part::Tail => h = (h + 0.60) % 1.0,
            Part::Fin => h = (h + 0.75) % 1.0,
            Part::Flank => {}
        }
        let sat = if part == Part::Belly { 0.65 } else { 0.95 };
        hsv_to_rgb(h, sat, shade)
    } else {
        // Solid hue
        let h = solid_hue / 360.0;
        let sat = 0.85;
        let mut val = shade;
        if part == Part::Belly {
            val = (shade * 1.15).min(1.0);
        } else if part == Part::Back {
            val = shade * 0.85;
        }
        hsv_to_rgb(h, sat, val)
    }
}

pub struct RotatingFishEffect {
    values: [f32; 6],
    depth: Box<[[f32; WIDTH]; HEIGHT]>,
    phase: u64,
    previous_ms: u64,
    last_beat: bool,
    swim_phase: f32,
    beat_pulse: f32,
    started: bool,
    bubbles: [Bubble; BUBBLE_COUNT],
    rng: XorShift32,
}

impl Default for RotatingFishEffect {
    fn default() -> Self {
        Self {
            values: [0.0, 60.0, 100.0, 0.0, 35.0, 100.0],
            depth: alloc_boxed_2d(0.0),
            phase: 0,
            previous_ms: 0,
            last_beat: false,
            swim_phase: 0.0,
            beat_pulse: 0.0,
            started: false,
            bubbles: [Bubble::default(); BUBBLE_COUNT],
            rng: XorShift32::new(0xf158_c0de),
        }
    }
}

impl RotatingFishEffect {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for RotatingFishEffect {
    fn id(&self) -> &'static str {
        "rotating_fish"
    }

    fn name(&self) -> &'static str {
        "Rotating Fish"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "axis" => Some(self.values[0]),
            "speed" => Some(self.values[1]),
            "size" => Some(self.values[2]),
            "color_mode" => Some(self.values[3]),
            "hue" => Some(self.values[4]),
            "reactivity" => Some(self.values[5]),
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
            "axis" => self.values[0] = value,
            "speed" => self.values[1] = value,
            "size" => self.values[2] = value,
            "color_mode" => self.values[3] = value,
            "hue" => self.values[4] = value,
            "reactivity" => self.values[5] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.phase = 0;
        self.previous_ms = 0;
        self.last_beat = false;
        self.swim_phase = 0.0;
        self.beat_pulse = 0.0;
        self.started = false;
        for b in &mut self.bubbles {
            b.active = false;
        }
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::BLACK);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                self.depth[y][x] = 0.0;
            }
        }

        let elapsed = if self.started {
            context.now_ms.saturating_sub(self.previous_ms)
        } else {
            self.started = true;
            0
        };
        self.previous_ms = context.now_ms;

        // Advance spin angle (in millidegrees to prevent cadence-dependent drift)
        let speed_deg = self.values[1].max(0.0) as u64;
        self.phase = (self.phase + elapsed * speed_deg) % 360_000;
        let angle = (self.phase as f32) * 0.00001745329252;

        // Audio reactivity
        let reactivity = self.values[5] / 100.0;
        let has_audio = context.audio.available && reactivity > 0.001;
        if has_audio && context.audio.beat && !self.last_beat {
            self.beat_pulse = 1.0;
        }
        self.last_beat = context.audio.beat;

        if elapsed > 0 {
            let decay = (-(elapsed as f32) / 120.0).exp();
            self.beat_pulse *= decay;
        }

        let bass = if has_audio { audio_bass(context.audio) } else { 0.0 };
        let audio_drive = self.beat_pulse.max(bass) * reactivity;

        // Swimming tail wiggle
        let swim_rate = 0.008 + 0.016 * audio_drive;
        self.swim_phase += swim_rate * (elapsed as f32);

        // Size with beat puffing
        let puff = 1.0 + 0.18 * audio_drive.min(1.0);
        let scale = 23.0 * (self.values[2] / 100.0) * puff;

        // Bubble emission
        if has_audio && self.beat_pulse > 0.8 {
            for b in &mut self.bubbles {
                if !b.active {
                    b.active = true;
                    b.x = 15.5;
                    b.y = 15.5;
                    b.speed = 0.012 + 0.008 * (self.rng.next_u32() % 100) as f32 / 100.0;
                    break;
                }
            }
        }

        for b in &mut self.bubbles {
            if b.active {
                b.y -= b.speed * (elapsed as f32);
                b.x += 0.004 * (b.y * 0.8).sin() * (elapsed as f32);
                if b.y < 0.0 || b.x < 0.0 || b.x >= WIDTH as f32 {
                    b.active = false;
                }
            }
        }

        // Set rotation angles based on axis parameter
        let (yaw, pitch, roll): (f32, f32, f32) = if self.values[0] == 0.0 {
            // Horizontal rotation
            (angle, 0.20, 0.0)
        } else {
            // Vertical rotation
            (0.22, angle, 0.0)
        };

        let cy = yaw.cos();
        let sy = yaw.sin();
        let cp = pitch.cos();
        let sp = pitch.sin();
        let cr = roll.cos();
        let sr = roll.sin();

        // Rotation matrix M = Ry * Rz * Rx
        let m00 = cy * cp;
        let m01 = cy * -sp * cr + sy * sr;
        let m02 = cy * sp * sr + sy * cr;
        let m10 = sp;
        let m11 = cp * cr;
        let m12 = -cp * sr;
        let m20 = -sy * cp;
        let m21 = -sy * -sp * cr + cy * sr;
        let m22 = -sy * sp * sr + cy * cr;

        let cam_dist = 4.0;
        let mut v_rot = [Vec3::default(); VERTEX_COUNT];
        let mut sx = [0.0f32; VERTEX_COUNT];
        let mut sy = [0.0f32; VERTEX_COUNT];
        let mut iz = [0.0f32; VERTEX_COUNT];

        for i in 0..VERTEX_COUNT {
            let vx = BASE_VERTICES[i].x;
            let vy = BASE_VERTICES[i].y;
            let mut vz = BASE_VERTICES[i].z;

            // Lateral swimming bend on tail and rear body
            if vx < 0.2 {
                let bend_factor = (0.2 - vx).powf(1.25);
                let wiggle_amp = 0.16 + 0.12 * audio_drive.min(1.0);
                vz += bend_factor * wiggle_amp * self.swim_phase.sin();
            }

            let rx = m00 * vx + m01 * vy + m02 * vz;
            let ry = m10 * vx + m11 * vy + m12 * vz;
            let rz = m20 * vx + m21 * vy + m22 * vz;

            v_rot[i] = Vec3 { x: rx, y: ry, z: rz };
            let z_cam = (cam_dist - rz).max(0.5);
            let inv_z = 1.0 / z_cam;
            iz[i] = inv_z;
            sx[i] = 15.5 + rx * scale * inv_z;
            sy[i] = 15.5 - ry * scale * inv_z;
        }

        // Keep bubble spawn position near the snout (vertex 0)
        for b in &mut self.bubbles {
            if b.active && (b.y >= 15.4 && b.y <= 15.6) {
                b.x = sx[0];
                b.y = sy[0];
            }
        }

        let light = Vec3 { x: 0.45, y: 0.75, z: 0.48 };
        let l_len = (light.x * light.x + light.y * light.y + light.z * light.z).sqrt();
        let norm_light = Vec3 {
            x: light.x / l_len,
            y: light.y / l_len,
            z: light.z / l_len,
        };

        let color_mode = self.values[3] as usize;
        let solid_hue = self.values[4];

        for t in 0..TRIANGLE_COUNT {
            let i0 = TRIANGLES[t].v0;
            let i1 = TRIANGLES[t].v1;
            let i2 = TRIANGLES[t].v2;

            let p0x = sx[i0];
            let p0y = sy[i0];
            let z0 = iz[i0];

            let p1x = sx[i1];
            let p1y = sy[i1];
            let z1 = iz[i1];

            let p2x = sx[i2];
            let p2y = sy[i2];
            let z2 = iz[i2];

            let e1x = v_rot[i1].x - v_rot[i0].x;
            let e1y = v_rot[i1].y - v_rot[i0].y;
            let e1z = v_rot[i1].z - v_rot[i0].z;

            let e2x = v_rot[i2].x - v_rot[i0].x;
            let e2y = v_rot[i2].y - v_rot[i0].y;
            let e2z = v_rot[i2].z - v_rot[i0].z;

            let mut nx = e1y * e2z - e1z * e2y;
            let mut ny = e1z * e2x - e1x * e2z;
            let mut nz = e1x * e2y - e1y * e2x;
            let nlen = (nx * nx + ny * ny + nz * nz).sqrt();
            if nlen < 1e-5 {
                continue;
            }
            nx /= nlen;
            ny /= nlen;
            nz /= nlen;

            let area = (p1x - p0x) * (p2y - p0y) - (p1y - p0y) * (p2x - p0x);
            if area.abs() < 1e-4 {
                continue;
            }
            let inv_area = 1.0 / area;

            let min_x = ((p0x.min(p1x).min(p2x)).floor() as i32).clamp(0, (WIDTH - 1) as i32);
            let max_x = ((p0x.max(p1x).max(p2x)).ceil() as i32).clamp(0, (WIDTH - 1) as i32);
            let min_y = ((p0y.min(p1y).min(p2y)).floor() as i32).clamp(0, (HEIGHT - 1) as i32);
            let max_y = ((p0y.max(p1y).max(p2y)).ceil() as i32).clamp(0, (HEIGHT - 1) as i32);

            let dot_light = (nx * norm_light.x + ny * norm_light.y + nz * norm_light.z).abs();
            let shade = 0.35 + 0.65 * dot_light;

            let base_hue = (0.5
                + (v_rot[i0].x + v_rot[i1].x + v_rot[i2].x) * 0.15
                + (context.now_ms as f32) * 0.0001)
                % 1.0;
            let col = get_part_color(TRIANGLES[t].part, shade, base_hue, color_mode, solid_hue);

            for py in min_y..=max_y {
                let py_f = py as f32;
                for px in min_x..=max_x {
                    let px_f = px as f32;
                    let w0 = ((p1x - px_f) * (p2y - py_f) - (p1y - py_f) * (p2x - px_f)) * inv_area;
                    let w1 = ((p2x - px_f) * (p0y - py_f) - (p2y - py_f) * (p0x - px_f)) * inv_area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 >= -0.001 && w1 >= -0.001 && w2 >= -0.001 {
                        let pz = w0 * z0 + w1 * z1 + w2 * z2;
                        let uy = py as usize;
                        let ux = px as usize;
                        if pz > self.depth[uy][ux] {
                            self.depth[uy][ux] = pz;
                            frame.set(ux, uy, col);
                        }
                    }
                }
            }
        }

        // Render eyes when facing camera
        for &e in &[25usize, 26usize] {
            if v_rot[e].z > -0.25 {
                let px = sx[e].round() as i32;
                let py = sy[e].round() as i32;
                if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                    let ux = px as usize;
                    let uy = py as usize;
                    if iz[e] >= self.depth[uy][ux] - 0.05 {
                        self.depth[uy][ux] = iz[e] + 0.01;
                        frame.set(ux, uy, Rgb::new(255, 255, 255));
                    }
                }
            }
        }

        // Render bubbles
        for b in &self.bubbles {
            if b.active {
                let bx = b.x.round() as i32;
                let by = b.y.round() as i32;
                if bx >= 0 && bx < WIDTH as i32 && by >= 0 && by < HEIGHT as i32 {
                    let ux = bx as usize;
                    let uy = by as usize;
                    if self.depth[uy][ux] < 0.35 {
                        frame.set(ux, uy, Rgb::new(120, 200, 255));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matrix_core::AudioSnapshot;

    #[test]
    fn test_rotating_fish_basics() {
        let mut effect = RotatingFishEffect::new();
        assert_eq!(effect.id(), "rotating_fish");
        assert_eq!(effect.name(), "Rotating Fish");
        assert_eq!(effect.parameters().len(), 6);

        assert_eq!(effect.parameter("axis"), Some(0.0));
        assert_eq!(effect.parameter("speed"), Some(60.0));
        assert_eq!(effect.parameter("size"), Some(100.0));
        assert_eq!(effect.parameter("color_mode"), Some(0.0));
        assert_eq!(effect.parameter("hue"), Some(35.0));
        assert_eq!(effect.parameter("reactivity"), Some(100.0));

        let audio = AudioSnapshot::default();
        let ctx0 = RenderContext {
            now_ms: 0,
            delta_seconds: 0.016,
            audio: &audio,
            color: Rgb::WHITE,
            text: "",
            scroll_speed: 10.0,
        };

        let mut frame0 = Frame::default();
        effect.render(&mut frame0, &ctx0);
        let lit0 = frame0.pixels.iter().filter(|&&p| p != Rgb::BLACK).count();
        assert!(lit0 > 40 && lit0 < 400, "Lit pixel count out of expected range: {}", lit0);
        assert_ne!(frame0.pixels[15 * WIDTH + 15], Rgb::BLACK, "Center must be lit");
        assert_eq!(frame0.pixels[0], Rgb::BLACK, "Top-left margin must be dark");
        assert_eq!(frame0.pixels[HEIGHT * WIDTH - 1], Rgb::BLACK, "Bottom-right margin must be dark");

        // Advance time - frame changes
        let ctx1 = RenderContext {
            now_ms: 1000,
            delta_seconds: 1.0,
            audio: &audio,
            color: Rgb::WHITE,
            text: "",
            scroll_speed: 10.0,
        };
        let mut frame1 = Frame::default();
        effect.render(&mut frame1, &ctx1);
        assert_ne!(frame0.pixels, frame1.pixels, "Frame must change as fish rotates");
    }
}
