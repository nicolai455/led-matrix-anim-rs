use crate::pixel::{Frame, PIXELS};
use serde::{Deserialize, Serialize};

pub const NUM_PANELS: usize = 4;
pub const PANEL_SIZE: usize = 16;
pub const TOTAL_SIZE: usize = 32;

pub const DEFAULT_MAX_MILLIAMPS: u32 = 12000;
pub const MIN_MILLIAMPS_LIMIT: u32 = 1000;
pub const MAX_MILLIAMPS_LIMIT: u32 = 60000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PanelStartCorner {
    #[default]
    TopLeft = 0,
    TopRight = 1,
    BottomLeft = 2,
    BottomRight = 3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PanelDirection {
    #[default]
    Horizontal = 0,
    Vertical = 1,
}

fn default_matrix_width() -> u8 {
    2
}
fn default_matrix_height() -> u8 {
    2
}
fn default_panel_order() -> [u8; NUM_PANELS] {
    [0, 1, 2, 3]
}
fn default_panel_rotation() -> [u16; NUM_PANELS] {
    [0, 0, 0, 0]
}
fn default_serpentine() -> [bool; NUM_PANELS] {
    [true, true, true, true]
}
fn default_start_corner() -> u8 {
    0
}
fn default_panel_layout() -> u8 {
    0
}
fn default_panel_serpentine() -> bool {
    false
}
fn default_mirror_x() -> bool {
    false
}
fn default_parallel_output() -> bool {
    false
}
fn default_data_pin() -> u8 {
    4
}
fn default_panel_pins() -> [u8; NUM_PANELS] {
    [4, 16, 17, 18]
}
fn default_led_brightness() -> u8 {
    128
}
fn default_led_max_milliamps() -> u32 {
    DEFAULT_MAX_MILLIAMPS
}
fn default_animation() -> String {
    "Rainbow".to_string()
}
fn default_auto_cycle_ms() -> u32 {
    0
}
fn default_fs_animation_path() -> String {
    "/animations/example.lfx".to_string()
}
fn default_led_type() -> String {
    "WS2812B".to_string()
}
fn default_led_color_order() -> String {
    "GRB".to_string()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelConfig {
    #[serde(default = "default_matrix_width")]
    pub matrix_width: u8,
    #[serde(default = "default_matrix_height")]
    pub matrix_height: u8,
    #[serde(default = "default_panel_order")]
    pub panel_order: [u8; NUM_PANELS],
    #[serde(default = "default_panel_rotation")]
    pub panel_rotation: [u16; NUM_PANELS],
    #[serde(default = "default_serpentine")]
    pub serpentine: [bool; NUM_PANELS],
    #[serde(default = "default_start_corner")]
    pub start_corner: u8,
    #[serde(default = "default_panel_layout")]
    pub panel_layout: u8,
    #[serde(default = "default_panel_serpentine")]
    pub panel_serpentine: bool,
    #[serde(default = "default_mirror_x")]
    pub mirror_x: bool,
    #[serde(default = "default_parallel_output")]
    pub parallel_output: bool,
    #[serde(default = "default_data_pin")]
    pub data_pin: u8,
    #[serde(default = "default_panel_pins")]
    pub panel_pins: [u8; NUM_PANELS],
    #[serde(default = "default_led_brightness")]
    pub led_brightness: u8,
    #[serde(default = "default_led_max_milliamps")]
    pub led_max_milliamps: u32,
    #[serde(default = "default_animation")]
    pub default_animation: String,
    #[serde(default = "default_auto_cycle_ms")]
    pub auto_cycle_ms: u32,
    #[serde(default = "default_fs_animation_path")]
    pub fs_animation_path: String,
    #[serde(default = "default_led_type")]
    pub led_type: String,
    #[serde(default = "default_led_color_order")]
    pub led_color_order: String,
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            matrix_width: default_matrix_width(),
            matrix_height: default_matrix_height(),
            panel_order: default_panel_order(),
            panel_rotation: default_panel_rotation(),
            serpentine: default_serpentine(),
            start_corner: default_start_corner(),
            panel_layout: default_panel_layout(),
            panel_serpentine: default_panel_serpentine(),
            mirror_x: default_mirror_x(),
            parallel_output: default_parallel_output(),
            data_pin: default_data_pin(),
            panel_pins: default_panel_pins(),
            led_brightness: default_led_brightness(),
            led_max_milliamps: default_led_max_milliamps(),
            default_animation: default_animation(),
            auto_cycle_ms: default_auto_cycle_ms(),
            fs_animation_path: default_fs_animation_path(),
            led_type: default_led_type(),
            led_color_order: default_led_color_order(),
        }
    }
}

