use crossbeam::channel::Sender;
use mdry::{color::Color, x11rb::Event, State};
use x11rb::{
    connection::Connection,
    protocol::xproto::{
        AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConfigureWindowAux, ConnectionExt,
        CreateWindowAux, EventMask, PropMode, SetMode, Window, WindowClass,
    },
    wrapper::ConnectionExt as _,
    xcb_ffi::XCBConnection,
    COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT, CURRENT_TIME,
};

use super::Widget;

// https://specifications.freedesktop.org/systemtray-spec/systemtray-spec-0.2.html#messages
// #define SYSTEM_TRAY_REQUEST_DOCK    0
// #define SYSTEM_TRAY_BEGIN_MESSAGE   1
// #define SYSTEM_TRAY_CANCEL_MESSAGE  2
const SYSTEM_TRAY_REQUEST_DOCK: u32 = 0;
const SYSTEM_TRAY_BEGIN_MESSAGE: u32 = 1;
const SYSTEM_TRAY_CANCEL_MESSAGE: u32 = 2;

// https://specifications.freedesktop.org/xembed-spec/xembed-spec-latest.html#idm45171900597248
// /* XEMBED messages */
// #define XEMBED_EMBEDDED_NOTIFY   0
// #define XEMBED_WINDOW_ACTIVATE   1
// #define XEMBED_WINDOW_DEACTIVATE 2
// #define XEMBED_REQUEST_FOCUS     3
// #define XEMBED_FOCUS_IN          4
// #define XEMBED_FOCUS_OUT         5
// #define XEMBED_FOCUS_NEXT        6
// #define XEMBED_FOCUS_PREV        7
// /* 8-9 were used for XEMBED_GRAB_KEY/XEMBED_UNGRAB_KEY */
// #define XEMBED_MODALITY_ON      10
// #define XEMBED_MODALITY_OFF     11
// #define XEMBED_REGISTER_ACCELERATOR     12
// #define XEMBED_UNREGISTER_ACCELERATOR   13
// #define XEMBED_ACTIVATE_ACCELERATOR     14
const XEMBED_EMBEDDED_NOTIFY: u32 = 0;
#[allow(unused)]
const XEMBED_WINDOW_ACTIVATE: u32 = 1;
#[allow(unused)]
const XEMBED_WINDOW_DEACTIVATE: u32 = 2;
#[allow(unused)]
const XEMBED_REQUEST_FOCUS: u32 = 3;
#[allow(unused)]
const XEMBED_FOCUS_IN: u32 = 4;
#[allow(unused)]
const XEMBED_FOCUS_OUT: u32 = 5;
#[allow(unused)]
const XEMBED_FOCUS_NEXT: u32 = 6;
#[allow(unused)]
const XEMBED_FOCUS_PREV: u32 = 7;
/* 8-9 were used for XEMBED_GRAB_KEY/XEMBED_UNGRAB_KEY */
#[allow(unused)]
const XEMBED_MODALITY_ON: u32 = 10;
#[allow(unused)]
const XEMBED_MODALITY_OFF: u32 = 11;
#[allow(unused)]
const XEMBED_REGISTER_ACCELERATOR: u32 = 12;
#[allow(unused)]
const XEMBED_UNREGISTER_ACCELERATOR: u32 = 13;
#[allow(unused)]
const XEMBED_ACTIVATE_ACCELERATOR: u32 = 14;

const XEMBED_VERSION: u32 = 0;
// /* Flags for _XEMBED_INFO */
// #define XEMBED_MAPPED                   (1 << 0)
// https://specifications.freedesktop.org/xembed-spec/xembed-spec-latest.html#lifecycle
/// If set the client should be mapped.
/// The embedder must track the flags field by selecting for PropertyNotify events on
/// the client and map and unmap the client appropriately
const XEMBED_MAPPED: u32 = 1 << 0;

