use crossbeam::channel::Sender;
use x11rb::{
    connection::Connection,
    protocol::{
        xproto::{
            AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConnectionExt, Cursor,
            EventMask,
        },
        Event,
    },
    xcb_ffi::XCBConnection,
    CURRENT_TIME,
};

use crate::State;
use mdry::{color::Color, shapes::Rect};

use super::{text::TextWidget, Widget};

const HAND_CURSOR: u16 = 60;
const LEFTPTR_CURSOR: u16 = 68;

const LEFT_BTN: u8 = 1;
// const RIGHT_BTN: u8 = 2;
// const MIDDLE_BTN: u8 = 3;
// const SCROLL_UP: u8 = 4;
// const SCROLL_DOWN: u8 = 5;

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
    normal_cursor: Cursor,
    hand_cursor: Cursor,
    hovering: Option<usize>,
}

impl Pager {
    pub fn new(
        connection: &XCBConnection,
        text_metrics: glyphon::Metrics,
        text_color: Color,
        selector_color: Color,
        padding: f32,
    ) -> Result<Self, crate::Error> {
        let font = connection.generate_id()?;
        connection.open_font(font, b"cursor")?;

        let hand_cursor = connection.generate_id()?;
        connection.create_glyph_cursor(
            hand_cursor,
            font,
            font,
            HAND_CURSOR,
            HAND_CURSOR + 1,
            0,
            0,
            0,
            u16::MAX,
            u16::MAX,
            u16::MAX,
        )?;

        let normal_cursor = connection.generate_id()?;
        connection.create_glyph_cursor(
            normal_cursor,
            font,
            font,
            LEFTPTR_CURSOR,
            LEFTPTR_CURSOR + 1,
            0,
            0,
            0,
            u16::MAX,
            u16::MAX,
            u16::MAX,
        )?;

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
            hand_cursor,
            normal_cursor,
            hovering: None,
        })
    }
}

impl Widget for Pager {
    fn setup(
        &mut self,
        state: &mut State,
        connection: &XCBConnection,
        screen_num: usize,
        redraw_sender: Sender<()>,
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
                    let (width, height) = state.measure_text(t, self.text_metrics);
                    let mut text_widget = TextWidget::new(
                        offset + self.padding,
                        0.,
                        t,
                        self.text_color,
                        self.text_metrics.font_size,
                        None,
                        width,
                        height,
                    );

                    text_widget
                        .setup(state, connection, screen_num, redraw_sender.clone())
                        .unwrap();

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
        state: &mut State,
        event: Event,
        _redraw_sender: Sender<()>,
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
            Event::MotionNotify(event) => {
                let event_x = event.event_x as f32;
                let hover = self
                    .desktops
                    .iter_mut()
                    .enumerate()
                    .map(|(i, tw)| (i, tw.x(), tw.size(state)))
                    .find(|(_, x, width)| hover(event_x, *x, *width, self.padding));

                if let Some((i, _, _)) = hover {
                    self.hovering = Some(i);
                    let change = ChangeWindowAttributesAux::new().cursor(self.hand_cursor);

                    connection
                        .change_window_attributes(state.window.xid, &change)?
                        .check()?;
                } else {
                    self.hovering = None;
                    let change = ChangeWindowAttributesAux::new().cursor(self.normal_cursor);

                    connection
                        .change_window_attributes(state.window.xid, &change)?
                        .check()?;
                }
            }
            Event::ButtonPress(event) => {
                if event.detail == LEFT_BTN {
                    if let Some(hovering) = self.hovering {
                        let message = ClientMessageEvent::new(
                            32,
                            screen.root,
                            state.window.atoms._NET_CURRENT_DESKTOP,
                            [hovering as u32, CURRENT_TIME, 0, 0, 0],
                        );

                        connection
                            .send_event(false, screen.root, EventMask::from(0xFFFFFFu32), message)?
                            .check()?;
                    }
                }
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
            let current_desktop = &mut self.desktops[current_desktop_index];

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

    fn size(&mut self, state: &mut State) -> f32 {
        self.desktops
            .iter_mut()
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

fn hover(event_x: f32, x: f32, width: f32, padding: f32) -> bool {
    event_x >= x - padding && event_x <= x + width + padding
}
