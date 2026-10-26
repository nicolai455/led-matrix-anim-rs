use matrix_core::{Effect, Frame, RenderContext, Rgb, HEIGHT, WIDTH};

#[derive(Default)]
pub struct RainbowEffect;

impl RainbowEffect {
    pub fn new() -> Self {
        Self
    }
}

impl Effect for RainbowEffect {
    fn id(&self) -> &'static str {
        "Rainbow"
    }

    fn name(&self) -> &'static str {
        "Rainbow"
    }

    fn reset(&mut self) {}

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let time = (context.now_ms / 20) as usize;
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let hue = (((x * 4 + y * 2 + time) & 0xFF) as f32) / 256.0;
                frame.set(x, y, Rgb::hsv(hue, 1.0, 1.0));
            }
        }
    }
}
