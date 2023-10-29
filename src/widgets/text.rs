use glyphon::Metrics;
use mdry::{color::Color, renderer::measure_text, State};

use super::Widget;

pub struct TextWidget {
    content: String,
    x: f32,
    y: f32,
    color: Color,
    font_size: f32,
    background: Option<Color>,
    requires_redraw: bool,
}

impl TextWidget {
    pub fn new(
        x: f32,
        y: f32,
        content: &str,
        text_color: Color,
        font_size: f32,
        background_color: Option<Color>,
    ) -> Self {
        Self {
            content: content.to_string(),
            background: background_color,
            requires_redraw: true,
            x,
            y,
            color: text_color,
            font_size,
        }
    }

    pub fn x(&self) -> f32 {
        self.x
    }

    pub fn y(&self) -> f32 {
        self.y
    }

    pub fn set_redraw(&mut self, redraw: bool) {
        self.requires_redraw = redraw;
    }
}

impl Widget for TextWidget {
    fn setup(
        &mut self,
        state: &mut State,
        connection: &x11rb::xcb_ffi::XCBConnection,
        screen_num: usize,
    ) -> Result<(), crate::Error> {
        Ok(())
    }

    fn on_event(
        &mut self,
        connection: &x11rb::xcb_ffi::XCBConnection,
        screen_num: usize,
        state: &mut State,
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

    fn draw(
        &mut self,
        connection: &x11rb::xcb_ffi::XCBConnection,
        screen_num: usize,
        state: &mut State,
        offset: f32,
    ) -> Result<(), crate::Error> {
        measure_text(state.draw_text_absolute(
            &self.content,
            self.x + offset,
            self.y,
            self.color,
            self.font_size,
        ));

        Ok(())
    }

    fn size(&self, state: &mut State) -> f32 {
        state
            .measure_text(&self.content, Metrics::new(self.font_size, self.font_size))
            .0
    }

    fn requires_redraw(&self) -> bool {
        self.requires_redraw
    }
}