pub struct SysTray {
    selection_owner: Window,
    tray_icons: Vec<TrayIcon>,
    _net_system_tray_s: u32,
    icons_size: u32,
    padding: u32,
    background_color: Color,
}

#[derive(Debug)]
struct TrayIcon {
    embedded_window: Window,
    wrapper_window: Window,
    should_be_mapped: bool,
    should_be_unmapped: bool,
    has_been_mapped: bool,
}

type Error = Box<dyn std::error::Error>;

impl SysTray {
    pub fn new(
        connection: &XCBConnection,
        screen_num: usize,
        bar_width: u32,
        bar_height: u32,
        icons_size: u32,
        padding: u32,
        background_color: Color,
    ) -> Result<Self, Error> {
        let create = CreateWindowAux::new();
        let win_id = connection.generate_id()?;
        connection
            .create_window(
                COPY_DEPTH_FROM_PARENT,
                win_id,
                connection.setup().roots[screen_num].root,
                bar_width as i16,
                0,
                1,
                bar_height as u16,
                0,
                WindowClass::INPUT_OUTPUT,
                COPY_FROM_PARENT,
                &create,
            )?
            .check()?;

        let atom_name = format!("_NET_SYSTEM_TRAY_S{}", screen_num);

        let _net_system_tray_s = connection
            .intern_atom(false, atom_name.as_bytes())?
            .reply()?
            .atom;

        Ok(Self {
            selection_owner: win_id,
            tray_icons: Vec::new(),
            _net_system_tray_s,
            icons_size,
            padding,
            background_color,
        })
    }

    fn embed_client(
        &mut self,
        connection: &XCBConnection,
        message_data: [u32; 5],
        state: &State,
    ) -> Result<(), Error> {
        // begin embedding life cycle in XEMBED specification
        // https://specifications.freedesktop.org/xembed-spec/xembed-spec-latest.html#lifecycle
        let message = message_data[1];
        if message == SYSTEM_TRAY_REQUEST_DOCK {
            let embedded_window = message_data[2];
            if self
                .tray_icons
                .iter()
                .find(|ti| ti.embedded_window == embedded_window)
                .is_some()
            {
                eprintln!("Tray client {embedded_window} is already embedded, ignoring request...");
                return Ok(());
            }

            let configure = ConfigureWindowAux::new().width(20).height(20);

            connection
                .configure_window(embedded_window, &configure)?
                .check()?;

            let attrs = ChangeWindowAttributesAux::new().event_mask(EventMask::STRUCTURE_NOTIFY);
            connection
                .change_window_attributes(embedded_window, &attrs)?
                .check()?;

            // create a wrapper window to match the depth, visual to be able to reparent it
            // and also match the  geometry of the embedded window
            let wrapper_window = connection.generate_id()?;

            let create =
                CreateWindowAux::new().background_pixel(self.background_color.to_argb_u32());

            let y = ((state.height / 2) - self.icons_size / 2) as i16;

            connection
                .create_window(
                    COPY_DEPTH_FROM_PARENT,
                    wrapper_window,
                    state.window.xid,
                    0,
                    y,
                    20,
                    20,
                    0,
                    WindowClass::INPUT_OUTPUT,
                    COPY_FROM_PARENT,
                    &create,
                )?
                .check()?;

            connection
                .change_save_set(SetMode::INSERT, embedded_window)?
                .check()?;

            connection
                .reparent_window(embedded_window, wrapper_window, 0, 0)?
                .check()?;

            let mut tray_icon = TrayIcon {
                embedded_window,
                wrapper_window,
                should_be_mapped: false,
                has_been_mapped: false,
                should_be_unmapped: false,
            };

            // get version from client/embedded window in the _XEMBED_INFO property
            let xembed_info = connection
                .get_property(
                    false,
                    embedded_window,
                    state.window.atoms._XEMBED_INFO,
                    state.window.atoms._XEMBED_INFO,
                    0,
                    2,
                )?
                .reply()?;

            // xembed_info[0]: version
            // xembed_info[1]: flags (currently only has XEMBED_MAPPED flag)
            let xembed_info = xembed_info
                .value32()
                .ok_or("Failed to get XEMBED_INFO")?
                .collect::<Vec<_>>();

            // send the embedder(wrapper) window id in a XEMBED_EMBEDDED_NOTIFY message
            // with the minimum supported xembed version (currently it's always 0)
            let send_event = ClientMessageEvent::new(
                32,
                embedded_window,
                state.window.atoms._XEMBED,
                [
                    CURRENT_TIME,           // x_time
                    XEMBED_EMBEDDED_NOTIFY, // message
                    0,                      // detail (idk what's this)
                    wrapper_window,         // data1
                    XEMBED_VERSION,         // data2
                ],
            );

            connection
                .send_event(false, embedded_window, EventMask::NO_EVENT, send_event)?
                .check()?;

            let mapped = xembed_info[1];

            if mapped == XEMBED_MAPPED {
                tray_icon.should_be_mapped = true;
            }

            self.tray_icons.push(tray_icon);
        } else if message == SYSTEM_TRAY_BEGIN_MESSAGE {
            println!("got SYSTEM_TRAY_BEGIN_MESSAGE");
        } else if message == SYSTEM_TRAY_CANCEL_MESSAGE {
            println!("got SYSTEM_TRAY_CANCEL_MESSAGE");
        }

        Ok(())
    }
}

