pub mod audio;
pub mod effect;
pub mod engine;
pub mod frames;
pub mod geometry;
pub mod pixel;
pub mod state;

// Re-export fixed public core API items at root of crate
pub use audio::{
    synthetic_audio, synthetic_audio_sample, AudioSnapshot, MicDsp, MicSettings,
    BANDS as AUDIO_BANDS, FFT_SIZE, SAMPLE_RATE as AUDIO_SAMPLE_RATE,
};
pub use effect::{Effect, ParameterDescriptor, ParameterKind, RenderContext};
pub use engine::{AudioMode, Engine};
pub use frames::{
    parse_json_frame, FrameSource, LfxHeader, LfxStream, LfxWriter, SliceFrameSource,
    BYTES_PER_FRAME, LFX_HEADER_SIZE, LFX_MAGIC,
};
pub use geometry::{
    calculate_frame_current_ma, calculate_max_brightness, PanelConfig, PanelDirection,
    PanelStartCorner, DEFAULT_MAX_MILLIAMPS, MAX_MILLIAMPS_LIMIT, MIN_MILLIAMPS_LIMIT, NUM_PANELS,
    PANEL_SIZE, TOTAL_SIZE,
};
pub use pixel::{Frame, Rgb, HEIGHT, PIXELS, WIDTH};
pub use state::{restore_runtime_state, serialize_runtime_state};
