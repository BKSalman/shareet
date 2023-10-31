use std::time::Duration;

use chrono::{DateTime, Local};
use crossbeam::channel::Sender;
use mdry::color::Color;
use smol::stream::StreamExt;

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
        redraw_sender: Sender<()>,
    ) -> Result<(), crate::Error> {
        std::thread::spawn(move || {
            smol::block_on(async {
                loop {
                    smol::Timer::interval(Duration::from_secs(1)).next().await;
                    redraw_sender.send(()).unwrap();
                }
            });
        });

        Ok(())
    }

    fn on_event(
        &mut self,
        connection: &x11rb::xcb_ffi::XCBConnection,
        screen_num: usize,
        state: &mut mdry::State,
        event: x11rb::protocol::Event,
        redraw_sender: Sender<()>,
    ) -> Result<(), crate::Error> {
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
            &Local::now().format("%H:%M:%S").to_string(),
            20. + offset,
            0.,
            self.color,
            self.font_size,
        );

        Ok(())
    }
}