impl Widget for SysTray {
    fn setup(
        &mut self,
        state: &mut mdry::State,
        connection: &XCBConnection,
        screen_num: usize,
        _redraw_sender: Sender<()>,
    ) -> Result<(), crate::Error> {
        let screen = &connection.setup().roots[screen_num];
        connection
            .change_property32(
                PropMode::REPLACE,
                self.selection_owner,
                state.window.atoms._NET_SYSTEM_TRAY_COLORS,
                AtomEnum::CARDINAL,
                &[26, 29, 36],
            )?
            .check()?;

        connection
            .change_property32(
                PropMode::REPLACE,
                self.selection_owner,
                state.window.atoms._NET_SYSTEM_TRAY_ORIENTATION,
                AtomEnum::CARDINAL,
                &[state.window.atoms._NET_SYSTEM_TRAY_ORIENTATION_HORZ],
            )?
            .check()?;

        connection
            .change_property32(
                PropMode::REPLACE,
                self.selection_owner,
                state.window.atoms._NET_WM_WINDOW_TYPE,
                AtomEnum::ATOM,
                &[state.window.atoms._NET_WM_WINDOW_TYPE_DOCK],
            )?
            .check()?;

        connection
            .change_property32(
                PropMode::REPLACE,
                self.selection_owner,
                state.window.atoms._NET_WM_STRUT_PARTIAL,
                AtomEnum::CARDINAL,
                // left, right, top, bottom, left_start_y, left_end_y,
                // right_start_y, right_end_y, top_start_x, top_end_x, bottom_start_x,
                // bottom_end_x
                &[
                    0,
                    0,
                    state.window.height,
                    0,
                    0,
                    0,
                    0,
                    0,
                    state.window.x as u32,
                    state.window.width,
                    0,
                    0,
                ],
            )?
            .check()?;

        let owner = connection
            .get_selection_owner(self._net_system_tray_s)?
            .reply()?
            .owner;

        if owner == x11rb::NONE {
            connection
                .set_selection_owner(self.selection_owner, self._net_system_tray_s, CURRENT_TIME)?
                .check()?;

            let change = ChangeWindowAttributesAux::new().event_mask(EventMask::STRUCTURE_NOTIFY);

            connection
                .change_window_attributes(self.selection_owner, &change)?
                .check()?;

            // notify clients of new selection owner
            let event = ClientMessageEvent::new(
                32,
                screen.root,
                state.window.atoms.MANAGER,
                [
                    CURRENT_TIME,
                    self._net_system_tray_s,
                    self.selection_owner,
                    0,
                    0,
                ],
            );

            connection
                .send_event(false, screen.root, EventMask::from(0xFFFFFFu32), event)?
                .check()?;

            connection.flush()?;
        } else {
            eprintln!("selections already owned by: {}", owner);
        }

        Ok(())
    }

