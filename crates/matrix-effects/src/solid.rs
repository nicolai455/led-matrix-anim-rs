use matrix_core::{Effect, Frame, RenderContext, Rgb};

pub struct SolidEffect {
    default_color: Rgb,
}

impl Default for SolidEffect {
    fn default() -> Self {
        Self {
            default_color: Rgb::WHITE,
        }
    }
}

impl SolidEffect {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_color(color: Rgb) -> Self {
        Self {
            default_color: color,
        }
    }
}

impl Effect for SolidEffect {
    fn id(&self) -> &'static str {
        "Solid"
    }

    fn name(&self) -> &'static str {
        "Solid"
    }

    fn reset(&mut self) {}

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let color = if context.color == Rgb::BLACK {
            self.default_color
        } else {
            context.color
        };
        frame.clear(color);
    }
}
