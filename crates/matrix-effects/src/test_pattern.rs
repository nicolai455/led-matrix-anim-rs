use matrix_core::{Effect, Frame, RenderContext, Rgb};

#[derive(Default)]
pub struct TestPatternEffect;

impl TestPatternEffect {
    pub fn new() -> Self {
        Self
    }
}

impl Effect for TestPatternEffect {
    fn id(&self) -> &'static str {
        "TestPattern"
    }

    fn name(&self) -> &'static str {
        "TestPattern"
    }

    fn reset(&mut self) {}

    fn render(&mut self, frame: &mut Frame, _context: &RenderContext<'_>) {
        frame.clear(Rgb::BLACK);

        // Fill each quadrant with panel colors (2x2 layout)
        // Top-left: Red
        let red = Rgb::new(255, 0, 0);
        for y in 0..16 {
            for x in 0..16 {
                frame.set(x, y, red);
            }
        }

        // Top-right: Green
        let green = Rgb::new(0, 255, 0);
        for y in 0..16 {
            for x in 16..32 {
                frame.set(x, y, green);
            }
        }

        // Bottom-left: Blue
        let blue = Rgb::new(0, 0, 255);
        for y in 16..32 {
            for x in 0..16 {
                frame.set(x, y, blue);
            }
        }

        // Bottom-right: Yellow
        let yellow = Rgb::new(255, 255, 0);
        for y in 16..32 {
            for x in 16..32 {
                frame.set(x, y, yellow);
            }
        }

        // Corner arrows (white)
        let white = Rgb::WHITE;
        for i in 0..=5 {
            frame.set(i, 0, white);
            frame.set(0, i, white);
            if i > 0 {
                frame.set(i, i, white);
            }

            frame.set(31 - i, 0, white);
            frame.set(31, i, white);
            if i > 0 {
                frame.set(31 - i, i, white);
            }

            frame.set(i, 31, white);
            frame.set(0, 31 - i, white);
            if i > 0 {
                frame.set(i, 31 - i, white);
            }

            frame.set(31 - i, 31, white);
            frame.set(31, 31 - i, white);
            if i > 0 {
                frame.set(31 - i, 31 - i, white);
            }
        }

        // Mark top-left start corner with cyan
        frame.set(0, 0, Rgb::new(0, 255, 255));

        // Horizontal arrow in top-left panel (magenta)
        let magenta = Rgb::new(255, 0, 255);
        for x in 3..13 {
            frame.set(x, 7, magenta);
            frame.set(x, 8, magenta);
        }
        frame.set(11, 5, magenta);
        frame.set(12, 6, magenta);
        frame.set(13, 7, magenta);
        frame.set(13, 8, magenta);
        frame.set(12, 9, magenta);
        frame.set(11, 10, magenta);

        // Serpentine snake pattern (white)
        for x in 14..=17 {
            frame.set(x, 14, white);
        }
        for y in 14..=16 {
            frame.set(17, y, white);
        }
        for x in 14..=17 {
            frame.set(x, 16, white);
        }
        for y in 16..=18 {
            frame.set(14, y, white);
        }
        for x in 14..=17 {
            frame.set(x, 18, white);
        }
    }
}
