#![forbid(unsafe_code)]

use matrix_core::{AudioSnapshot, Rgb};

#[inline]
pub fn clamp01(v: f32) -> f32 {
    if !v.is_finite() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

#[inline]
pub fn to_byte(v: f32) -> u8 {
    if !v.is_finite() || v <= 0.0 {
        0
    } else if v >= 255.0 {
        255
    } else {
        v as u8
    }
}

pub fn audio_bass(audio: &AudioSnapshot) -> f32 {
    if !audio.available {
        return 0.0;
    }
    let b = (audio.bands[0] + audio.bands[1] + audio.bands[2] + audio.bands[3]) * 0.25;
    clamp01(b.max(audio.level * 0.5))
}

pub fn audio_mid(audio: &AudioSnapshot) -> f32 {
    if !audio.available {
        return 0.0;
    }
    let m =
        (audio.bands[4] + audio.bands[5] + audio.bands[6] + audio.bands[7] + audio.bands[8]) * 0.2;
    clamp01(m)
}

pub fn audio_treble(audio: &AudioSnapshot) -> f32 {
    if !audio.available {
        return 0.0;
    }
    let t = (audio.bands[9]
        + audio.bands[10]
        + audio.bands[11]
        + audio.bands[12]
        + audio.bands[13]
        + audio.bands[14]
        + audio.bands[15])
        * (1.0 / 7.0);
    clamp01(t)
}

use matrix_core::pixel::{HEIGHT, WIDTH};

#[inline]
pub fn alloc_boxed_2d<T: Copy>(val: T) -> Box<[[T; WIDTH]; HEIGHT]> {
    vec![[val; WIDTH]; HEIGHT]
        .into_boxed_slice()
        .try_into()
        .unwrap_or_else(|_| unreachable!("fixed matrix height"))
}

#[inline]
pub fn alloc_boxed_3d<T: Copy, const D: usize>(val: T) -> Box<[[[T; D]; WIDTH]; HEIGHT]> {
    vec![[[val; D]; WIDTH]; HEIGHT]
        .into_boxed_slice()
        .try_into()
        .unwrap_or_else(|_| unreachable!("fixed matrix height"))
}

pub struct XorShift32 {
    state: u32,
}

impl XorShift32 {
    pub const fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 0x12345678 } else { seed },
        }
    }

    pub fn next_u32(&mut self) -> u32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        self.state
    }

    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() & 0xFFFF) as f32 / 65535.0
    }

    pub fn next_signed_f32(&mut self) -> f32 {
        self.next_f32() * 2.0 - 1.0
    }
}

pub fn hsv_to_rgb(mut h: f32, s: f32, v: f32) -> Rgb {
    h = h - h.floor();
    let c = v * s;
    let hp = h * 6.0;
    let x = c * (1.0 - ((hp % 2.0) - 1.0).abs());
    let m = v - c;
    let (rf, gf, bf) = match hp as usize {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    Rgb::new(
        to_byte((rf + m) * 255.0),
        to_byte((gf + m) * 255.0),
        to_byte((bf + m) * 255.0),
    )
}

/// Black -> Red -> Orange -> Yellow -> White heat map
pub fn heat_to_rgb(heat: f32) -> Rgb {
    let t = clamp01(heat);
    if t <= 0.0 {
        Rgb::BLACK
    } else if t < 0.25 {
        Rgb::new(to_byte(t * 4.0 * 255.0), 0, 0)
    } else if t < 0.5 {
        Rgb::new(255, to_byte((t - 0.25) * 4.0 * 180.0), 0)
    } else if t < 0.75 {
        Rgb::new(
            255,
            to_byte(180.0 + (t - 0.5) * 4.0 * 75.0),
            to_byte((t - 0.5) * 4.0 * 40.0),
        )
    } else {
        Rgb::new(255, 255, to_byte(40.0 + (t - 0.75) * 4.0 * 215.0))
    }
}
