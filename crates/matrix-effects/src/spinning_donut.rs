use crate::util::{alloc_boxed_2d, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};
use std::f32::consts::PI;

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

fn donut_color(hue: f32, brightness: f32, white: bool) -> Rgb {
    if white {
        let v = to_byte(255.0 * brightness);
        return Rgb::new(v, v, v);
    }
    let mut h = hue - hue.floor();
    h *= 6.0;
    let x = 1.0 - ((h % 2.0) - 1.0).abs();
    let low = 0.12f32;
    let high = brightness * 255.0;
    let mid = low + (1.0 - low) * x;

    let (r, g, b) = match h as usize {
        0 => (high, high * mid, high * low),
        1 => (high * mid, high, high * low),
        2 => (high * low, high, high * mid),
        3 => (high * low, high * mid, high),
        4 => (high * mid, high * low, high),
        _ => (high, high * low, high * mid),
    };
    Rgb::new(to_byte(r), to_byte(g), to_byte(b))
}

pub struct SpinningDonutEffect {
    values: [f32; 4],
    phase: f32,
    depth: Box<[[f32; WIDTH]; HEIGHT]>,
}

impl Default for SpinningDonutEffect {
    fn default() -> Self {
        Self {
            values: [45.0, 100.0, 0.0, 30.0],
            phase: 0.0,
            depth: alloc_boxed_2d(0.0),
        }
    }
}

impl SpinningDonutEffect {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for SpinningDonutEffect {
    fn id(&self) -> &'static str {
        "spinning_donut"
    }

    fn name(&self) -> &'static str {
        "Spinning Donut"
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
        self.depth.as_mut().fill([0.0; WIDTH]);
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::BLACK);
        self.depth.as_mut().fill([0.0; WIDTH]);

        let dt = context.delta_seconds.clamp(0.0, 1.0);
        let speed_deg = self.values[0];
        self.phase = (self.phase + dt * speed_deg * (PI / 180.0)) % (2.0 * PI);

        let angle = self.phase;
        let a = 0.35 + angle;
        let b = angle * 0.5;
        let ca = a.cos();
        let sa = a.sin();
        let cb = b.cos();
        let sb = b.sin();

        let scale = 34.0 * self.values[1] / 100.0;
        let color_mode = self.values[2] as usize;
        let solid_hue = self.values[3] / 360.0;

        let two_pi = 2.0 * PI;

        for ring in 0..128 {
            let theta = (ring as f32) * (two_pi / 128.0);
            let ct = theta.cos();
            let st = theta.sin();
            let hue = if color_mode == 0 {
                (ring as f32) / 128.0
            } else {
                solid_hue
            };

            for tube in 0..48 {
                let phi = (tube as f32) * (two_pi / 48.0);
                let cp = phi.cos();
                let sp = phi.sin();
                let radius = 2.0 + 0.8 * cp;

                let x = radius * ct;
                let y = radius * st * ca - 0.8 * sp * sa;
                let z = radius * st * sa + 0.8 * sp * ca;
                let rx = x * cb + z * sb;
                let rz = -x * sb + z * cb;

                let inverse_z = 1.0 / (9.0 - rz);
                let px = (15.5 + scale * rx * inverse_z + 0.5).floor() as i32;
                let py = (15.5 + scale * y * inverse_z + 0.5).floor() as i32;

                if px < 0 || py < 0 || px >= WIDTH as i32 || py >= HEIGHT as i32 {
                    continue;
                }
                let ux = px as usize;
                let uy = py as usize;

                if inverse_z <= self.depth[uy][ux] {
                    continue;
                }
                self.depth[uy][ux] = inverse_z;

                let nx = cp * ct;
                let ny = cp * st * ca - sp * sa;
                let nz = cp * st * sa + sp * ca;
                let light = -0.35 * (nx * cb + nz * sb) - 0.45 * ny + 0.82 * (-nx * sb + nz * cb);
                let shade = 0.15 + 0.85 * light.clamp(0.0, 1.0);

                frame.set(ux, uy, donut_color(hue, shade, color_mode == 2));
            }
        }
    }
}