impl PanelConfig {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.matrix_width != 2 || self.matrix_height != 2 {
            return Err("matrix dimensions must be 2x2 panels for 32x32 matrix");
        }
        let mut seen = [false; NUM_PANELS];
        for &order in &self.panel_order {
            let idx = order as usize;
            if idx >= NUM_PANELS {
                return Err("panelOrder contains out-of-range index (must be 0..3)");
            }
            if seen[idx] {
                return Err("panelOrder contains duplicate panel index");
            }
            seen[idx] = true;
        }
        if self.start_corner > 3 {
            return Err("start_corner must be in 0..=3");
        }
        if self.panel_layout > 1 {
            return Err("panel_layout must be 0 (Horizontal) or 1 (Vertical)");
        }
        for rot in self.panel_rotation {
            if rot != 0 && rot != 90 && rot != 180 && rot != 270 {
                return Err("panel rotation must be 0, 90, 180, or 270");
            }
        }
        if self.led_max_milliamps < MIN_MILLIAMPS_LIMIT
            || self.led_max_milliamps > MAX_MILLIAMPS_LIMIT
        {
            return Err("led_max_milliamps out of range");
        }
        if self.led_type != "WS2812B" {
            return Err("ledType must be 'WS2812B'");
        }
        if self.led_color_order != "GRB" {
            return Err("ledColorOrder must be 'GRB'");
        }
        Ok(())
    }

    /// Maps a 2D logical panel coordinate (logical_x, logical_y) to the physical panel index (0..3).
    pub fn get_physical_panel_index(
        &self,
        logical_panel_x: usize,
        logical_panel_y: usize,
    ) -> usize {
        let mw = self.matrix_width as usize;
        let mh = self.matrix_height as usize;
        if logical_panel_x >= mw || logical_panel_y >= mh {
            return usize::MAX;
        }

        let mut panel_x = logical_panel_x;
        let mut panel_y = logical_panel_y;

        // Apply start corner transformation
        match self.start_corner {
            1 => {
                // TopRight
                panel_x = mw - 1 - logical_panel_x;
            }
            2 => {
                // BottomLeft
                panel_y = mh - 1 - logical_panel_y;
            }
            3 => {
                // BottomRight
                panel_x = mw - 1 - logical_panel_x;
                panel_y = mh - 1 - logical_panel_y;
            }
            _ => {
                // TopLeft (0)
            }
        }

        let mut panel_index = if self.panel_layout == 0 {
            // Horizontal
            let mut idx = panel_y * mw + panel_x;
            if self.panel_serpentine && (panel_y % 2 == 1) {
                idx = panel_y * mw + (mw - 1 - panel_x);
            }
            idx
        } else {
            // Vertical
            let mut idx = panel_x * mh + panel_y;
            if self.panel_serpentine && (panel_x % 2 == 1) {
                idx = panel_x * mh + (mh - 1 - panel_y);
            }
            idx
        };

        if panel_index < NUM_PANELS {
            panel_index = self.panel_order[panel_index] as usize;
        }
        panel_index
    }

    /// Maps logical pixel coordinate (x, y) on the 32x32 matrix to the physical 1D LED index (0..1023).
    pub fn get_led_index(&self, mut x: usize, y: usize) -> usize {
        if x >= TOTAL_SIZE || y >= TOTAL_SIZE {
            return usize::MAX;
        }

        if self.mirror_x {
            x = TOTAL_SIZE - 1 - x;
        }

        let logical_panel_x = x / PANEL_SIZE;
        let logical_panel_y = y / PANEL_SIZE;
        let physical_panel = self.get_physical_panel_index(logical_panel_x, logical_panel_y);
        if physical_panel >= NUM_PANELS {
            return usize::MAX;
        }

        let local_x = x % PANEL_SIZE;
        let local_y = y % PANEL_SIZE;

        let (rotated_x, rotated_y) = match self.panel_rotation[physical_panel] {
            90 => (PANEL_SIZE - 1 - local_y, local_x),
            180 => (PANEL_SIZE - 1 - local_x, PANEL_SIZE - 1 - local_y),
            270 => (local_y, PANEL_SIZE - 1 - local_x),
            _ => (local_x, local_y),
        };

        let panel_start_index = physical_panel * (PANEL_SIZE * PANEL_SIZE);
        if self.serpentine[physical_panel] && (rotated_y % 2 == 1) {
            panel_start_index + rotated_y * PANEL_SIZE + (PANEL_SIZE - 1 - rotated_x)
        } else {
            panel_start_index + rotated_y * PANEL_SIZE + rotated_x
        }
    }

    /// Maps logical pixel coordinate (x, y) to Some(led_index) if valid, or None if out of bounds.
    #[inline]
    pub fn map_coordinate(&self, x: usize, y: usize) -> Option<usize> {
        let idx = self.get_led_index(x, y);
        if idx < PIXELS {
            Some(idx)
        } else {
            None
        }
    }
}

