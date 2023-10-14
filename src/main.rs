use std::sync::Arc;

use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache};
use idkman::{Atoms, State, Window};
use x11rb::{
    connection::Connection,
    protocol::{
        xproto::{
            AtomEnum, ConfigureWindowAux, ConnectionExt, CreateGCAux, CreateWindowAux, EventMask,
            PropMode, WindowClass,
        },
        Event,
    },
    wrapper::ConnectionExt as _,
    xcb_ffi::XCBConnection,
    COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT,
};

type Error = Box<dyn std::error::Error>;

fn main() {
    let _ = pollster::block_on(run());
}

async fn run() -> Result<(), Error> {
    let (connection, screen_num) = XCBConnection::connect(None)?;

    let connection = Arc::new(connection);

    let screen = &connection.setup().roots[screen_num];

    let atoms = Atoms::new(connection.as_ref())?.reply()?;

    let width = screen.width_in_pixels;
    let height = 30;

    let width = 100;
    let height = 100;

    let window_id = connection.generate_id()?;

    let create = CreateWindowAux::new()
        .event_mask(
            EventMask::EXPOSURE
                | EventMask::STRUCTURE_NOTIFY
                | EventMask::VISIBILITY_CHANGE
                | EventMask::KEY_PRESS
                | EventMask::KEY_RELEASE
                | EventMask::KEYMAP_STATE
                | EventMask::BUTTON_PRESS
                | EventMask::BUTTON_RELEASE
                | EventMask::POINTER_MOTION,
        )
        .background_pixel(0);

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
            b"lmao",
        )?
        .check()?;

    // connection
    //     .change_property32(
    //         PropMode::REPLACE,
    //         window_id,
    //         atoms._NET_WM_WINDOW_TYPE,
    //         AtomEnum::ATOM,
    //         &[atoms._NET_WM_WINDOW_TYPE_DOCK],
    //     )?
    //     .check()?;

    // connection
    //     .change_property32(
    //         PropMode::REPLACE,
    //         window_id,
    //         atoms._NET_WM_STATE,
    //         AtomEnum::ATOM,
    //         &[atoms._NET_WM_STATE_STICKY, atoms._NET_WM_STATE_ABOVE],
    //     )?
    //     .check()?;

    // connection
    //     .change_property32(
    //         PropMode::REPLACE,
    //         window_id,
    //         atoms._NET_WM_STRUT_PARTIAL,
    //         AtomEnum::CARDINAL,
    //         // left, right, top, bottom, left_start_y, left_end_y,
    //         // right_start_y, right_end_y, top_start_x, top_end_x, bottom_start_x,
    //         // bottom_end_x
    //         &[
    //             0,
    //             0,
    //             0,
    //             height as u32,
    //             0,
    //             0,
    //             0,
    //             0,
    //             0,
    //             0,
    //             0,
    //             screen.width_in_pixels as u32,
    //         ],
    //     )?
    //     .check()?;

    // let picture = connection.generate_id()?;

    // let create = CreatePictureAux::new()
    //     .repeat(Repeat::NORMAL)
    //     // .graphicsexposure(0xff_ff_ff_ff)
    //     .polymode(PolyMode::IMPRECISE);

    // connection.render_create_picture(picture, window, format_id, &create)?;

    let display_scale = 1.;

    let mut font_system = FontSystem::new();

    let mut buffer = Buffer::new_empty(Metrics::new(32.0, 44.0).scale(display_scale));

    let mut buffer = buffer.borrow_with(&mut font_system);

    buffer.set_size(width as f32, height as f32);

    let attrs = Attrs::new().family(Family::Monospace);

    buffer.set_text("lmao", attrs, Shaping::Basic);

    buffer.shape_until_scroll();

    let mut swash_cache = SwashCache::new();

    let font_color = Color::rgb(0xFF, 0xFF, 0xFF);

    let gc = connection.generate_id()?;

    let create = CreateGCAux::new();

    connection.create_gc(gc, window_id, &create)?;

    connection.map_window(window_id)?;

    let window = Window {
        xid: window_id,
        connection: connection.as_ref(),
        screen_num,
        width: width as u32,
        height: height as u32,
        atoms,
    };

    let mut state = State::new(window).await;

    let mut redraw = true;

    loop {
        connection.flush()?;
        if redraw {
            state.update();
            match state.render() {
                Ok(_) => {}
                // Reconfigure the surface if lost
                Err(wgpu::SurfaceError::Lost) => state.resize(state.width, state.height),
                // The system is out of memory, we should probably quit
                Err(wgpu::SurfaceError::OutOfMemory) => return Ok(()),
                // All other errors (Outdated, Timeout) should be resolved by the next frame
                Err(e) => eprintln!("{:?}", e),
            }

            redraw = false;
        }

        let event = connection.wait_for_event()?;
        let mut event_option = Some(event);
        while let Some(event) = event_option {
            if !matches!(event, Event::MotionNotify(_)) {
                println!("got event: {event:#?}");
            }
            match event {
                Event::ClientMessage(event) => {
                    println!("client message: {event:#?}");
                }
                Event::MotionNotify(_) => {
                    redraw = true;
                }
                Event::Expose(event) => {
                    let width = event.width as u32;
                    let height = event.height as u32;
                    let configure = ConfigureWindowAux::new().width(width).height(height);
                    connection.configure_window(event.window, &configure)?;
                    state.resize(event.width as u32, event.height as u32);
                    redraw = true;
                }
                Event::LeaveNotify(_) => {
                    redraw = true;
                }
                Event::EnterNotify(_) => {
                    redraw = true;
                }
                Event::ResizeRequest(event) => {
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
