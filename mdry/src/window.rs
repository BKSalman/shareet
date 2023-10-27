use x11rb::{connection::Connection, protocol::xproto, xcb_ffi::XCBConnection};

unsafe impl<'a> raw_window_handle::HasRawWindowHandle for Window<'a> {
    fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle {
        let mut window_handle = raw_window_handle::XcbWindowHandle::empty();
        window_handle.window = self.xid;
        raw_window_handle::RawWindowHandle::Xcb(window_handle)
    }
}

unsafe impl<'a> raw_window_handle::HasRawDisplayHandle for Window<'a> {
    fn raw_display_handle(&self) -> raw_window_handle::RawDisplayHandle {
        let screen = &self.connection.setup().roots[self.screen_num];
        let mut display_handle = raw_window_handle::XcbDisplayHandle::empty();
        display_handle.connection = self.connection.get_raw_xcb_connection();
        display_handle.screen = screen.root as i32;
        raw_window_handle::RawDisplayHandle::Xcb(display_handle)
    }
}

#[derive(Debug)]
pub struct Window<'a> {
    pub xid: xproto::Window,
    pub connection: &'a XCBConnection,
    pub screen_num: usize,
    pub width: u32,
    pub height: u32,
    pub atoms: Atoms,
    pub display_scale: f32,
}

x11rb::atom_manager! {
    pub Atoms : AtomsCookie {
        _NET_WM_STATE,
        _NET_WM_STATE_MODAL,
        _NET_WM_STATE_STICKY,
        _NET_WM_STATE_MAXIMIZED_VERT,
        _NET_WM_STATE_MAXIMIZED_HORZ,
        _NET_WM_STATE_SHADED,
        _NET_WM_STATE_SKIP_TASKBAR,
        _NET_WM_STATE_SKIP_PAGER,
        _NET_WM_STATE_HIDDEN,
        _NET_WM_STATE_FULLSCREEN,
        _NET_WM_STATE_ABOVE,
        _NET_WM_STATE_BELOW,
        _NET_WM_STATE_DEMANDS_ATTENTION,

        _NET_WM_WINDOW_TYPE,
        _NET_WM_WINDOW_TYPE_DESKTOP,
        _NET_WM_WINDOW_TYPE_DOCK,
        _NET_WM_WINDOW_TYPE_TOOLBAR,
        _NET_WM_WINDOW_TYPE_MENU,
        _NET_WM_WINDOW_TYPE_UTILITY,
        _NET_WM_WINDOW_TYPE_SPLASH,
        _NET_WM_WINDOW_TYPE_DIALOG,
        _NET_WM_WINDOW_TYPE_NORMAL,

        _NET_CLIENT_LIST,
        _NET_DESKTOP_VIEWPORT,
        _NET_DESKTOP_GEOMETRY,
        _NET_NUMBER_OF_DESKTOPS,
        _NET_CURRENT_DESKTOP,
        _NET_DESKTOP_NAMES,
        _NET_WORKAREA,
        _NET_WM_DESKTOP,
        _NET_WM_STRUT,
        _NET_FRAME_EXTENTS,
        _NET_WM_STRUT_PARTIAL,

        _NET_WM_NAME,
        WM_NAME,

        WM_PROTOCOLS,
        _NET_WM_PING,
        WM_DELETE_WINDOW,
    }
}
