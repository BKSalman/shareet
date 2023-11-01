use std::{sync::Arc, time::Duration};

use chrono::Local;
use crossbeam::channel::Sender;
use glyphon::{Attrs, FontSystem, Metrics, Shaping};
use mdry::{
    color::Color,
    renderer::{measure_text, Font, TextInner},
};
use smol::stream::StreamExt;

use super::Widget;

pub struct SysTime {
    font_size: f32,
    color: Color,
    text: Option<Arc<TextInner>>,
    x: f32,
}

impl SysTime {
    pub fn new(font_size: f32, color: Color) -> Self {
        Self {
            font_size,
            color,
            text: None,
            x: 0.,
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
        let text = Arc::new(TextInner::new(
            state.font_system_mut(),
            &Local::now().format("%H:%M:%S").to_string(),
            0.,
            0.,
            100.,
            100.,
            self.font_size,
            self.color,
            Font::DEFAULT,
        ));
        let mut text_buffer = glyphon::Buffer::new(
            state.font_system_mut(),
            Metrics::new(self.font_size, self.font_size),
        );

        let physical_width = state.width as f32 * state.window.display_scale;
        let physical_height = state.height as f32 * state.window.display_scale;

        text_buffer.set_size(state.font_system_mut(), physical_width, physical_height);
        text_buffer.set_text(
            state.font_system_mut(),
            &text.content,
            Attrs::new().family(text.font.family.into_glyphon_family()),
            Shaping::Advanced,
        );

        let (width, height) = measure_text(&text_buffer);

        text_buffer.set_size(state.font_system_mut(), width, height);

        self.text = Some(text);

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
        let text = self.text.take().expect("text should always be initialized");
        match Arc::try_unwrap(text) {
            Ok(mut inner) => {
                inner.content = Local::now().format("%H:%M:%S").to_string();
                inner.buffer.set_text(
                    state.font_system_mut(),
                    &inner.content,
                    Attrs::new().family(inner.font.family.into_glyphon_family()),
                    Shaping::Advanced,
                );

                let (width, height) = measure_text(&inner.buffer);
                inner.bounds.right = (inner.x + width) as i32;
                inner.bounds.bottom = (inner.y + height) as i32;
                inner
                    .buffer
                    .set_size(state.font_system_mut(), width, height);
                inner.x = offset + 50.;

                self.text = Some(Arc::new(inner));
            }
            Err(inner_arc) => {
                // TODO: replace the whole thing
                self.text = Some(inner_arc);
            }
        }

        if let Some(text) = &self.text {
            state.draw_text_absolute(text.clone());
        }

        Ok(())
    }
}
