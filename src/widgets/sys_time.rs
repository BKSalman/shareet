use chrono::{DateTime, Local};
use mdry::color::Color;

use super::Widget;

// FIXME: this needs to be updated regularely somehow (?)
pub struct SysTime {
    font_size: f32,
    current_time: DateTime<Local>,
    color: Color,
}

impl SysTime {
    pub fn new(font_size: f32, color: Color) -> Self {
        Self {
            current_time: Local::now(),
            font_size,
            color,
        }
    }
}

impl Widget for SysTime {
    fn setup(
        &mut self,
        state: &mut mdry::State,
        connection: &x11rb::xcb_ffi::XCBConnection,
        screen_num: usize,
    ) -> Result<(), crate::Error> {
        Ok(())
    }

    fn on_event(
        &mut self,
        connection: &x11rb::xcb_ffi::XCBConnection,
        screen_num: usize,
        state: &mut mdry::State,
        event: x11rb::protocol::Event,
    ) -> Result<(), crate::Error> {
        self.current_time = Local::now();

        Ok(())
    }

    fn draw(
        &mut self,
        connection: &x11rb::xcb_ffi::XCBConnection,
        screen_num: usize,
        state: &mut mdry::State,
        offset: f32,
    ) -> Result<(), crate::Error> {
        state.draw_text_absolute(
            &self.current_time.format("%H:%M:%S").to_string(),
            20. + offset,
            0.,
            self.color,
            self.font_size,
        );

        Ok(())
    }
}
