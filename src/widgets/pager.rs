use glyphon::FontSystem;
use x11rb::{
    connection::Connection,
    protocol::{
        xproto::{AtomEnum, ConnectionExt},
        Event,
    },
    xcb_ffi::XCBConnection,
};

use crate::{
    shapes::{Mesh, Rect, Shape},
    text_renderer::Text,
    State,
};

use super::Widget;

pub struct Pager {
    pub text_metrics: glyphon::Metrics,
    pub text_color: glyphon::Color,
    pub selector_mesh: Option<Mesh>,
    pub desktops: Vec<(i32, String)>,
    pub atoms: PagerAtoms,
    pub requires_redraw: bool,
    pub texts: Vec<Text>,
}

impl Widget for Pager {
    fn setup(
        &mut self,
        state: &State,
        connection: &XCBConnection,
        screen_num: usize,
    ) -> Result<(), crate::Error> {
        let screen = &connection.setup().roots[screen_num];
        let padding = 5;

        let desktops = connection
            .get_property(
                false,
                screen.root,
                self.atoms._NET_DESKTOP_NAMES,
                AtomEnum::ANY,
                0,
                u32::MAX,
            )?
            .reply()?;
        let desktops = get_desktop_names(desktops.value);

        self.desktops = desktops
            .into_iter()
            .enumerate()
            .map(|(i, t)| {
                let x = 20 * i as i32 + padding;
                (x, t)
            })
            .collect();

        let reply = connection
            .get_property(
                false,
                state.screen.root,
                self.atoms._NET_CURRENT_DESKTOP,
                AtomEnum::CARDINAL,
                0,
                4,
            )?
            .reply()?;

        let value32 = reply.value32();

        if let Some(mut value) = value32 {
            let current_desktop_index = value.next().unwrap() as usize;
            let current_desktop = &self.desktops[current_desktop_index];

            let rect = Rect {
                x: current_desktop.0,
                y: state.height as i32 - 2,
                width: 20,
                height: 2,
            };

            self.selector_mesh = Some(crate::painter::Painter::create_mesh(
                Shape::Rect(rect),
                crate::Color::rgb(0, 0, 0),
            ));
        }

        Ok(())
    }

    fn on_event(
        &mut self,
        connection: &XCBConnection,
        state: &mut State,
        event: Event,
    ) -> Result<(), crate::Error> {
        match event {
            Event::PropertyNotify(event) if event.window == state.screen.root => {
                if event.atom == self.atoms._NET_CURRENT_DESKTOP {
                    let reply = connection
                        .get_property(
                            false,
                            state.screen.root,
                            self.atoms._NET_CURRENT_DESKTOP,
                            AtomEnum::CARDINAL,
                            0,
                            4,
                        )?
                        .reply()?;

                    let value32 = reply.value32();

                    if let Some(mut value) = value32 {
                        let current_desktop_index = value.next().unwrap() as usize;
                        let current_desktop = &self.desktops[current_desktop_index];

                        let rect = Rect {
                            x: current_desktop.0,
                            y: state.height as i32 - 2,
                            width: 20,
                            height: 2,
                        };

                        self.selector_mesh = Some(crate::painter::Painter::create_mesh(
                            Shape::Rect(rect),
                            crate::Color::rgb(0, 0, 0),
                        ));

                        // if let Some(mesh_handle) = self.current_desktop {
                        //     state.painter.remove_mesh(mesh_handle).unwrap();
                        // }

                        // self.current_desktop = Some(
                        //     state
                        //         .painter
                        //         .add_shape_absolute(Shape::Rect(rect), crate::Color::rgb(0, 0, 0)),
                        // );
                    }
                }

                self.requires_redraw = true;
            }
            _ => {}
        }
        Ok(())
    }

    fn meshes(&self) -> Vec<Mesh> {
        if let Some(mesh) = self.selector_mesh.clone() {
            vec![mesh]
        } else {
            vec![]
        }
    }

    fn texts(&self, font_system: &mut FontSystem) -> Vec<Text> {
        self.desktops
            .iter()
            .cloned()
            .map(|(x, t)| {
                let mut buffer = glyphon::Buffer::new(font_system, self.text_metrics);
                buffer.set_size(font_system, 1920., 30.);
                buffer.set_text(
                    font_system,
                    &t,
                    glyphon::Attrs::new(),
                    glyphon::Shaping::Advanced,
                );
                Text {
                    x,
                    y: 0,
                    color: self.text_color,
                    content: t,
                    bounds: glyphon::TextBounds {
                        left: 0,
                        top: 0,
                        right: 1920, // TODO: make this dynamic somehow
                        bottom: 30,  // TODO: make this dynamic somehow
                    },
                    buffer,
                }
            })
            .collect()
    }

    fn size(&self) -> u32 {
        0
    }

    fn requires_redraw(&self) -> bool {
        self.requires_redraw
    }
}

pub fn get_desktop_names(values: Vec<u8>) -> Vec<String> {
    values
        .split(|c| *c == 0)
        .map(|c| String::from_utf8_lossy(c).to_string())
        .collect::<Vec<String>>()
}

x11rb::atom_manager! {
    pub PagerAtoms : AtomsCookie {
        _NET_NUMBER_OF_DESKTOPS,
        _NET_CURRENT_DESKTOP,
        _NET_DESKTOP_NAMES,
        _NET_WM_NAME,
        WM_NAME,
    }
}
