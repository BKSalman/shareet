use std::sync::Arc;

use mdry::{color::Color, window::Window};
use shareet::{
    create_window,
    widgets::{cpu_usage::CPUUsage, pager::Pager, sys_time::SysTime, sys_tray::SysTray},
    Bar, Error,
};
use x11rb::{
    connection::Connection,
    protocol::{
        xproto::{ChangeWindowAttributesAux, ConnectionExt, EventMask},
        Event,
    },
    xcb_ffi::XCBConnection,
};

#[cfg(feature = "profiling")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() -> Result<(), Error> {
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
    let height = 35;

    // let width = 100;
    // let height = 100;

    let display_scale = 1.;

    let window = create_window(&connection, width, height, screen_num, display_scale, false)?;

    let mut bar = pollster::block_on(run(window));

    connection.flush()?;

    let change = ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE);

    connection
        .change_window_attributes(screen.root, &change)?
        .check()?;

    let foreground = Color::rgb(191, 189, 182);
    let background = Color::rgb(26, 29, 36);

    bar.widgets.push(Box::new(Pager::new(
        &connection,
        glyphon::Metrics::new(bar.state.height as f32, bar.state.height as f32),
        foreground,
        Color::rgb(233, 86, 120),
        5.,
    )?));

    bar.widgets.push(Box::new(SysTray::new(
        &connection,
        screen_num,
        bar.state.width,
        bar.state.height,
        20,
        5,
        background,
    )?));

    bar.widgets
        .push(Box::new(SysTime::new(bar.state.height as f32, foreground)));

    // XXX: broken
    // bar.widgets
    //     .push(Box::new(CPUUsage::new(bar.state.height as f32, foreground)));

    let (event_sender, event_receiver) = crossbeam::channel::unbounded::<Event>();
    let (redraw_sender, redraw_receiver) = crossbeam::channel::unbounded::<()>();

    for widget in bar.widgets.iter_mut() {
        widget
            .setup(
                &mut bar.state,
                &connection,
                screen_num,
                redraw_sender.clone(),
            )
            .unwrap();
    }

    {
        let connection = connection.clone();
        std::thread::spawn(move || {
            loop {
                #[cfg(feature = "profiling")]
                match receiver.try_recv() {
                    Ok(_) => {
                        drop(profiler);
                        std::process::exit(0);
                    }
                    Err(_) => {}
                }

                let event = connection.wait_for_event().unwrap();
                let mut event_option = Some(event);
                while let Some(event) = event_option {
                    // if matches!(event, Event::PropertyNotify(_)) {
                    //     println!("got event: {event:#?}");
                    // }

                    event_sender.send(event).unwrap();

                    event_option = connection.poll_for_event().unwrap();
                }
            }
        });
    }
    loop {
        crossbeam::select! {
            recv(event_receiver) -> event => {
                if let Ok(event) = event {

                match event {
                    Event::ClientMessage(event) => {
                        if event.data.as_data32()[0] == bar.state.window.atoms.WM_DELETE_WINDOW {
                            return Ok(());
                        }
                    }
                    Event::PropertyNotify(event) if event.window == screen.root => {
                        redraw_sender.send(()).unwrap();
                    }
                    Event::Expose(_) => redraw_sender.send(())?,
                    Event::LeaveNotify(_) => redraw_sender.send(())?,
                    Event::EnterNotify(_) => redraw_sender.send(())?,
                    Event::ConfigureNotify(_) => redraw_sender.send(())?,
                    _ => {}
                }

                for widget in bar.widgets.iter_mut() {
                    if let Err(e) =
                        widget.on_event(&connection, screen_num, &mut bar.state, event.clone(), redraw_sender.clone())
                    {
                        eprintln!("widget error: {e}");
                    }
                }
                }
            },
            recv(redraw_receiver) -> _ => {
                let width = bar.state.width as f32;
                bar.state.clear_background(background);
                let mut roffset = 0.;
                let mut loffset = 0.;
                for widget in bar.widgets.iter_mut() {
                    let size = widget.size(&mut bar.state);
                    match widget.alignment() {
                        shareet::widgets::Alignment::Left => {
                            widget.draw(&connection, screen_num, &mut bar.state, loffset)?;
                            loffset += size;
                        },
                        shareet::widgets::Alignment::Right => {
                            widget.draw(&connection, screen_num, &mut bar.state, width - roffset - size)?;
                            roffset += size;
                        },
                    }
                }
                bar.state.update()?;
                match bar.state.render() {
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
            }
        }
    }
}

async fn run<'a>(window: Window<'a>) -> Bar<'a> {
    Bar::new(window).await
}
