use glyphon::{Attrs, Family, FontSystem, Metrics, Shaping, SwashCache, TextAtlas};
use painter::Painter;
use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle, XcbDisplayHandle,
    XcbWindowHandle,
};
use renderer::Renderer;
use shapes::Mesh;
use text_renderer::Text;
use wgpu::MultisampleState;
use widgets::Widget;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, PropMode, Screen, WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;
use x11rb::xcb_ffi::XCBConnection;
use x11rb::{
    connection::Connection,
    protocol::{xproto, Event},
};
use x11rb::{COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT};

mod painter;
mod renderer;
pub mod shapes;
pub mod text_renderer;
pub mod widgets;

pub type Error = Box<dyn std::error::Error>;

pub struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Color {
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub fn rgba_f32(&self) -> [f32; 4] {
        [
            self.r as f32 / 255.,
            self.g as f32 / 255.,
            self.b as f32 / 255.,
            self.a as f32 / 255.,
        ]
    }

    pub fn rgb_f32(&self) -> [f32; 3] {
        [
            self.r as f32 / 255.,
            self.g as f32 / 255.,
            self.b as f32 / 255.,
        ]
    }
}

pub trait Vertex: bytemuck::Pod + bytemuck::Zeroable {
    fn desc() -> wgpu::VertexBufferLayout<'static>;
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexColored {
    position: [f32; 3],
    color: [f32; 3],
}

impl VertexColored {
    pub fn add_offset_mut(&mut self, offset: f32) {
        self.position[0] += offset;
    }
    pub fn add_offset(&self, offset: f32) -> [f32; 3] {
        [
            self.position[0] + offset,
            self.position[1],
            self.position[2],
        ]
    }
}

impl Vertex for VertexColored {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<VertexColored>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

unsafe impl<'a> HasRawWindowHandle for Window<'a> {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut window_handle = XcbWindowHandle::empty();
        window_handle.window = self.xid;
        RawWindowHandle::Xcb(window_handle)
    }
}

unsafe impl<'a> HasRawDisplayHandle for Window<'a> {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        let screen = &self.connection.setup().roots[self.screen_num];
        let mut display_handle = XcbDisplayHandle::empty();
        display_handle.connection = self.connection.get_raw_xcb_connection();
        display_handle.screen = screen.root as i32;
        RawDisplayHandle::Xcb(display_handle)
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

pub struct Bar<'a> {
    pub state: State<'a>,
    pub widgets: Vec<Box<dyn Widget>>,
}

impl<'a> Bar<'a> {
    pub async fn new(window: Window<'a>, screen: &'a Screen) -> Bar<'a> {
        let state = State::new(window, screen).await;
        Self {
            state,
            widgets: vec![],
        }
    }
}

pub struct State<'a> {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pub width: u32,
    pub height: u32,
    // The window must be declared after the surface so
    // it gets dropped after it as the surface contains
    // unsafe references to the window's resources.
    pub window: Window<'a>,
    renderer: Renderer,
    pub text_renderer: crate::text_renderer::TextRenderer,
    pub painter: Painter,
    pub current_tag_index: Option<usize>,
    pub screen: &'a Screen,
}

impl<'a> State<'a> {
    // Creating some of the wgpu types requires async code
    pub async fn new(window: Window<'a>, screen: &'a Screen) -> State<'a> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        let width = window.width;
        let height = window.height;

        // # Safety
        //
        // The surface needs to live as long as the window that created it.
        // State owns the window so this should be safe.
        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        // Shader code in this tutorial assumes an sRGB surface texture. Using a different
        // one will result all the colors coming out darker. If you want to support non
        // sRGB surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let renderer = Renderer::new(config.format, &device).await;
        let painter = Painter::new();

        let font_system = FontSystem::new();
        let text_cache = SwashCache::new();
        let mut atlas = TextAtlas::new(&device, &queue, surface_format);
        let text_renderer =
            glyphon::TextRenderer::new(&mut atlas, &device, MultisampleState::default(), None);

        let text_renderer = crate::text_renderer::TextRenderer {
            renderer: text_renderer,
            cache: text_cache,
            font_system,
            atlas,
            texts: Vec::new(),
        };

