use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

use chrono::Local;
use crossbeam::channel::Sender;
use glyphon::{Attrs, Metrics, Shaping};
use mdry::{
    color::Color,
    renderer::{measure_text, Font, TextInner},
};
use smol::stream::StreamExt;
use sysinfo::{CpuExt, SystemExt};

use super::Widget;

pub struct CPUUsage {
    font_size: f32,
    color: Color,
    text: Option<Arc<TextInner>>,
    system: sysinfo::System,
}

impl CPUUsage {
    pub fn new(font_size: f32, color: Color) -> Self {
        let system = sysinfo::System::new_with_specifics(
            sysinfo::RefreshKind::new().with_cpu(sysinfo::CpuRefreshKind::new().with_cpu_usage()),
        );
        Self {
            font_size,
            color,
            text: None,
            system,
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
                self.system.refresh_cpu();
                inner.x = offset;
                inner.content = format!(" {}%", self.system.global_cpu_info().cpu_usage() as u32);
                inner.buffer.set_text(
                    state.font_system_mut(),
                    &inner.content,
                    Attrs::new().family(inner.font.family.into_glyphon_family()),
                    Shaping::Advanced,
                );

                let (width, height) = measure_text(&inner.buffer);
                inner.bounds.left = inner.x as i32;
                inner.bounds.right = (inner.x + width) as i32;
                inner
                    .buffer
                    .set_size(state.font_system_mut(), width, height);

                self.text = Some(Arc::new(inner));
            }
            Err(_inner_arc) => {
                let width = state.width as f32;
                let height = state.height as f32;
                let scale = state.window.display_scale;
                self.text = Some(Arc::new(TextInner::new(
                    state.font_system_mut(),
                    &format!(" {}%", self.system.global_cpu_info().cpu_usage() as u32),
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
