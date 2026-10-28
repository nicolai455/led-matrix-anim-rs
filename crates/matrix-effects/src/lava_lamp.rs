use crate::util::{audio_bass, audio_mid, audio_treble, clamp01, hsv_to_rgb, to_byte};
use matrix_core::{
    Effect, Frame, ParameterDescriptor, ParameterKind, RenderContext, Rgb, HEIGHT, WIDTH,
};

const PALETTES: &[&str] = &["Groovy Lava", "Cyber Violet", "Toxic Lime", "Rainbow"];

const PARAMETERS: [ParameterDescriptor; 5] = [
    ParameterDescriptor {
        id: "speed",
        label: "Convection speed",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 100.0,
        step: 1.0,
        default: 35.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "blobs",
        label: "Wax blobs",
        kind: ParameterKind::Number,
        min: 3.0,
        max: 6.0,
        step: 1.0,
        default: 4.0,
        options: &[],
        unit: "",
    },
    ParameterDescriptor {
        id: "viscosity",
        label: "Wax elasticity",
        kind: ParameterKind::Number,
        min: 10.0,
        max: 90.0,
        step: 1.0,
        default: 50.0,
        options: &[],
        unit: "%",
    },
    ParameterDescriptor {
        id: "palette",
        label: "Color fluid",
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

const MAX_BLOBS: usize = 6;

#[derive(Clone, Copy, Debug)]
struct WaxBlob {
    x: f32,
    y: f32,
    vy: f32,
    radius: f32,
    heat: f32, // 1.0 = hot (rising), 0.0 = cold (sinking)
}

pub struct LavaLampEffect {
    values: [f32; 5],
    blobs: [WaxBlob; MAX_BLOBS],
    color_cycle: f32,
    bubble_phase: f32,
}

impl Default for LavaLampEffect {
    fn default() -> Self {
        Self {
            values: [35.0, 4.0, 50.0, 3.0, 1.0],
            blobs: [
                WaxBlob { x: 15.0, y: 24.0, vy: -4.0, radius: 4.2, heat: 1.0 },
                WaxBlob { x: 17.0, y: 18.0, vy: -3.0, radius: 3.6, heat: 0.8 },
                WaxBlob { x: 14.0, y: 10.0, vy: 2.5, radius: 4.0, heat: 0.2 },
                WaxBlob { x: 18.0, y: 6.0, vy: 3.5, radius: 3.2, heat: 0.1 },
                WaxBlob { x: 16.0, y: 22.0, vy: -2.0, radius: 3.0, heat: 0.9 },
                WaxBlob { x: 15.0, y: 14.0, vy: 1.5, radius: 2.8, heat: 0.4 },
            ],
            color_cycle: 0.0,
            bubble_phase: 0.0,
        }
    }
}

impl LavaLampEffect {
    pub fn new() -> Self {
        Self::default()
    }

    fn bottle_half_width(y: f32) -> f32 {
        // Tapered glass bottle profile:
        // Top cap: y=0..3 (width ~5)
        // Upper neck: y=3..8 (slender width ~6..7)
        // Main belly: y=8..24 (curves outwards to width ~10.5)
        // Base skirt: y=24..31 (metallic base width ~12)
        if y < 4.0 {
            5.0
        } else if y < 9.0 {
            5.0 + (y - 4.0) * 0.4
        } else if y < 24.0 {
            7.0 + 3.8 * ((y - 9.0) / 15.0 * std::f32::consts::PI).sin()
        } else {
            8.5 + (y - 24.0) * 0.45
        }
    }

    fn liquid_and_wax_color(pal: usize, color_cycle: f32, field_strength: f32, y_norm: f32) -> Rgb {
        let is_wax = field_strength >= 1.0;
        let wax_core = clamp01(field_strength - 1.0);

        match pal {
            0 => {
                // Groovy Lava: Deep ruby-orange wax inside glowing translucent amber fluid
                if is_wax {
                    let r = 255;
                    let g = to_byte(40.0 + 170.0 * wax_core);
                    let b = to_byte(10.0 + 50.0 * wax_core);
                    Rgb::new(r, g, b)
                } else {
                    let fluid_bright = (0.25 + 0.75 * field_strength).clamp(0.1, 0.9);
                    let r = to_byte(220.0 * fluid_bright);
                    let g = to_byte(140.0 * fluid_bright);
                    let b = to_byte(20.0 * fluid_bright);
                    Rgb::new(r, g, b)
                }
            }
            1 => {
                // Cyber Violet: Fluorescent hot pink wax in deep indigo/navy fluid
                if is_wax {
                    let r = 255;
                    let g = to_byte(20.0 + 140.0 * wax_core);
                    let b = to_byte(180.0 + 75.0 * wax_core);
                    Rgb::new(r, g, b)
                } else {
                    let fluid_bright = (0.2 + 0.8 * field_strength).clamp(0.1, 0.85);
                    let r = to_byte(30.0 * fluid_bright);
                    let g = to_byte(10.0 * fluid_bright);
                    let b = to_byte(180.0 * fluid_bright);
                    Rgb::new(r, g, b)
                }
            }
            2 => {
                // Toxic Lime: Acid lime wax in glowing electric cyan fluid
                if is_wax {
                    let r = to_byte(120.0 + 135.0 * wax_core);
                    let g = 255;
                    let b = to_byte(20.0 + 100.0 * wax_core);
                    Rgb::new(r, g, b)
                } else {
                    let fluid_bright = (0.25 + 0.75 * field_strength).clamp(0.1, 0.9);
                    let r = to_byte(10.0 * fluid_bright);
                    let g = to_byte(200.0 * fluid_bright);
                    let b = to_byte(240.0 * fluid_bright);
                    Rgb::new(r, g, b)
                }
            }
            _ => {
                // Rainbow Fluid
                let h_fluid = (color_cycle + y_norm * 0.3) % 1.0;
                let h_wax = (h_fluid + 0.5) % 1.0;
                if is_wax {
                    hsv_to_rgb(h_wax, 0.95, (0.7 + 0.3 * wax_core).min(1.0))
                } else {
                    hsv_to_rgb(h_fluid, 0.85, 0.2 + 0.5 * field_strength)
                }
            }
        }
    }
}

impl Effect for LavaLampEffect {
    fn id(&self) -> &'static str {
        "lava_lamp"
    }

    fn name(&self) -> &'static str {
        "Liquid Lava Lamp"
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &PARAMETERS
    }
    fn static_parameters(&self) -> Option<&'static [ParameterDescriptor]> { Some(&PARAMETERS) }

    fn parameter(&self, id: &str) -> Option<f32> {
        match id {
            "speed" => Some(self.values[0]),
            "blobs" => Some(self.values[1]),
            "viscosity" => Some(self.values[2]),
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
            "blobs" => self.values[1] = value,
            "viscosity" => self.values[2] = value,
            "palette" => self.values[3] = value,
            "audio_reactive" => self.values[4] = value,
            _ => return Err(format!("effect '{}' has no parameter '{}'", self.id(), id)),
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.blobs = [
            WaxBlob { x: 15.0, y: 24.0, vy: -4.0, radius: 4.2, heat: 1.0 },
            WaxBlob { x: 17.0, y: 18.0, vy: -3.0, radius: 3.6, heat: 0.8 },
            WaxBlob { x: 14.0, y: 10.0, vy: 2.5, radius: 4.0, heat: 0.2 },
            WaxBlob { x: 18.0, y: 6.0, vy: 3.5, radius: 3.2, heat: 0.1 },
            WaxBlob { x: 16.0, y: 22.0, vy: -2.0, radius: 3.0, heat: 0.9 },
            WaxBlob { x: 15.0, y: 14.0, vy: 1.5, radius: 2.8, heat: 0.4 },
        ];
        self.color_cycle = 0.0;
        self.bubble_phase = 0.0;
    }

    fn deactivate(&mut self) {
        self.reset();
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        frame.clear(Rgb::new(8, 8, 12)); // Dim room background

        let dt = context.delta_seconds.clamp(0.001, 0.1);
        let audio_active = self.values[4] > 0.5 && context.audio.available;
        let bass = if audio_active { audio_bass(context.audio) } else { 0.0 };
        let mid = if audio_active { audio_mid(context.audio) } else { 0.0 };
        let treble = if audio_active { audio_treble(context.audio) } else { 0.0 };
        let beat = audio_active && context.audio.beat;

        let speed = (self.values[0] / 30.0) * (1.0 + bass * 0.9);
        self.color_cycle = (self.color_cycle + dt * 0.14) % 1.0;
        self.bubble_phase = (self.bubble_phase + dt * 6.0) % (std::f32::consts::PI * 2.0);

        let blob_count = (self.values[1].round() as usize).clamp(3, MAX_BLOBS);
        let pal = self.values[3] as usize;
        let elasticity = self.values[2] / 50.0;

        let center_x = 15.5f32;

        // 1. Simulate Wax Blob Thermodynamics & Convection
        for i in 0..blob_count {
            let blob = &mut self.blobs[i];

            // Bottom coil heating vs top surface cooling
            if blob.y > 23.0 {
                blob.heat = (blob.heat + dt * 1.5).min(1.0);
            } else if blob.y < 7.0 {
                blob.heat = (blob.heat - dt * 1.2).max(0.0);
            }

            // Buoyancy force: hot wax rises (negative vy), cold wax sinks (positive vy)
            let buoyancy = (blob.heat - 0.5) * -12.0 * speed;
            let beat_kick = if beat { -8.0 } else { 0.0 };

            blob.vy += (buoyancy + beat_kick - blob.vy * 1.8) * dt * elasticity;
            blob.y += blob.vy * dt;

            // Horizontal drift constrained by bottle profile
            let half_w = (Self::bottle_half_width(blob.y) - blob.radius - 1.0).max(0.0);
            let min_x = center_x - half_w;
            let max_x = center_x + half_w;
            blob.x = blob.x.clamp(min_x, max_x);

            // Rebound at top/bottom extremes
            if blob.y <= 5.0 {
                blob.y = 5.0;
                blob.vy = blob.vy.abs();
            } else if blob.y >= 25.0 {
                blob.y = 25.0;
                blob.vy = -blob.vy.abs();
            }
        }

        // 2. Render Bottle Geometry & Liquid Field
        for y in 0..HEIGHT {
            let yf = y as f32;
            let half_w = Self::bottle_half_width(yf);
            let left_wall = center_x - half_w;
            let right_wall = center_x + half_w;

            for x in 0..WIDTH {
                let xf = x as f32;

                if xf >= left_wall && xf <= right_wall {
                    // Top Cap & Base metallic hardware
                    let is_top_cap = yf <= 3.0;
                    let is_base = yf >= 25.0;

                    if is_top_cap || is_base {
                        // Brushed silver / gold metallic finish
                        let nx = (xf - center_x) / half_w;
                        let specular = (1.0 - nx.abs()).powi(3);
                        let metal_bright = to_byte(80.0 + 175.0 * specular + mid * 40.0);
                        frame.set(x, y, Rgb::new(metal_bright, metal_bright, to_byte(metal_bright as f32 * 1.1)));
                    } else {
                        // Glass interior liquid: compute Metaball Wax Field
                        let mut field = 0.0f32;
                        for i in 0..blob_count {
                            let b = &self.blobs[i];
                            let dx = xf - b.x;
                            let dy = yf - b.y;
                            let d2 = dx * dx + dy * dy;
                            let r2 = b.radius * b.radius;
                            field += (r2 / (d2 + 1.2)).powf(1.4);
                        }

                        // Glass edge glass reflection highlight (left edge)
                        let dist_to_left = (xf - left_wall).abs();
                        let is_glass_glint = dist_to_left < 1.2 && yf > 6.0 && yf < 23.0;

                        let mut pixel_c = Self::liquid_and_wax_color(pal, self.color_cycle, field, yf / 32.0);

                        // Micro-sparkle bubbles from treble
                        #[allow(clippy::manual_is_multiple_of)]
                        if audio_active && treble > 0.4 && (xf as usize * 7 + yf as usize * 13) % 23 == 0 {
                            pixel_c = Rgb::WHITE;
                        }

                        if is_glass_glint {
                            pixel_c.r = pixel_c.r.saturating_add(70);
                            pixel_c.g = pixel_c.g.saturating_add(90);
                            pixel_c.b = pixel_c.b.saturating_add(110);
                        }

                        frame.set(x, y, pixel_c);
                    }
                }
            }
        }
    }
}
