use serde::{Deserialize, Serialize};

pub const WIDTH: usize = 32;
pub const HEIGHT: usize = 32;
pub const PIXELS: usize = WIDTH * HEIGHT; // 1024

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };

    #[inline]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// HSV to RGB conversion.
    /// `h` is hue in 0.0..1.0 (wraps around).
    /// `s` is saturation in 0.0..1.0 (clamped).
    /// `v` is value/brightness in 0.0..1.0 (clamped).
    pub fn hsv(h: f32, s: f32, v: f32) -> Self {
        let v = if !v.is_finite() || v <= 0.0 {
            return Self::BLACK;
        } else if v >= 1.0 {
            1.0
        } else {
            v
        };

        let s = if !s.is_finite() || s <= 0.0 {
            let val = (v * 255.0 + 0.5) as u8;
            return Self::new(val, val, val);
        } else if s >= 1.0 {
            1.0
        } else {
            s
        };

        let h_fract = h.fract();
        let h = if h_fract < 0.0 {
            h_fract + 1.0
        } else {
            h_fract
        };
        let h_deg = h * 6.0;
        let c = v * s;
        let x = c * (1.0 - ((h_deg % 2.0) - 1.0).abs());
        let m = v - c;

        let (r1, g1, b1) = match h_deg as usize {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };

        Self::new(
            ((r1 + m) * 255.0 + 0.5) as u8,
            ((g1 + m) * 255.0 + 0.5) as u8,
            ((b1 + m) * 255.0 + 0.5) as u8,
        )
    }

    /// Scale RGB brightness by a factor in 0.0..=1.0 (or > 1.0 clamped to 255).
    #[inline]
    pub fn scale(self, factor: f32) -> Self {
        if factor <= 0.0 || !factor.is_finite() {
            return Self::BLACK;
        }
        let clamp_byte = |val: u8| -> u8 {
            let scaled = val as f32 * factor + 0.5;
            if scaled >= 255.0 {
                255
            } else {
                scaled as u8
            }
        };
        Self::new(clamp_byte(self.r), clamp_byte(self.g), clamp_byte(self.b))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub pixels: [Rgb; PIXELS],
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            pixels: [Rgb::BLACK; PIXELS],
        }
    }
}

impl Frame {
    #[inline]
    pub const fn new() -> Self {
        Self {
            pixels: [Rgb::BLACK; PIXELS],
        }
    }

    #[inline]
    pub fn clear(&mut self, color: Rgb) {
        self.pixels.fill(color);
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, color: Rgb) {
        if x < WIDTH && y < HEIGHT {
            self.pixels[y * WIDTH + x] = color;
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> Rgb {
        if x < WIDTH && y < HEIGHT {
            self.pixels[y * WIDTH + x]
        } else {
            Rgb::BLACK
        }
    }

    /// In-place saturating additive blending of RGB components in a single bounds-checked indexing.
    #[inline]
    pub fn add_rgb(&mut self, x: usize, y: usize, r: u8, g: u8, b: u8) {
        if x < WIDTH && y < HEIGHT {
            let px = &mut self.pixels[y * WIDTH + x];
            px.r = px.r.saturating_add(r);
            px.g = px.g.saturating_add(g);
            px.b = px.b.saturating_add(b);
        }
    }

    /// In-place saturating additive blending with another `Rgb` color.
    #[inline]
    pub fn blend_add(&mut self, x: usize, y: usize, color: Rgb) {
        self.add_rgb(x, y, color.r, color.g, color.b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_basics() {
        let black = Rgb::BLACK;
        assert_eq!(black, Rgb::new(0, 0, 0));
        let white = Rgb::WHITE;
        assert_eq!(white, Rgb::new(255, 255, 255));

        let red = Rgb::hsv(0.0, 1.0, 1.0);
        assert_eq!(red, Rgb::new(255, 0, 0));

        let scaled = white.scale(0.5);
        assert_eq!(scaled, Rgb::new(128, 128, 128));

        let zero_scaled = white.scale(0.0);
        assert_eq!(zero_scaled, Rgb::BLACK);
    }

    #[test]
    fn test_frame_basics() {
        let mut frame = Frame::default();
        assert_eq!(frame.get(0, 0), Rgb::BLACK);

        frame.set(10, 20, Rgb::new(100, 150, 200));
        assert_eq!(frame.get(10, 20), Rgb::new(100, 150, 200));

        // Additive blending
        frame.add_rgb(10, 20, 50, 150, 60);
        assert_eq!(frame.get(10, 20), Rgb::new(150, 255, 255)); // saturating add

        frame.blend_add(10, 20, Rgb::new(20, 20, 20));
        assert_eq!(frame.get(10, 20), Rgb::new(170, 255, 255));

        // Out of bounds writes do not panic
        frame.set(32, 0, Rgb::WHITE);
        frame.set(0, 32, Rgb::WHITE);
        frame.add_rgb(32, 32, 10, 10, 10);
        frame.blend_add(32, 32, Rgb::WHITE);
        assert_eq!(frame.get(32, 0), Rgb::BLACK);
        assert_eq!(frame.get(0, 32), Rgb::BLACK);

        frame.clear(Rgb::new(1, 2, 3));
        assert_eq!(frame.get(0, 0), Rgb::new(1, 2, 3));
        assert_eq!(frame.get(31, 31), Rgb::new(1, 2, 3));
    }
}
