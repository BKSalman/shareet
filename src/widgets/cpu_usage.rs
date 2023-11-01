use std::{sync::Arc, time::Duration};

use chrono::Local;
use crossbeam::channel::{Receiver, Sender};
use glyphon::{Attrs, Shaping};
use mdry::{
    color::Color,
    renderer::{measure_text, Font, TextInner},
};
use smol::stream::StreamExt;
use systemstat::{CPULoad, Platform};

use super::Widget;

pub struct CPUUsage {
    font_size: f32,
    color: Color,
    text: Option<Arc<TextInner>>,
    cpu_load_sender: Sender<CPULoad>,
    cpu_load_receiver: Receiver<CPULoad>,
}

impl CPUUsage {
    pub fn new(font_size: f32, color: Color) -> Self {
        let (cpu_load_sender, cpu_load_receiver) = crossbeam::channel::unbounded();
        Self {
            font_size,
            color,
            text: None,
            cpu_load_sender,
            cpu_load_receiver,
        }
    }
}

impl Widget for CPUUsage {
    fn setup(
        &mut self,
        state: &mut mdry::State,
        _connection: &x11rb::xcb_ffi::XCBConnection,
        _screen_num: usize,
        redraw_sender: Sender<()>,
    ) -> Result<(), crate::Error> {
        let width = state.width as f32;
        let height = state.height as f32;
        let scale = state.window.display_scale;
        let text = Arc::new(TextInner::new(
            state.font_system_mut(),
            &Local::now().format("%H:%M:%S").to_string(),
            0.,
            0.,
            width * scale,
            height * scale,
            self.font_size,
            self.color,
            Font::DEFAULT,
        ));

        self.text = Some(text);

        {
            let cpu_load_sender = self.cpu_load_sender.clone();
            std::thread::spawn(move || {
                smol::block_on(async {
                    let system = systemstat::System::new();
                    loop {
                        let measurement =
                            system.cpu_load_aggregate().expect("could not get cpu info");
                        smol::Timer::interval(Duration::from_secs(1)).next().await;
                        let _ = cpu_load_sender
                            .send(measurement.done().expect("could not read cpu load"));
                        redraw_sender.send(()).unwrap();
                    }
                });
            });
        }

        Ok(())
    }

    fn on_event(
        &mut self,
        _connection: &x11rb::xcb_ffi::XCBConnection,
        _screen_num: usize,
        _state: &mut mdry::State,
        _event: x11rb::protocol::Event,
        _redraw_sender: Sender<()>,
    ) -> Result<(), crate::Error> {
        Ok(())
    }

    fn draw(
        &mut self,
        _connection: &x11rb::xcb_ffi::XCBConnection,
        _screen_num: usize,
        state: &mut mdry::State,
        offset: f32,
    ) -> Result<(), crate::Error> {
        let text = self.text.take().expect("text should always be initialized");
        match Arc::try_unwrap(text) {
            Ok(mut inner) => {
                if let Ok(cpu_load) = self.cpu_load_receiver.try_recv() {
                    inner.x = offset;
                    inner.content = format!(" {}%", (cpu_load.user * 100.) as u32);
                    inner.buffer.set_text(
                        state.font_system_mut(),
                        &inner.content,
                        Attrs::new().family(inner.font.family.into_glyphon_family()),
                        Shaping::Advanced,
                    );

                    let (width, _height) = measure_text(&inner.buffer);
                    inner.bounds.left = inner.x as i32;
                    inner.bounds.right = (inner.x + width) as i32;
                }

                self.text = Some(Arc::new(inner));
            }
            Err(_inner_arc) => {
                let width = state.width as f32;
                let height = state.height as f32;
                let scale = state.window.display_scale;
                self.text = Some(Arc::new(TextInner::new(
                    state.font_system_mut(),
                    &String::from(" 0%"),
                    0.,
                    0.,
                    width * scale,
                    height * scale,
                    self.font_size,
                    self.color,
                    Font::DEFAULT,
                )));
            }
        }

        if let Some(text) = &self.text {
            state.draw_text_absolute(text.clone());
        }

        Ok(())
    }

    fn size(&mut self, _state: &mut mdry::State) -> f32 {
        let text = self.text.take().expect("text should always be initialized");
        let size = match Arc::try_unwrap(text) {
            Ok(inner) => {
                let (width, _height) = measure_text(&inner.buffer);
                println!("width: {width}");
                self.text = Some(Arc::new(inner));

                width
            }
            Err(inner_arc) => {
                // TODO: replace the whole thing
                self.text = Some(inner_arc);
                0.
            }
        };

        size + 10.
    }

    fn alignment(&self) -> super::Alignment {
        super::Alignment::Right
    }
}