    fn on_event(
        &mut self,
        connection: &XCBConnection,
        _screen_num: usize,
        state: &mut mdry::State,
        event: x11rb::protocol::Event,
        redraw_sender: Sender<()>,
    ) -> Result<(), crate::Error> {
        match event {
            Event::ClientMessage(event) => {
                if event.type_ == state.window.atoms._NET_SYSTEM_TRAY_OPCODE {
                    let message_data = event.data.as_data32();
                    self.embed_client(connection, message_data, &state)?;
                    return Ok(());
                }

                if event.type_ == self._net_system_tray_s {
                    println!("systray event");
                }
            }
            Event::Expose(event) => {
                if event.window == self.selection_owner {
                    println!("{event:#?}");
                }
            }
            Event::PropertyNotify(event) => {
                if let Some(tray_icon) = self
                    .tray_icons
                    .iter_mut()
                    .find(|ti| ti.embedded_window == event.window)
                {
                    let xembed_info = connection
                        .get_property(
                            false,
                            tray_icon.embedded_window,
                            state.window.atoms._XEMBED_INFO,
                            state.window.atoms._XEMBED_INFO,
                            0,
                            2,
                        )?
                        .reply()?;

                    let xembed_info = xembed_info
                        .value32()
                        .ok_or("Failed to get XEMBED_INFO")?
                        .collect::<Vec<_>>();
                    let mapped = xembed_info[1];

                    if mapped == XEMBED_MAPPED {
                        tray_icon.should_be_mapped = true;
                        tray_icon.has_been_mapped = false;
                        redraw_sender.send(())?;
                    } else {
                        tray_icon.should_be_unmapped = true;
                        redraw_sender.send(())?;
                    }
                }
            }
            Event::UnmapNotify(event) => {
                self.tray_icons.retain(|ti| {
                    if ti.embedded_window == event.window {
                        let _ = connection.destroy_window(ti.wrapper_window);
                        return false;
                    }

                    true
                });
            }
            Event::DestroyNotify(event) => {
                self.tray_icons.retain(|ti| {
                    if ti.embedded_window == event.window {
                        let _ = connection.destroy_window(ti.wrapper_window);
                        return false;
                    }

                    true
                });
            }
            _ => {}
        }

        Ok(())
    }

    fn draw(
        &mut self,
        connection: &XCBConnection,
        _screen_num: usize,
        _state: &mut mdry::State,
        offset: f32,
    ) -> Result<(), crate::Error> {
        for (i, ti) in self.tray_icons.iter_mut().enumerate() {
            let x = (offset + ((self.icons_size + self.padding) * i as u32) as f32) as i32;
            let configure = ConfigureWindowAux::new().x(x);
            connection.configure_window(ti.wrapper_window, &configure)?;
            if ti.should_be_mapped && !ti.has_been_mapped {
                connection.map_window(ti.wrapper_window)?;
                connection.map_window(ti.embedded_window)?;
                ti.has_been_mapped = true;
            } else if ti.should_be_unmapped {
                connection.unmap_window(ti.embedded_window)?;
                connection.unmap_window(ti.wrapper_window)?;
                ti.has_been_mapped = false;
                ti.should_be_mapped = false;
            }
        }

        Ok(())
    }

    fn size(&mut self, _state: &mut State) -> f32 {
        ((self.icons_size + self.padding) * self.tray_icons.len() as u32) as f32
    }

    fn alignment(&self) -> super::Alignment {
        super::Alignment::Right
    }
}
