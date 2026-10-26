use matrix_core::{Effect, Frame, FrameSource, RenderContext, Rgb};

pub struct FramesEffect {
    source: Option<Box<dyn FrameSource>>,
    frame_delay_ms: u32,
    current_idx: usize,
    last_ms: u64,
    frame_loaded: bool,
    current_frame: Option<Box<Frame>>,
}

impl Default for FramesEffect {
    fn default() -> Self {
        Self {
            source: None,
            frame_delay_ms: 100,
            current_idx: 0,
            last_ms: 0,
            frame_loaded: false,
            current_frame: None,
        }
    }
}

impl FramesEffect {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_source(source: Box<dyn FrameSource>, delay_ms: u32) -> Self {
        Self {
            source: Some(source),
            frame_delay_ms: if delay_ms == 0 { 100 } else { delay_ms },
            current_idx: 0,
            last_ms: 0,
            frame_loaded: false,
            current_frame: None,
        }
    }

    pub fn set_source(&mut self, source: Box<dyn FrameSource>, delay_ms: u32) {
        self.source = Some(source);
        if delay_ms > 0 {
            self.frame_delay_ms = delay_ms;
        }
        self.reset();
    }

    /// Returns true if the working frame buffer is currently allocated.
    #[inline]
    pub fn has_frame_buffer(&self) -> bool {
        self.current_frame.is_some()
    }
}

pub fn frame_animation(source: Box<dyn FrameSource>, delay_ms: u32) -> Box<dyn Effect> {
    Box::new(FramesEffect::from_source(source, delay_ms))
}

impl Effect for FramesEffect {
    fn id(&self) -> &'static str {
        "Frames"
    }

    fn name(&self) -> &'static str {
        "Frames"
    }

    fn reset(&mut self) {
        self.current_idx = 0;
        self.last_ms = 0;
        self.frame_loaded = false;
        if let Some(ref mut frame) = self.current_frame {
            frame.clear(Rgb::BLACK);
        }
    }

    fn deactivate(&mut self) {
        self.current_frame = None;
        self.frame_loaded = false;
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let Some(source) = self.source.as_mut() else {
            frame.clear(Rgb::BLACK);
            return;
        };

        let count = source.frame_count();
        if count == 0 {
            frame.clear(Rgb::BLACK);
            return;
        }

        let delay = (self.frame_delay_ms.max(1)) as u64;

        let current_frame = self
            .current_frame
            .get_or_insert_with(|| Box::new(Frame::default()));

        if !self.frame_loaded {
            self.last_ms = context.now_ms;
            if source
                .read_frame_into(self.current_idx, current_frame)
                .is_err()
            {
                current_frame.clear(Rgb::BLACK);
            }
            self.frame_loaded = true;
        } else {
            if context.now_ms < self.last_ms {
                self.last_ms = context.now_ms;
            }
            let elapsed = context.now_ms.saturating_sub(self.last_ms);
            if elapsed >= delay {
                let frames_to_advance = (elapsed / delay) as usize;
                self.current_idx = (self.current_idx + frames_to_advance) % count;
                self.last_ms += (frames_to_advance as u64) * delay;
                if source
                    .read_frame_into(self.current_idx, current_frame)
                    .is_err()
                {
                    current_frame.clear(Rgb::BLACK);
                }
            }
        }

        *frame = (**current_frame).clone();
    }
}
