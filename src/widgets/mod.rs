use crossbeam::channel::Sender;
use x11rb::xcb_ffi::XCBConnection;

use mdry::State;

pub mod pager;
pub mod sys_time;
pub mod sys_tray;
pub mod text;

pub trait Widget {
    fn setup(
        &mut self,
        state: &mut State,
        connection: &XCBConnection,
        screen_num: usize,
        redraw_sender: Sender<()>,
    ) -> Result<(), crate::Error>;
    fn on_event(
        &mut self,
        connection: &XCBConnection,
        screen_num: usize,
        state: &mut State,
        event: x11rb::protocol::Event,
        redraw_sender: Sender<()>,
    ) -> Result<(), crate::Error>;

    fn draw(
        &mut self,
        connection: &XCBConnection,
        screen_num: usize,
        state: &mut State,
        offset: f32,
    ) -> Result<(), crate::Error>;

    fn size(&self, state: &mut State) -> f32 {
        0.
    }

    fn requires_redraw(&self) -> bool {
        true
    }
}
