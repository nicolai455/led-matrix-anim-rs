//! Shared model types for the effect GIF / contact-sheet renderer.

use serde::Serialize;

/// How audio is fed to the engine while rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum AudioChoice {
    /// Deterministic synthetic techno track from `matrix_core::synthetic_audio`.
    Synthetic,
    /// Silent `AudioSnapshot` (audio-reactive effects idle).
    Off,
}

impl AudioChoice {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Synthetic => "synthetic",
            Self::Off => "off",
        }
    }

    /// Engine `audio_source` command payload for this choice.
    pub fn engine_source(self) -> &'static str {
        match self {
            Self::Synthetic => "Synthetic",
            Self::Off => "Off",
        }
    }

    /// Snapshot for the given timestamp.
    pub fn snapshot(self, t_ms: u64) -> matrix_core::AudioSnapshot {
        match self {
            Self::Synthetic => matrix_core::synthetic_audio(t_ms),
            Self::Off => matrix_core::AudioSnapshot::silent(),
        }
    }
}

/// One entry of `Engine::catalog()` enriched with the index it has in
/// `builtin_effects()`.
#[derive(Clone, Debug)]
pub struct EffectInfo {
    pub index: usize,
    pub id: String,
    pub name: String,
    pub parameter_count: usize,
    pub audio_reactive: bool,
}

impl EffectInfo {
    pub fn has_parameters(&self) -> bool {
        self.parameter_count > 0
    }

    /// File stem used for every artifact of this effect: `001-TestPattern`.
    pub fn file_stem(&self) -> String {
        format!("{:03}-{}", self.index + 1, sanitize(&self.id))
    }
}

/// Keep artifact names portable: anything outside `[A-Za-z0-9._-]` becomes `_`.
fn sanitize(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Per-effect summary row written to `summary.json`.
#[derive(Clone, Debug, Serialize)]
pub struct EffectSummary {
    pub index: usize,
    pub id: String,
    pub name: String,
    pub frames: u64,
    pub fps: u32,
    pub seconds: f64,
    pub max_luma: u8,
    pub mean_luma: f64,
    pub nonblack_frames: u64,
    pub distinct_colors_max: usize,
    pub has_parameters: bool,
    pub audio_reactive: bool,
    pub bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl EffectSummary {
    pub fn for_error(info: &EffectInfo, fps: u32, seconds: f64, error: String) -> Self {
        Self {
            index: info.index,
            id: info.id.clone(),
            name: info.name.clone(),
            frames: 0,
            fps,
            seconds,
            max_luma: 0,
            mean_luma: 0.0,
            nonblack_frames: 0,
            distinct_colors_max: 0,
            has_parameters: info.has_parameters(),
            audio_reactive: info.audio_reactive,
            bytes: 0,
            error: Some(error),
        }
    }
}

/// Luma in 0..=255 using Rec. 709 weights (integer math, no float drift).
#[inline]
pub fn luma(pixel: matrix_core::Rgb) -> u8 {
    let value =
        (2126 * pixel.r as u32 + 7152 * pixel.g as u32 + 722 * pixel.b as u32) / 10_000;
    value.min(255) as u8
}