/// Estimate the power draw in milliamps for a frame at the specified brightness.
/// Quiescent current is ~1 mA per LED controller.
/// WS2812B draws up to ~20 mA per color channel at 255.
pub fn calculate_frame_current_ma(frame: &Frame, brightness: u8) -> u32 {
    let quiescent_ma = PIXELS as u32; // ~1024 mA for 1024 LEDs
    if brightness == 0 {
        return quiescent_ma;
    }

    let b_factor = brightness as u64;
    let mut total_channel_sum: u64 = 0;
    for pixel in &frame.pixels {
        total_channel_sum += pixel.r as u64 + pixel.g as u64 + pixel.b as u64;
    }

    // current_per_channel = (channel * brightness / 255) * 20 / 255 mA
    // total_color_ma = total_channel_sum * brightness * 20 / (255 * 255)
    let color_ma = (total_channel_sum * b_factor * 20) / (255 * 255);
    quiescent_ma + color_ma as u32
}

/// Calculate the maximum allowable brightness (0..=target_brightness) such that
/// estimated current does not exceed `max_milliamps`.
pub fn calculate_max_brightness(frame: &Frame, target_brightness: u8, max_milliamps: u32) -> u8 {
    let current_ma = calculate_frame_current_ma(frame, target_brightness);
    if current_ma <= max_milliamps || target_brightness == 0 {
        return target_brightness;
    }

    let quiescent_ma = PIXELS as u32;
    if max_milliamps <= quiescent_ma {
        return 0;
    }

    let allowed_color_ma = max_milliamps - quiescent_ma;
    let actual_color_ma = current_ma.saturating_sub(quiescent_ma);
    if actual_color_ma == 0 {
        return target_brightness;
    }

    let scale = (allowed_color_ma as f32) / (actual_color_ma as f32);
    let clamped_b = ((target_brightness as f32) * scale).floor() as u8;
    clamped_b.min(target_brightness)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_golden_geometry_16_configs() {
        let fixture_str = include_str!("../../../../tests/legacy_geometry.json");
        let cases: serde_json::Value = serde_json::from_str(fixture_str).unwrap();
        let cases_arr = cases.as_array().expect("array of cases");
        assert_eq!(cases_arr.len(), 16);

        for (i, case) in cases_arr.iter().enumerate() {
            let config: PanelConfig = serde_json::from_value(case.clone()).unwrap();
            let expected_indices = case["indices"].as_array().unwrap();
            assert_eq!(expected_indices.len(), 1024);

            let mut idx = 0;
            for y in 0..32 {
                for x in 0..32 {
                    let expected = expected_indices[idx].as_u64().unwrap() as usize;
                    let actual = config.get_led_index(x, y);
                    assert_eq!(
                        actual, expected,
                        "Case {i} mismatch at ({x}, {y}): got {actual}, expected {expected}"
                    );
                    idx += 1;
                }
            }
        }
    }

    #[test]
    fn test_current_limiting() {
        let mut frame = Frame::default();
        // Dark frame draws quiescent current (~1024 mA)
        assert_eq!(calculate_frame_current_ma(&frame, 255), 1024);

        // Full white frame
        frame.clear(crate::pixel::Rgb::WHITE);
        let full_white_ma = calculate_frame_current_ma(&frame, 255);
        // 1024 * 3 * 20 + 1024 = 61440 + 1024 = 62464
        assert!(full_white_ma > 60000);

        let capped_b = calculate_max_brightness(&frame, 255, 12000);
        assert!(capped_b < 60);
        let capped_ma = calculate_frame_current_ma(&frame, capped_b);
        assert!(capped_ma <= 12000);

        // When max_milliamps is less than or equal to quiescent, return 0
        assert_eq!(calculate_max_brightness(&frame, 255, 1024), 0);
        assert_eq!(calculate_max_brightness(&frame, 255, 500), 0);
        // Budget zero: must return 0 without panic or division by zero
        assert_eq!(calculate_max_brightness(&frame, 255, 0), 0);
        assert_eq!(calculate_max_brightness(&frame, 128, 0), 0);
        assert_eq!(calculate_max_brightness(&frame, 0, 0), 0);
    }

    #[test]
    fn test_power_budget_zero_and_range_limits() {
        let mut cfg = PanelConfig::default();
        assert!(cfg.validate().is_ok());

        // Zero budget is strictly rejected
        cfg.led_max_milliamps = 0;
        assert!(cfg.validate().is_err());

        // Sub-minimum limit rejected (< 1000 mA)
        cfg.led_max_milliamps = 999;
        assert!(cfg.validate().is_err());

        // Minimum limit boundary valid (1000 mA)
        cfg.led_max_milliamps = 1000;
        assert!(cfg.validate().is_ok());

        // Maximum limit boundary valid (60000 mA)
        cfg.led_max_milliamps = 60000;
        assert!(cfg.validate().is_ok());

        // Over-maximum limit rejected (> 60000 mA)
        cfg.led_max_milliamps = 60001;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_panel_config_validation() {
        let valid = PanelConfig::default();
        assert!(valid.validate().is_ok());

        let mut invalid_width = valid.clone();
        invalid_width.matrix_width = 4;
        assert!(invalid_width.validate().is_err());

        let mut dup_order = valid.clone();
        dup_order.panel_order = [0, 1, 1, 3];
        assert!(dup_order.validate().is_err());

        let mut oob_order = valid.clone();
        oob_order.panel_order = [0, 1, 2, 4];
        assert!(oob_order.validate().is_err());

        let mut invalid_led_type = valid.clone();
        invalid_led_type.led_type = "SK6812".to_string();
        assert!(invalid_led_type.validate().is_err());

        let mut invalid_color_order = valid.clone();
        invalid_color_order.led_color_order = "RGB".to_string();
        assert!(invalid_color_order.validate().is_err());
    }
}