        State {
            surface,
            device,
            queue,
            config,
            width,
            height,
            window,
            renderer,
            painter,
            text_renderer,
            current_tag_index: None,
            screen,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.width = width;
            self.height = height;
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.text_renderer
                .resize(width as f32, height as f32, self.window.display_scale);
        }
    }

    pub fn input(&mut self, event: &Event) -> bool {
        false
    }

    pub fn update(
        &mut self,
        meshes: Vec<(&Mesh, f32)>,
        texts: Vec<(&Text, f32)>,
    ) -> Result<(), wgpu::SurfaceError> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        self.text_renderer
            .prepare(&self.device, &self.queue, self.width, self.height, texts)?;

        let mut painter_meshes = self.painter.meshes();
        painter_meshes.extend(meshes);

        self.renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            painter_meshes,
            self.width,
            self.height,
        );

        Ok(())
    }

    pub fn render(&mut self, meshes: Vec<(&Mesh, f32)>) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            let mut painter_meshes = self.painter.meshes();
            painter_meshes.extend(meshes);

            self.renderer.render(&mut render_pass, painter_meshes);
            self.text_renderer.render(&mut render_pass).unwrap();
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        self.text_renderer.trim();

        Ok(())
    }

    pub fn add_text_absolute(
        &mut self,
        content: &str,
        x: i32,
        y: i32,
        color: glyphon::Color,
        font_size: f32,
    ) {
        let mut text_buffer = glyphon::Buffer::new(
            &mut self.text_renderer.font_system,
            Metrics::new(font_size, self.height as f32),
        );

        let physical_width = self.width as f32 * self.window.display_scale;
        let physical_height = self.height as f32 * self.window.display_scale;

        text_buffer.set_size(
            &mut self.text_renderer.font_system,
            physical_width,
            physical_height,
        );
        text_buffer.set_text(
            &mut self.text_renderer.font_system,
            content,
            Attrs::new().family(Family::SansSerif),
            Shaping::Advanced,
        );
        text_buffer.shape_until_scroll(&mut self.text_renderer.font_system);

        self.text_renderer.add_text(Text {
            x,
            y,
            color,
            content: content.to_string(),
            bounds: glyphon::TextBounds {
                left: 0,
                top: 0,
                right: self.width as i32,
                bottom: self.height as i32,
            },
            buffer: text_buffer,
        });
    }
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

pub fn create_window(
    connection: &XCBConnection,
    width: u16,
    height: u16,
    screen_num: usize,
    display_scale: f32,
) -> Result<Window, Error> {
    let screen = &connection.setup().roots[screen_num];

    let atoms = Atoms::new(connection)?.reply()?;

    let window_id = connection.generate_id()?;

    let create = CreateWindowAux::new().event_mask(
        EventMask::EXPOSURE
            | EventMask::STRUCTURE_NOTIFY
            | EventMask::VISIBILITY_CHANGE
            | EventMask::KEY_PRESS
            | EventMask::KEY_RELEASE
            | EventMask::KEYMAP_STATE
            | EventMask::BUTTON_PRESS
            | EventMask::BUTTON_RELEASE
            | EventMask::POINTER_MOTION
            | EventMask::PROPERTY_CHANGE,
    );

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

    connection
        .change_property8(
            PropMode::REPLACE,
            window_id,
            atoms.WM_NAME,
            AtomEnum::STRING,
            b"lmao",
        )?
        .check()?;

    connection
        .change_property8(
            PropMode::REPLACE,
            window_id,
            x11rb::protocol::xproto::Atom::from(x11rb::protocol::xproto::AtomEnum::WM_CLASS),
            AtomEnum::STRING,
            b"lmao",
        )?
        .check()?;

    connection
        .change_property32(
            PropMode::REPLACE,
            window_id,
            atoms._NET_WM_WINDOW_TYPE,
            AtomEnum::ATOM,
            &[atoms._NET_WM_WINDOW_TYPE_DOCK],
        )?
        .check()?;

    // connection
    //     .change_property32(
    //         PropMode::REPLACE,
    //         window_id,
    //         atoms.WM_PROTOCOLS,
    //         AtomEnum::ATOM,
    //         &[atoms.WM_DELETE_WINDOW, atoms._NET_WM_PING],
    //     )?
    //     .check()?;

    connection
        .change_property32(
            PropMode::REPLACE,
            window_id,
            atoms._NET_WM_STATE,
            AtomEnum::ATOM,
            &[atoms._NET_WM_STATE_STICKY, atoms._NET_WM_STATE_ABOVE],
        )?
        .check()?;

    connection
        .change_property32(
            PropMode::REPLACE,
            window_id,
            atoms._NET_WM_STRUT_PARTIAL,
            AtomEnum::CARDINAL,
            // left, right, top, bottom, left_start_y, left_end_y,
            // right_start_y, right_end_y, top_start_x, top_end_x, bottom_start_x,
            // bottom_end_x
            &[
                0,
                0,
                0,
                height as u32,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                screen.width_in_pixels as u32,
            ],
        )?
        .check()?;

    connection.map_window(window_id)?;

    connection.flush()?;

    Ok(Window {
        xid: window_id,
        connection,
        screen_num,
        width: width as u32,
        height: height as u32,
        atoms,
        display_scale,
    })
}
