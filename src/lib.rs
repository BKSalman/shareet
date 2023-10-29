use mdry::window::{Window, WindowType};
use mdry::State;

use widgets::Widget;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, PropMode, WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;
use x11rb::xcb_ffi::XCBConnection;
use x11rb::{COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT};

pub mod widgets;

pub type Error = Box<dyn std::error::Error>;

pub trait Vertex: bytemuck::Pod + bytemuck::Zeroable {
    fn desc() -> wgpu::VertexBufferLayout<'static>;
}

pub struct Bar<'a> {
    pub state: State<'a>,
    pub widgets: Vec<Box<dyn Widget>>,
}

impl<'a> Bar<'a> {
    pub async fn new(window: mdry::window::Window<'a>) -> Bar<'a> {
        let state = State::new(window).await;
        Self {
            state,
            widgets: vec![],
        }
    }
}

pub fn create_window(
    connection: &XCBConnection,
    width: u16,
    height: u16,
    screen_num: usize,
    display_scale: f32,
    bottom: bool,
) -> Result<Window, Error> {
    let screen = &connection.setup().roots[screen_num];

    let atoms = mdry::window::Atoms::new(connection)?.reply()?;

    let window_id = connection.generate_id()?;

    let create = CreateWindowAux::new().event_mask(
        EventMask::EXPOSURE
            | EventMask::STRUCTURE_NOTIFY
            | EventMask::VISIBILITY_CHANGE
            | EventMask::KEY_PRESS
            | EventMask::KEY_RELEASE
            | EventMask::KEYMAP_STATE
            | EventMask::BUTTON_PRESS
            | EventMask::BUTTON_RELEASE
            | EventMask::POINTER_MOTION
            | EventMask::PROPERTY_CHANGE,
    );

    let (y, struts) = if bottom {
        (
            (screen.height_in_pixels - height) as i16,
            // left, right, top, bottom, left_start_y, left_end_y,
            // right_start_y, right_end_y, top_start_x, top_end_x, bottom_start_x,
            // bottom_end_x
            [
                0,
                0,
                0,
                height as u32,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                screen.width_in_pixels as u32,
            ],
        )
    } else {
        (
            0,
            // left, right, top, bottom, left_start_y, left_end_y,
            // right_start_y, right_end_y, top_start_x, top_end_x, bottom_start_x,
            // bottom_end_x
            [
                0,
                0,
                height as u32,
                0,
                0,
                0,
                0,
                0,
                0,
                screen.width_in_pixels as u32,
                0,
                0,
            ],
        )
    };

    connection.create_window(
        COPY_DEPTH_FROM_PARENT,
        window_id,
        screen.root,
        0,
        y,
        width,
        height,
        0,
        WindowClass::INPUT_OUTPUT,
        COPY_FROM_PARENT,
        &create,
    )?;

    connection
        .change_property8(
            PropMode::REPLACE,
            window_id,
            atoms._NET_WM_NAME,
            AtomEnum::STRING,
            b"lmao",
        )?
        .check()?;

    connection
        .change_property8(
            PropMode::REPLACE,
            window_id,
            atoms.WM_NAME,
            AtomEnum::STRING,
            b"lmao",
        )?
        .check()?;

    connection
        .change_property8(
            PropMode::REPLACE,
            window_id,
            x11rb::protocol::xproto::Atom::from(x11rb::protocol::xproto::AtomEnum::WM_CLASS),
            AtomEnum::STRING,
            b"lmao",
        )?
        .check()?;

    connection
        .change_property32(
            PropMode::REPLACE,
            window_id,
            atoms._NET_WM_WINDOW_TYPE,
            AtomEnum::ATOM,
            &[atoms._NET_WM_WINDOW_TYPE_DOCK],
        )?
        .check()?;

    // connection
    //     .change_property32(
    //         PropMode::REPLACE,
    //         window_id,
    //         atoms.WM_PROTOCOLS,
    //         AtomEnum::ATOM,
    //         &[atoms.WM_DELETE_WINDOW, atoms._NET_WM_PING],
    //     )?
    //     .check()?;

    connection
        .change_property32(
            PropMode::REPLACE,
            window_id,
            atoms._NET_WM_STATE,
            AtomEnum::ATOM,
            &[atoms._NET_WM_STATE_STICKY, atoms._NET_WM_STATE_ABOVE],
        )?
        .check()?;

    connection
        .change_property32(
            PropMode::REPLACE,
            window_id,
            atoms._NET_WM_STRUT_PARTIAL,
            AtomEnum::CARDINAL,
            &struts,
        )?
        .check()?;

    connection.map_window(window_id)?;

    connection.flush()?;

    Ok(Window {
        xid: window_id,
        connection,
        screen_num,
        width: width as u32,
        height: height as u32,
        atoms,
        display_scale,
        x: 0,
        y: y.into(),
        window_type: WindowType::Dock { bottom, struts },
    })
}
