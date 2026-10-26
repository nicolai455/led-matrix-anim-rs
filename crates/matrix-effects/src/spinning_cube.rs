use crate::util::to_byte;
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const COLOR_MODES: &[&str] = &["Rainbow", "Solid", "White"];

const PARAMETERS: [ParameterDescriptor; 4] = [
    ParameterDescriptor {
        id: "speed",
        label: "Spin speed",
        kind: ParameterKind::Number,
        min: 0.0,
        max: 180.0,
        step: 1.0,
        default: 45.0,
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
        default: 30.0,
        options: &[],
        unit: "degrees",
    },
];

#[derive(Clone, Copy)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

#[inline]
fn dot(a: Vec3, b: Vec3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn cube_color(hue: f32, shade: f32, white: bool) -> Rgb {
    let v = 255.0 * shade;
    if white {
        let b = to_byte(v);
        return Rgb::new(b, b, b);
    }
    let mut h = hue - hue.floor();
    h *= 6.0;
    let low = v * 0.12;
    let mid = low + (v - low) * (1.0 - ((h % 2.0) - 1.0).abs());

    let (r, g, b) = match h as usize {
        0 => (v, mid, low),
        1 => (mid, v, low),
        2 => (low, v, mid),
        3 => (low, mid, v),
        4 => (mid, low, v),
        _ => (v, low, mid),
    };
    Rgb::new(to_byte(r), to_byte(g), to_byte(b))
}

pub struct SpinningCubeEffect {
    values: [f32; 4],
    phase: f32, // angle in radians
}

impl Default for SpinningCubeEffect {
    fn default() -> Self {
        Self {
            values: [45.0, 100.0, 0.0, 30.0],
            phase: 0.0,
        }
    }
}

impl SpinningCubeEffect {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for SpinningCubeEffect {
    fn id(&self) -> &'static str {
        "spinning_cube"
    }

    fn name(&self) -> &'static str {
        "Spinning Cube"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "size" => Some(self.values[1]),
            "color_mode" => Some(self.values[2]),
            "hue" => Some(self.values[3]),
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
            "size" => self.values[1] = value,
            "color_mode" => self.values[2] = value,
            "hue" => self.values[3] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.phase = 0.0;
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let dt = context.delta_seconds.clamp(0.0, 1.0);
        let speed_deg = self.values[0];
        self.phase = (self.phase + dt * speed_deg * (std::f32::consts::PI / 180.0))
            % (2.0 * std::f32::consts::PI);

        let angle = self.phase;
        let a = 0.4 + angle;
        let b = 0.55 + angle * 0.5;
        let ca = a.cos();
        let sa = a.sin();
        let cb = b.cos();
        let sb = b.sin();

        let axes = [
            Vec3 {
                x: cb,
                y: 0.0,
                z: -sb,
            },
            Vec3 {
                x: sa * sb,
                y: ca,
                z: sa * cb,
            },
            Vec3 {
                x: ca * sb,
                y: -sa,
                z: ca * cb,
            },
        ];
        let camera = Vec3 {
            x: 0.0,
            y: 0.0,
            z: 6.0,
        };
        let light = Vec3 {
            x: -0.35,
            y: -0.45,
            z: 0.82,
        };
        let scale = 40.0 * self.values[1] / 100.0;

        let origin = [
            dot(camera, axes[0]),
            dot(camera, axes[1]),
            dot(camera, axes[2]),
        ];

        let color_mode = self.values[2] as usize;
        let solid_hue = self.values[3] / 360.0;

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let ray = Vec3 {
                    x: (x as f32 - 15.5) / scale,
                    y: (y as f32 - 15.5) / scale,
                    z: -1.0,
                };
                let mut near = 0.0f32;
                let mut far = 100.0f32;
                let mut face = -1i32;
                let mut sign = 1.0f32;
                let mut hit = true;

                for i in 0..3 {
                    let d = dot(ray, axes[i]);
                    if d.abs() < 0.000001 {
                        if origin[i].abs() > 1.0 {
                            hit = false;
                            break;
                        }
                        continue;
                    }
                    let mut entry = (-1.0 - origin[i]) / d;
                    let mut exit = (1.0 - origin[i]) / d;
                    let normal_sign = if d > 0.0 { -1.0 } else { 1.0 };
                    if entry > exit {
                        std::mem::swap(&mut entry, &mut exit);
                    }
                    if entry > near {
                        near = entry;
                        face = i as i32;
                        sign = normal_sign;
                    }
                    far = far.min(exit);
                    if near > far {
                        hit = false;
                        break;
                    }
                }

                if !hit || face < 0 {
                    frame.set(x, y, Rgb::BLACK);
                    continue;
                }

                let light_dot = sign * dot(axes[face as usize], light);
                let shade = 0.2 + 0.8 * light_dot.clamp(0.0, 1.0);
                let hue = if color_mode == 0 {
                    (face as f32 * 2.0 + if sign > 0.0 { 1.0 } else { 0.0 }) / 6.0
                } else {
                    solid_hue
                };

                frame.set(x, y, cube_color(hue, shade, color_mode == 2));
            }
        }
    }
}
