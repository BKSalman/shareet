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

use super::{text::TextWidget, Widget};

pub struct Pager {
    text_metrics: glyphon::Metrics,
    text_color: glyphon::Color,
    selector_mesh: Option<Mesh>,
    desktops: Vec<TextWidget>,
    atoms: PagerAtoms,
    requires_redraw: bool,
    padding: i32,
    size: u32,
}

impl Pager {
    pub fn new(
        connection: &XCBConnection,
        text_metrics: glyphon::Metrics,
        text_color: glyphon::Color,
        padding: i32,
    ) -> Result<Self, crate::Error> {
        Ok(Self {
            text_metrics,
            text_color,
            selector_mesh: None,
            atoms: PagerAtoms::new(connection)?.reply()?,
            requires_redraw: true,
            desktops: Vec::new(),
            padding,
            size: 0,
        })
    }
}

impl Widget for Pager {
    fn setup(
        &mut self,
        state: &mut State,
        connection: &XCBConnection,
        screen_num: usize,
    ) -> Result<(), crate::Error> {
        let screen = &connection.setup().roots[screen_num];

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

        let (offset, text_widgets) =
            desktops
                .iter()
                .cloned()
                .fold((0, Vec::new()), |(offset, mut text_widgets), t| {
                    let text_widget = TextWidget::new(
                        offset as i32 + self.padding,
                        0,
                        &t,
                        self.text_color,
                        &mut state.text_renderer.font_system,
                        self.text_metrics,
                        None,
                    );
                    let add = text_widget.size() + self.padding as u32;

                    text_widgets.push(text_widget);

                    (offset + add, text_widgets)
                });

        self.size = offset;

        self.desktops = text_widgets;
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
                x: current_desktop.x(),
                y: state.height as i32 - 2,
                width: current_desktop.size(),
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
                            x: current_desktop.x(),
                            y: state.height as i32 - 2,
                            width: current_desktop.size(),
                            height: 2,
                        };

                        self.selector_mesh = Some(crate::painter::Painter::create_mesh(
                            Shape::Rect(rect),
                            crate::Color::rgb(0, 0, 0),
                        ));
                    }
                }

                self.requires_redraw = true;
            }
            _ => {}
        }
        Ok(())
    }

    fn meshes(&self) -> Vec<&Mesh> {
        self.selector_mesh
            .as_ref()
            .map(|m| vec![m])
            .unwrap_or(vec![])
    }

    fn texts(&self, _font_system: &mut FontSystem) -> Vec<&Text> {
        self.desktops.iter().fold(Vec::new(), |mut acc, tw| {
            acc.extend(tw.texts(_font_system));
            acc
        })
    }

    fn size(&self) -> u32 {
        // self.desktops.iter().map(|t| t.size()).sum::<u32>() + self.padding as u32
        self.size
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
