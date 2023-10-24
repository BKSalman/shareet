use std::sync::Arc;

use shareet::{create_window, widgets::pager::Pager, Bar, Error};
use x11rb::{
    connection::Connection,
    protocol::{
        xproto::{ChangeWindowAttributesAux, ConfigureWindowAux, ConnectionExt, EventMask},
        Event,
    },
    xcb_ffi::XCBConnection,
};

#[cfg(feature = "profiling")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    let _ = pollster::block_on(run());
}

async fn run() -> Result<(), Error> {
    #[cfg(feature = "profiling")]
    let profiler = dhat::Profiler::new_heap();
    #[cfg(feature = "profiling")]
    println!("Profiling...");

    #[cfg(feature = "profiling")]
    let (sender, receiver) = std::sync::mpsc::channel();

    #[cfg(feature = "profiling")]
    ctrlc::set_handler(move || sender.send(()).expect("Could not send signal on channel."))
        .expect("Error setting Ctrl-C handler");

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

    let mut redraw = true;

    connection.flush()?;

    let change = ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE);

    connection
        .change_window_attributes(screen.root, &change)?
        .check()?;

    let pager = Pager::new(
        &connection,
        glyphon::Metrics::new(bar.state.height as f32, bar.state.height as f32),
        glyphon::Color::rgb(0, 0, 0),
        5,
    )?;

    bar.widgets.push(Box::new(pager));

    let pager = Pager::new(
        &connection,
        glyphon::Metrics::new(bar.state.height as f32, bar.state.height as f32),
        glyphon::Color::rgb(0, 0, 0),
        5,
    )?;

    bar.widgets.push(Box::new(pager));

    for widget in bar.widgets.iter_mut() {
        widget
            .setup(&mut bar.state, &connection, screen_num)
            .unwrap();
    }

    loop {
        #[cfg(feature = "profiling")]
        match receiver.try_recv() {
            Ok(_) => {
                drop(profiler);
                std::process::exit(0);
            }
            Err(_) => {}
        }
        if redraw {
            let (meshes, texts, _) = bar.widgets.iter().fold(
                (Vec::new(), Vec::new(), 0.),
                |(mut meshes, mut texts, offset), w| {
                    meshes.extend(w.meshes().into_iter().map(|m| (m, offset)));
                    texts.extend(
                        w.texts(&mut bar.state.text_renderer.font_system)
                            .into_iter()
                            .map(|t| (t, offset)),
                    );
                    let size = w.size();
                    return (meshes, texts, offset + size as f32);
                },
            );

            bar.state.update(meshes.clone(), texts)?;
            match bar.state.render(meshes) {
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
