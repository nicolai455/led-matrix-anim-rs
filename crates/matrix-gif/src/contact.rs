//! Contact sheets: one PNG per effect with a 2x3 grid of sampled frames.

use std::path::Path;

use image::{Rgba, RgbaImage};
use matrix_core::{Frame, Rgb};
use matrix_effects::text::{draw_char, FONT_HEIGHT, FONT_SPACING, FONT_WIDTH};

use crate::model::EffectInfo;

/// Relative sample positions inside the recorded clip.
pub const SAMPLE_PERCENT: [u32; 6] = [10, 30, 50, 70, 85, 95];

const COLUMNS: u32 = 2;
const ROWS: u32 = 3;
const MARGIN: u32 = 16;
const COL_GAP: u32 = 16;
const ROW_GAP: u32 = 14;
const LABEL_GAP: u32 = 6;
const BACKGROUND: Rgba<u8> = Rgba([12, 12, 16, 255]);
const LABEL_COLOR: Rgba<u8> = Rgba([235, 235, 240, 255]);
const MUTED_COLOR: Rgba<u8> = Rgba([150, 150, 160, 255]);

/// Width in pixels `text` occupies at the given font `scale`.
fn text_width(text: &str, scale: u32) -> u32 {
    let chars = text.chars().count() as u32;
    if chars == 0 {
        return 0;
    }
    chars * (FONT_WIDTH as u32 + FONT_SPACING as u32) * scale - FONT_SPACING as u32 * scale
}

/// Largest scale `<= max` at which `text` fits into `available` pixels (at least 1).
fn fitting_scale(text: &str, available: u32, max: u32) -> u32 {
    let mut scale = max.max(1);
    while scale > 1 && text_width(text, scale) > available {
        scale -= 1;
    }
    scale
}

/// Shortens `text` with a trailing `.` until it fits `available` pixels at `scale`.
fn fit_text(text: &str, available: u32, scale: u32) -> String {
    if text_width(text, scale) <= available {
        return text.to_string();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let candidate: String = chars.iter().collect::<String>() + ".";
        if text_width(&candidate, scale) <= available {
            return candidate;
        }
    }
    String::new()
}

/// Paints `text` into `canvas` at (`x`, `y`), rendered from the shared 5x7
/// bitmap font via `draw_char` and scaled with nearest-neighbour blocks.
fn blit_text(canvas: &mut RgbaImage, text: &str, x: u32, y: u32, scale: u32, color: Rgba<u8>) {
    let mut cursor = x as i64;
    for character in text.chars() {
        // draw_char only writes into a 32x32 Frame, so rasterize the glyph into a
        // scratch frame and copy its 5x7 cells out at the requested scale.
        let mut scratch = Frame::new();
        draw_char(&mut scratch, character, 0, 0, Rgb::WHITE);
        for row in 0..FONT_HEIGHT as u32 {
            for col in 0..FONT_WIDTH as u32 {
                if scratch.get(col as usize, row as usize) == Rgb::BLACK {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let px = cursor + i64::from(col * scale + dx);
                        let py = i64::from(y + row * scale + dy);
                        if px >= 0 && py >= 0 && (px as u32) < canvas.width() && (py as u32) < canvas.height() {
                            canvas.put_pixel(px as u32, py as u32, color);
                        }
                    }
                }
            }
        }
        cursor += i64::from((FONT_WIDTH as u32 + FONT_SPACING as u32) * scale);
    }
}

