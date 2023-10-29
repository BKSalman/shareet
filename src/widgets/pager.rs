use x11rb::{
    connection::Connection,
    protocol::{
        xproto::{AtomEnum, ConnectionExt},
        Event,
    },
    xcb_ffi::XCBConnection,
};

use crate::State;
use mdry::{color::Color, shapes::Rect};

use super::{text::TextWidget, Widget};

pub struct Pager {
    text_metrics: glyphon::Metrics,
    text_color: Color,
    current_desktop: Option<usize>,
    desktops: Vec<TextWidget>,
    atoms: PagerAtoms,
    requires_redraw: bool,
    padding: f32,
    width: f32,
    selector_color: Color,
}

impl Pager {
    pub fn new(
        connection: &XCBConnection,
        text_metrics: glyphon::Metrics,
        text_color: Color,
        selector_color: Color,
        padding: f32,
    ) -> Result<Self, crate::Error> {
        Ok(Self {
            text_metrics,
            text_color,
            atoms: PagerAtoms::new(connection)?.reply()?,
            requires_redraw: true,
            desktops: Vec::new(),
            padding,
            width: 0.,
            current_desktop: None,
            selector_color,
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
                .fold((0., Vec::new()), |(offset, mut text_widgets), t| {
                    let mut text_widget = TextWidget::new(
                        offset + self.padding,
                        0.,
                        t,
                        self.text_color,
                        self.text_metrics.font_size,
                        None,
                    );

                    text_widget.setup(state, connection, screen_num).unwrap();

                    let offset = offset + text_widget.size(state) + self.padding;

                    text_widgets.push(text_widget);

                    (offset, text_widgets)
                });

        self.width = offset;

        self.desktops = text_widgets;
        let reply = connection
            .get_property(
                false,
                screen.root,
                self.atoms._NET_CURRENT_DESKTOP,
                AtomEnum::CARDINAL,
                0,
                4,
            )?
            .reply()?;

        let value32 = reply.value32();

        if let Some(mut value) = value32 {
            let current_desktop_index = value.next().unwrap() as usize;

            self.current_desktop = Some(current_desktop_index);
        }

        Ok(())
    }

    fn on_event(
        &mut self,
        connection: &XCBConnection,
        screen_num: usize,
        _state: &mut State,
        event: Event,
    ) -> Result<(), crate::Error> {
        let screen = &connection.setup().roots[screen_num];
        match event {
            Event::PropertyNotify(event) if event.window == screen.root => {
                if event.atom == self.atoms._NET_CURRENT_DESKTOP {
                    let reply = connection
                        .get_property(
                            false,
                            screen.root,
                            self.atoms._NET_CURRENT_DESKTOP,
                            AtomEnum::CARDINAL,
                            0,
                            4,
                        )?
                        .reply()?;

                    let value32 = reply.value32();

                    if let Some(mut value) = value32 {
                        let current_desktop_index = value.next().unwrap() as usize;

                        if current_desktop_index > self.desktops.len() - 1 {
                            eprintln!(
                                "tried to switch to an out of bound desktop in pager: {current_desktop_index}"
                            );
                            return Ok(());
                        }
                        self.current_desktop = Some(current_desktop_index);
                    }
                }

                self.requires_redraw = true;
            }
            _ => {}
        }
        Ok(())
    }

    fn draw(
        &mut self,
        connection: &XCBConnection,
        screen_num: usize,
        state: &mut State,
        offset: f32,
    ) -> Result<(), crate::Error> {
        for desktop in self.desktops.iter_mut() {
            desktop.draw(connection, screen_num, state, offset)?;
        }

        if let Some(current_desktop_index) = self.current_desktop {
            let current_desktop = &self.desktops[current_desktop_index];

            let rect = Rect {
                x: current_desktop.x() + offset,
                y: state.height as f32 - 2.,
                width: current_desktop.size(state) as u32,
                height: 2,
                color: self.selector_color,
            };

            state.draw_shape_absolute(mdry::shapes::Shape::Rect(rect));
        }

        Ok(())
    }

    fn size(&self, state: &mut State) -> f32 {
        self.desktops
            .iter()
            .map(|t| t.size(state) + self.padding)
            .sum::<f32>()
            + self.padding
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
