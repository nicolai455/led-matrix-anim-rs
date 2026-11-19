//! Animated GIF writing with nearest-neighbour upscaling and render statistics.

use std::collections::HashSet;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame as ImageFrame, Rgba, RgbaImage};
use matrix_core::Frame;

use crate::model::luma;

/// Aggregated statistics over every recorded frame of one effect.
#[derive(Clone, Debug, Default)]
pub struct GifStats {
    pub frames: u64,
    pub max_luma: u8,
    pub luma_sum: u64,
    pub nonblack_frames: u64,
    pub distinct_colors_max: usize,
}

impl GifStats {
    pub fn mean_luma(&self) -> f64 {
        if self.frames == 0 {
            return 0.0;
        }
        let pixels = self.frames * (matrix_core::PIXELS as u64);
        self.luma_sum as f64 / pixels as f64
    }

    /// Folds one rendered frame into the statistics.
    pub fn absorb(&mut self, frame: &Frame) {
        self.frames += 1;
        let mut distinct = HashSet::new();
        let mut nonblack = false;
        for pixel in frame.pixels.iter() {
            let value = luma(*pixel);
            if value > self.max_luma {
                self.max_luma = value;
            }
            self.luma_sum += u64::from(value);
            if value > 0 {
                nonblack = true;
            }
            distinct.insert((pixel.r, pixel.g, pixel.b));
        }
        if nonblack {
            self.nonblack_frames += 1;
        }
        if distinct.len() > self.distinct_colors_max {
            self.distinct_colors_max = distinct.len();
        }
    }
}

/// Copies `frame` into an RGBA image scaled by `scale` using nearest-neighbour.
pub fn upscale(frame: &Frame, scale: u32) -> RgbaImage {
    let size = matrix_core::WIDTH as u32;
    let mut image = RgbaImage::new(size * scale, size * scale);
    for y in 0..size {
        for x in 0..size {
            let pixel = frame.get(x as usize, y as usize);
            let rgba = Rgba([pixel.r, pixel.g, pixel.b, 255]);
            for dy in 0..scale {
                for dx in 0..scale {
                    image.put_pixel(x * scale + dx, y * scale + dy, rgba);
                }
            }
        }
    }
    image
}

/// Encodes frames into an animated, infinitely looping GIF at `path`.
///
/// Frames are written through `next_frame` so callers can render lazily instead
/// of holding the whole animation in memory. Data is first written to a
/// `.part` file and renamed only on success, so a failed run never leaves a
/// truncated GIF behind.
pub fn write_gif<F>(
    path: &Path,
    scale: u32,
    fps: u32,
    frame_total: u64,
    mut next_frame: F,
) -> Result<GifStats, String>
where
    F: FnMut(u64) -> Result<Frame, String>,
{
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let part = part_path(path);
    let file = File::create(&part)
        .map_err(|error| format!("cannot create {}: {error}", part.display()))?;

    let mut stats = GifStats::default();
    let delay = Delay::from_numer_denom_ms(1000, fps.max(1));
    let scale = scale.max(1);

    let result = (|| -> Result<(), String> {
        let mut encoder = GifEncoder::new_with_speed(BufWriter::new(file), 10);
        encoder
            .set_repeat(Repeat::Infinite)
            .map_err(|error| format!("cannot set GIF repeat: {error}"))?;
        for index in 0..frame_total {
            let frame = next_frame(index)?;
            stats.absorb(&frame);
            let buffer = upscale(&frame, scale);
            let image_frame = ImageFrame::from_parts(buffer, 0, 0, delay);
            encoder
                .encode_frame(image_frame)
                .map_err(|error| format!("cannot encode frame {index}: {error}"))?;
        }
        Ok(())
    })();

    match result {
        Ok(()) => {
            std::fs::rename(&part, path).map_err(|error| {
                format!(
                    "cannot move {} to {}: {error}",
                    part.display(),
                    path.display()
                )
            })?;
            Ok(stats)
        }
        Err(error) => {
            let _ = std::fs::remove_file(&part);
            Err(error)
        }
    }
}

fn part_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    path.with_file_name(name)
}
