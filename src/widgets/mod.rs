use glyphon::FontSystem;
use x11rb::xcb_ffi::XCBConnection;

use crate::{shapes::Mesh, text_renderer::Text, State};

pub mod pager;
pub mod text;

pub trait Widget {
    fn setup(
        &mut self,
        state: &mut State,
        connection: &XCBConnection,
        screen_num: usize,
    ) -> Result<(), crate::Error>;
    fn on_event(
        &mut self,
        connection: &XCBConnection,
        state: &mut State,
        event: x11rb::protocol::Event,
    ) -> Result<(), crate::Error>;

    fn meshes(&self) -> Vec<&Mesh> {
        vec![]
    }
    fn texts(&self, _font_system: &mut FontSystem) -> Vec<&Text> {
        vec![]
    }
    fn size(&self) -> u32 {
        0
    }
    fn requires_redraw(&self) -> bool {
        true
    }
}
