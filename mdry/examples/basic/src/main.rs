use std::sync::Arc;

use mdry::{
    color::Color,
    shapes::{Rect, Shape},
    window::{Atoms, Window},
    x11rb::{
        self,
        connection::Connection,
        protocol::xproto::{
            AtomEnum, ConfigureWindowAux, ConnectionExt as _, CreateWindowAux, EventMask, PropMode,
            WindowClass,
        },
        wrapper::ConnectionExt as _,
        xcb_ffi::XCBConnection,
        Event, COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT,
    },
    State,
};

type Error = Box<dyn std::error::Error>;

fn main() -> Result<(), Error> {
    let (connection, screen_num) = XCBConnection::connect(None)?;
    let connection = Arc::new(connection);
    let window = create_window(&connection, 500, 500, screen_num, 1.)?;

    let mut state = pollster::block_on(create_state(window));

    let mut redraw = true;
    loop {
        if redraw {
            state.update()?;
            state.render()?;
            redraw = false;
        }

        state.clear_background(Color::rgb(180, 170, 100));

        state.draw_shape_absolute(Shape::Rect(Rect {
            x: 20.,
            y: 20.,
            width: 20,
            height: 20,
            color: Color::rgb(0, 0, 0),
        }));

        state.draw_text("lmao", 40., 40., Color::rgb(0, 100, 0), 20.);

        let event = connection.wait_for_event()?;
        let mut event_option = Some(event);
        while let Some(event) = event_option {
            match event {
                Event::ClientMessage(event) => {
                    // window manager requested to close the window
                    if event.data.as_data32()[0] == state.window.atoms.WM_DELETE_WINDOW {
                        return Ok(());
                    }
                }
                Event::Expose(event) => {
                    let width = event.width as u32;
                    let height = event.height as u32;
                    let configure = ConfigureWindowAux::new().width(width).height(height);
                    connection.configure_window(event.window, &configure)?;
                    state.resize(width, height);

                    redraw = true;
                }
                Event::ConfigureNotify(event) => {
                    let width = event.width as u32;
                    let height = event.height as u32;
                    state.resize(width, height);

                    redraw = true;
                }
                _ => {}
            }

            event_option = connection.poll_for_event()?;
        }
    }
}

async fn create_state<'a>(window: Window<'a>) -> State {
    State::new(window).await
}

pub fn create_window(
    connection: &XCBConnection,
    width: u16,
    height: u16,
    screen_num: usize,
    display_scale: f32,
) -> Result<Window, Error> {
    let screen = &connection.setup().roots[screen_num];

    let atoms = Atoms::new(connection)?.reply()?;

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

    connection.create_window(
        COPY_DEPTH_FROM_PARENT,
        window_id,
        screen.root,
        0,
        (screen.height_in_pixels - height) as i16,
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
            b"simple",
        )?
        .check()?;

    connection
        .change_property8(
            PropMode::REPLACE,
            window_id,
            atoms.WM_NAME,
            AtomEnum::STRING,
            b"simple",
        )?
        .check()?;

    connection
        .change_property8(
            PropMode::REPLACE,
            window_id,
            x11rb::protocol::xproto::Atom::from(x11rb::protocol::xproto::AtomEnum::WM_CLASS),
            AtomEnum::STRING,
            b"simple",
        )?
        .check()?;

    connection
        .change_property32(
            PropMode::REPLACE,
            window_id,
            atoms._NET_WM_WINDOW_TYPE,
            AtomEnum::ATOM,
            &[atoms._NET_WM_WINDOW_TYPE_NORMAL],
        )?
        .check()?;

    connection
        .change_property32(
            PropMode::REPLACE,
            window_id,
            atoms.WM_PROTOCOLS,
            AtomEnum::ATOM,
            &[atoms.WM_DELETE_WINDOW, atoms._NET_WM_PING],
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
    })
}
