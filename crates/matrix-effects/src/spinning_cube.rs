use crate::util::to_byte;
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const COLOR_MODES: &[&str] = &["Rainbow", "Solid", "White"];

const PARAMETERS: [ParameterDescriptor; 5] = [
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
    ParameterDescriptor {
        id: "wireframe",
        label: "Wireframe",
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

fn draw_line(frame: &mut Frame, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgb) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    let mut x = x0;
    let mut y = y0;

    loop {
        if x >= 0 && x < WIDTH as i32 && y >= 0 && y < HEIGHT as i32 {
            frame.set(x as usize, y as usize, color);
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
}

pub struct SpinningCubeEffect {
    values: [f32; 5],
    phase: f32, // angle in radians
}

impl Default for SpinningCubeEffect {
    fn default() -> Self {
        Self {
            values: [45.0, 100.0, 0.0, 30.0, 0.0],
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
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "size" => Some(self.values[1]),
            "color_mode" => Some(self.values[2]),
            "hue" => Some(self.values[3]),
            "wireframe" => Some(self.values[4]),
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
            "wireframe" => self.values[4] = value,
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

        let color_mode = self.values[2] as usize;
        let solid_hue = self.values[3] / 360.0;
        let wireframe = self.values[4] != 0.0;

        if wireframe {
            frame.clear(Rgb::BLACK);

            const CUBE_VERTICES: [Vec3; 8] = [
                Vec3 {
                    x: -1.0,
                    y: -1.0,
                    z: -1.0,
                },
                Vec3 {
                    x: 1.0,
                    y: -1.0,
                    z: -1.0,
                },
                Vec3 {
                    x: 1.0,
                    y: 1.0,
                    z: -1.0,
                },
                Vec3 {
                    x: -1.0,
                    y: 1.0,
                    z: -1.0,
                },
                Vec3 {
                    x: -1.0,
                    y: -1.0,
                    z: 1.0,
                },
                Vec3 {
                    x: 1.0,
                    y: -1.0,
                    z: 1.0,
                },
                Vec3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
                Vec3 {
                    x: -1.0,
                    y: 1.0,
                    z: 1.0,
                },
            ];

            const CUBE_EDGES: [(usize, usize); 12] = [
                (0, 1),
                (1, 2),
                (2, 3),
                (3, 0),
                (4, 5),
                (5, 6),
                (6, 7),
                (7, 4),
                (0, 4),
                (1, 5),
                (2, 6),
                (3, 7),
            ];

            // Project 8 cube vertices to screen space
            let mut proj = [(0.0f32, 0.0f32, 0.0f32); 8];
            for (i, v) in CUBE_VERTICES.iter().enumerate() {
                let px = v.x * axes[0].x + v.y * axes[1].x + v.z * axes[2].x;
                let py = v.x * axes[0].y + v.y * axes[1].y + v.z * axes[2].y;
                let pz = v.x * axes[0].z + v.y * axes[1].z + v.z * axes[2].z;
                let dist = 6.0 - pz;
                let sx = 15.5 + scale * px / dist;
                let sy = 15.5 + scale * py / dist;
                proj[i] = (sx, sy, pz);
            }

            // Draw edges sorted by average depth (farthest first so nearer edges paint on top)
            let mut edge_order: [usize; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
            edge_order.sort_by(|&a, &b| {
                let (u1, v1) = CUBE_EDGES[a];
                let (u2, v2) = CUBE_EDGES[b];
                let z1 = proj[u1].2 + proj[v1].2;
                let z2 = proj[u2].2 + proj[v2].2;
                z1.partial_cmp(&z2).unwrap_or(std::cmp::Ordering::Equal)
            });

            for &edge_idx in &edge_order {
                let (u, v) = CUBE_EDGES[edge_idx];
                let x0 = (proj[u].0 + 0.5).floor() as i32;
                let y0 = (proj[u].1 + 0.5).floor() as i32;
                let x1 = (proj[v].0 + 0.5).floor() as i32;
                let y1 = (proj[v].1 + 0.5).floor() as i32;

                let avg_z = (proj[u].2 + proj[v].2) * 0.5;
                let z_norm = ((avg_z + 1.732) / 3.464).clamp(0.0, 1.0);
                let shade = 0.30 + 0.60 * z_norm;

                let hue = if color_mode == 0 {
                    edge_idx as f32 / 12.0
                } else {
                    solid_hue
                };

                let color = cube_color(hue, shade, color_mode == 2);
                draw_line(frame, x0, y0, x1, y1, color);
            }

            // Draw 8 vertices on top of edges sorted by depth
            let mut vertex_order: [usize; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
            vertex_order.sort_by(|&a, &b| {
                proj[a].2.partial_cmp(&proj[b].2).unwrap_or(std::cmp::Ordering::Equal)
            });

            for &v_idx in &vertex_order {
                let vx = (proj[v_idx].0 + 0.5).floor() as i32;
                let vy = (proj[v_idx].1 + 0.5).floor() as i32;
                if vx >= 0 && vx < WIDTH as i32 && vy >= 0 && vy < HEIGHT as i32 {
                    let z_norm = ((proj[v_idx].2 + 1.732) / 3.464).clamp(0.0, 1.0);
                    let v_byte = to_byte(200.0 + 55.0 * z_norm);
                    let v_color = Rgb::new(v_byte, v_byte, v_byte);
                    frame.set(vx as usize, vy as usize, v_color);
                }
            }
            return;
        }

        let origin = [
            dot(camera, axes[0]),
            dot(camera, axes[1]),
            dot(camera, axes[2]),
        ];

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

#[cfg(test)]
mod tests {
    use super::*;
    use matrix_core::AudioSnapshot;

    #[test]
    fn test_spinning_cube_wireframe_toggle_and_render() {
        let mut effect = SpinningCubeEffect::new();
        assert_eq!(effect.parameter("wireframe"), Some(0.0));

        let audio = AudioSnapshot::default();
        let ctx = RenderContext {
            now_ms: 1000,
            delta_seconds: 0.016,
            audio: &audio,
            color: Rgb::WHITE,
            text: "",
            scroll_speed: 10.0,
        };

        let mut solid_frame = Frame::default();
        effect.render(&mut solid_frame, &ctx);
        let solid_lit = solid_frame.pixels.iter().filter(|&&p| p != Rgb::BLACK).count();
        assert!(solid_lit > 0, "Solid cube must light pixels");

        assert!(effect.set_parameter("wireframe", 1.0).is_ok());
        assert_eq!(effect.parameter("wireframe"), Some(1.0));

        let mut wireframe_frame = Frame::default();
        effect.render(&mut wireframe_frame, &ctx);
        let wireframe_lit = wireframe_frame.pixels.iter().filter(|&&p| p != Rgb::BLACK).count();
        assert!(wireframe_lit > 0, "Wireframe cube must light pixels");

        // Wireframe should have unlit pixels inside faces (less dense than solid)
        assert!(wireframe_lit < solid_lit, "Wireframe should be less dense than solid cube");

        // Verify invalid parameter values are rejected
        assert!(effect.set_parameter("wireframe", 2.0).is_err());
        assert!(effect.set_parameter("wireframe", 0.5).is_err());
    }
}
