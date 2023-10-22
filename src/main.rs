use std::sync::Arc;

use shareet::{
    create_window,
    shapes::{Rect, Shape},
    widgets::Widget,
    Bar, Error,
};
use x11rb::{
    connection::Connection,
    protocol::{
        xproto::{ChangeWindowAttributesAux, ConfigureWindowAux, ConnectionExt, EventMask},
        Event,
    },
    xcb_ffi::XCBConnection,
};

fn main() {
    let _ = pollster::block_on(run());
}

async fn run() -> Result<(), Error> {
    let (connection, screen_num) = XCBConnection::connect(None)?;

    let connection = Arc::new(connection);

    let screen = &connection.setup().roots[screen_num];

    let width = screen.width_in_pixels;
    let height = 30;

    // let width = 100;
    // let height = 100;

    let display_scale = 1.;

    let window = create_window(&connection, width, height, screen_num, display_scale)?;

    let mut bar = Bar::new(window, screen).await;

    bar.state.painter.add_shape_absolute(
        Shape::Rect(Rect {
            x: 0,
            y: 0,
            width: 500,
            height: height as u32,
        }),
        shareet::Color::rgb(255, 255, 255),
    );

    let mut redraw = true;

    connection.flush()?;

    let change = ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE);

    connection
        .change_window_attributes(screen.root, &change)?
        .check()?;

    let mut pager = shareet::widgets::pager::Pager {
        text_metrics: glyphon::Metrics::new(bar.state.height as f32, bar.state.height as f32),
        text_color: glyphon::Color::rgb(0, 0, 0),
        selector_mesh: None,
        desktops: vec![],
        atoms: shareet::widgets::pager::PagerAtoms::new(connection.as_ref())?.reply()?,
        requires_redraw: true,
        texts: vec![],
    };

    pager.setup(&bar.state, &connection, screen_num).unwrap();

    bar.widgets.push(Box::new(pager));

    loop {
        if redraw {
            let (meshes, texts) =
                bar.widgets
                    .iter()
                    .fold((Vec::new(), Vec::new()), |mut acc, w| {
                        if w.requires_redraw() {
                            acc.0.extend(w.meshes());
                            acc.1
                                .extend(w.texts(&mut bar.state.text_renderer.font_system));
                        }
                        return acc;
                    });
            bar.state.update(&meshes, &texts)?;
            match bar.state.render(&meshes) {
                Ok(_) => {}
                // Reconfigure the surface if lost
                Err(wgpu::SurfaceError::Lost) => {
                    bar.state.resize(bar.state.width, bar.state.height)
                }
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
            // if matches!(event, Event::PropertyNotify(_)) {
            //     println!("got event: {event:#?}");
            // }
            match event {
                Event::ClientMessage(event) => {
                    if event.data.as_data32()[0] == bar.state.window.atoms.WM_DELETE_WINDOW {
                        return Ok(());
                    }

                    // println!("client message: {event:#?}");
                }
                Event::PropertyNotify(event) if event.window == screen.root => {
                    redraw = true;
                }
                Event::MotionNotify(_) => redraw = true,
                Event::Expose(event) => {
                    let width = event.width as u32;
                    let height = event.height as u32;
                    let configure = ConfigureWindowAux::new().width(width).height(height);
                    connection.configure_window(event.window, &configure)?;
                    bar.state.resize(width, height);

                    redraw = true;
                }
                Event::LeaveNotify(_) => redraw = true,
                Event::EnterNotify(_) => redraw = true,
                // Event::ResizeRequest(event) => {
                //     let width = event.width as u32;
                //     let height = event.height as u32;
                //     let configure = ConfigureWindowAux::new().width(width).height(height);
                //     connection.configure_window(event.window, &configure)?;
                //     state.resize(width, height);

                //     println!("resize");

                //     request_redraw(&mut redraw)
                // }
                Event::ConfigureNotify(event) => {
                    let width = event.width as u32;
                    let height = event.height as u32;
                    bar.state.resize(width, height);

                    redraw = true;
                }
                _ => {}
            }

            for widget in bar.widgets.iter_mut() {
                if let Err(e) = widget.on_event(&connection, &mut bar.state, event.clone()) {
                    eprintln!("widget error: {e}");
                }
            }

            event_option = connection.poll_for_event()?;
        }
    }
}