/// Nearest-neighbour copy of a rendered frame at `scale` (no alpha blending:
/// matrix pixels are always opaque).
fn scale_frame(frame: &Frame, scale: u32) -> RgbaImage {
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

/// Composes the 2x3 contact sheet for `info` from six sampled frames.
/// `samples[i]` belongs to `SAMPLE_PERCENT[i]`, in order.
pub fn build_sheet(info: &EffectInfo, samples: &[Frame], cell_scale: u32) -> RgbaImage {
    let cell_scale = cell_scale.max(1);
    let image_size = matrix_core::WIDTH as u32 * cell_scale;
    // Cell labels shrink to fit their column instead of being cut off.
    let max_label_scale = (cell_scale / 2).max(2);
    let label_scale = fitting_scale(&info.id, image_size, max_label_scale);
    let percent_scale = label_scale;
    let label_height = FONT_HEIGHT as u32 * label_scale;
    let percent_height = FONT_HEIGHT as u32 * percent_scale;
    let cell_height = label_height + LABEL_GAP + image_size + LABEL_GAP + percent_height;

    let title = format!("{:03}-{}", info.index + 1, info.id);
    let grid_width = COLUMNS * image_size + (COLUMNS - 1) * COL_GAP;
    // Long ids must not widen or overflow the sheet: shrink the title instead.
    let title_scale = fitting_scale(&title, grid_width, 3);
    let title_height = FONT_HEIGHT as u32 * title_scale;

    let width = MARGIN * 2 + grid_width;
    let header_height = title_height + 12;
    let height = MARGIN + header_height + ROWS * cell_height + (ROWS - 1) * ROW_GAP + MARGIN;

    let mut canvas = RgbaImage::from_pixel(width, height, BACKGROUND);
    blit_text(&mut canvas, &title, MARGIN, MARGIN, title_scale, LABEL_COLOR);

    for (slot, frame) in samples.iter().take(SAMPLE_PERCENT.len()).enumerate() {
        let column = slot as u32 % COLUMNS;
        let row = slot as u32 / COLUMNS;
        let cell_x = MARGIN + column * (image_size + COL_GAP);
        let cell_y = MARGIN + header_height + row * (cell_height + ROW_GAP);

        let label = fit_text(&info.id, image_size, label_scale);
        blit_text(&mut canvas, &label, cell_x, cell_y, label_scale, LABEL_COLOR);
        let cell = scale_frame(frame, cell_scale);
        image::imageops::replace(
            &mut canvas,
            &cell,
            i64::from(cell_x),
            i64::from(cell_y + label_height + LABEL_GAP),
        );

        let percent = format!("{}%", SAMPLE_PERCENT[slot]);
        blit_text(
            &mut canvas,
            &percent,
            cell_x,
            cell_y + label_height + LABEL_GAP + image_size + LABEL_GAP,
            percent_scale,
            MUTED_COLOR,
        );
    }

    canvas
}

/// Writes `sheet` as PNG to `path`, creating parent directories as needed.
/// Writes through a `.part` file so an interrupted run leaves no broken PNG.
pub fn save_png(sheet: &RgbaImage, path: &Path) -> Result<u64, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    let part = path.with_file_name(name);
    sheet
        .save_with_format(&part, image::ImageFormat::Png)
        .map_err(|error| format!("cannot write {}: {error}", part.display()))?;
    std::fs::rename(&part, path)
        .map_err(|error| format!("cannot move {} to {}: {error}", part.display(), path.display()))?;
    std::fs::metadata(path)
        .map(|meta| meta.len())
        .map_err(|error| format!("cannot stat {}: {error}", path.display()))
}

/// Frame indices to sample for `frames` total recorded frames.
pub fn sample_indices(frames: u64) -> Vec<u64> {
    SAMPLE_PERCENT
        .iter()
        .map(|percent| {
            let scaled = (u128::from(frames.saturating_sub(1)) * u128::from(*percent)) / 100;
            u64::try_from(scaled).unwrap_or(u64::MAX)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(id: &str) -> EffectInfo {
        EffectInfo {
            index: 0,
            id: id.to_string(),
            name: id.to_string(),
            parameter_count: 0,
            audio_reactive: false,
        }
    }

    #[test]
    fn sample_indices_are_monotonic_and_bounded() {
        let indices = sample_indices(100);
        assert_eq!(indices.len(), SAMPLE_PERCENT.len());
        assert_eq!(indices.first().copied(), Some(9));
        assert_eq!(indices.last().copied(), Some(94));
        assert!(indices.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    fn sample_indices_handle_single_frame() {
        assert!(sample_indices(1).iter().all(|index| *index == 0));
    }

    #[test]
    fn text_width_drops_trailing_spacing() {
        assert_eq!(text_width("AB", 4), (6 * 4 * 2) - 4);
        assert_eq!(text_width("", 4), 0);
    }

    #[test]
    fn fitting_scale_shrinks_until_text_fits() {
        assert_eq!(fitting_scale("AB", 1000, 4), 4);
        assert_eq!(fitting_scale("ABC", text_width("ABC", 2), 4), 2);
        // Never drops below 1, even when the text cannot fit at all.
        assert_eq!(fitting_scale("ABCDEFGH", 1, 3), 1);
    }

    #[test]
    fn fit_text_truncates_with_ellipsis_marker() {
        assert_eq!(fit_text("short", 1000, 2), "short");
        let truncated = fit_text("very_long_effect_id", text_width("ab", 2), 2);
        assert!(text_width(&truncated, 2) <= text_width("ab", 2));
        assert!(truncated.ends_with('.'));
    }

    #[test]
    fn sheet_has_expected_grid_extent() {
        let samples = vec![Frame::new(); 6];
        let sheet = build_sheet(&info("Test"), &samples, 4);
        let expected_width = MARGIN * 2 + COLUMNS * 128 + (COLUMNS - 1) * COL_GAP;
        assert_eq!(sheet.width(), expected_width);
        assert!(sheet.height() > 3 * 128);
    }

    #[test]
    fn long_ids_stay_inside_the_sheet() {
        let samples = vec![Frame::new(); 6];
        let sheet = build_sheet(
            &info("an_extremely_long_effect_identifier_that_would_overflow"),
            &samples,
            4,
        );
        let expected_width = MARGIN * 2 + COLUMNS * 128 + (COLUMNS - 1) * COL_GAP;
        assert_eq!(sheet.width(), expected_width);
    }

    #[test]
    fn sheet_tolerates_missing_samples() {
        let sheet = build_sheet(&info("Partial"), &[Frame::new()], 2);
        assert!(sheet.width() > 0 && sheet.height() > 0);
    }
}
