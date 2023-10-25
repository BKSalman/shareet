use glyphon::{Attrs, FontSystem, TextBounds};

use crate::shapes::Mesh;

use super::Widget;

pub struct TextWidget {
    text: crate::Text,
    background: Option<Mesh>,
    requires_redraw: bool,
}

impl TextWidget {
    pub fn new(
        x: i32,
        y: i32,
        content: &str,
        text_color: glyphon::Color,
        font_system: &mut FontSystem,
        metrics: glyphon::Metrics,
        background_color: Option<crate::Color>,
    ) -> Self {
        let mut buffer = glyphon::Buffer::new(font_system, metrics);
        // TODO: get actual window width and height
        buffer.set_size(font_system, 1920., 30.);
        buffer.set_text(
            font_system,
            content,
            Attrs::new().family(glyphon::Family::Monospace),
            glyphon::Shaping::Advanced,
        );

        let (width, total_lines) = buffer
            .layout_runs()
            .fold((0.0, 0usize), |(width, total_lines), run| {
                (run.line_w.max(width), total_lines + 1)
            });

        let height = total_lines as f32 * buffer.metrics().line_height;

        buffer.set_size(font_system, width, height);

        let background = background_color.map(|b| {
            crate::Painter::create_mesh(
                crate::shapes::Shape::Rect(crate::shapes::Rect {
                    x,
                    y,
                    width: width.ceil() as u32,
                    height: height.ceil() as u32,
                }),
                b,
            )
        });

        Self {
            text: crate::Text {
                x,
                y,
                color: text_color,
                content: content.to_string(),
                bounds: TextBounds {
                    left: x,
                    top: 0,
                    right: x + width.ceil() as i32,
                    bottom: height.ceil() as i32,
                },
                buffer,
            },
            background,
            requires_redraw: true,
        }
    }

    pub fn x(&self) -> i32 {
        self.text.x
    }

    pub fn set_redraw(&mut self, redraw: bool) {
        self.requires_redraw = redraw;
    }
}

impl Widget for TextWidget {
    fn setup(
        &mut self,
        state: &mut crate::State,
        connection: &x11rb::xcb_ffi::XCBConnection,
        screen_num: usize,
    ) -> Result<(), crate::Error> {
        Ok(())
    }

    fn on_event(
        &mut self,
        connection: &x11rb::xcb_ffi::XCBConnection,
        state: &mut crate::State,
        event: x11rb::protocol::Event,
    ) -> Result<(), crate::Error> {
        match event {
            x11rb::protocol::Event::Expose(_) => {
                self.requires_redraw = true;
            }
            _ => {}
        }
        Ok(())
    }

    fn meshes(&self) -> Vec<&crate::shapes::Mesh> {
        self.background.as_ref().map(|b| vec![b]).unwrap_or(vec![])
    }

    fn texts(&self, _font_system: &mut FontSystem) -> Vec<&crate::text_renderer::Text> {
        vec![&self.text]
    }

    fn size(&self) -> u32 {
        self.text.bounds.right as u32 - self.text.bounds.left as u32
    }

    fn requires_redraw(&self) -> bool {
        self.requires_redraw
    }
}
