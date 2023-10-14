use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle, XcbDisplayHandle,
    XcbWindowHandle,
};
use renderer::Renderer;
use shapes::Shape;
use wgpu::util::DeviceExt;
use wgpu::{Extent3d, ImageCopyTexture, TextureDescriptor};
use x11rb::xcb_ffi::XCBConnection;
use x11rb::{
    connection::Connection,
    protocol::{xproto, Event},
};

mod buffer;
mod primitive;
mod renderer;
mod shapes;

pub trait Vertex: bytemuck::Pod + bytemuck::Zeroable {
    fn desc() -> wgpu::VertexBufferLayout<'static>;
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexColored {
    position: [f32; 3],
    color: [f32; 3],
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

const VERTICES: &[VertexColored] = &[
    VertexColored {
        position: [0.0, 0.5, 0.0],
        color: [1.0, 0.0, 0.0],
    },
    VertexColored {
        position: [-0.5, -0.5, 0.0],
        color: [0.0, 0.0, 1.0],
    },
    VertexColored {
        position: [0.5, -0.5, 0.0],
        color: [0.0, 1.0, 0.0],
    },
    VertexColored {
        position: [0.5, 0.5, 0.0],
        color: [0.5, 0.0, 0.5],
    },
];

const INDICES: &[u16] = &[0, 1, 2];

unsafe impl<'a> HasRawWindowHandle for Window<'a> {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut window_handle = XcbWindowHandle::empty();
        window_handle.window = self.xid.try_into().unwrap();
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

pub struct Window<'a> {
    pub xid: xproto::Window,
    pub connection: &'a XCBConnection,
    pub screen_num: usize,
    pub width: u32,
    pub height: u32,
    pub atoms: Atoms,
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
    window: Window<'a>,
    renderer: Renderer,
}

impl<'a> State<'a> {
    // Creating some of the wgpu types requires async code
    pub async fn new(window: Window<'a>) -> State {
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
            width: width as u32,
            height: height as u32,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let renderer = Renderer::new(config.format, &device).await;

        State {
            surface,
            device,
            queue,
            config,
            width: width as u32,
            height: height as u32,
            window,
            renderer,
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
        }
    }

    pub fn input(&mut self, event: &Event) -> bool {
        false
    }

    pub fn update(&mut self) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        let shapes = Vec::from([Shape {
            indices: vec![0, 1, 2],
            vertices: vec![
                VertexColored {
                    position: [0.0, 0.5, 0.0],
                    color: [1.0, 0.0, 0.0],
                },
                VertexColored {
                    position: [-0.5, -0.5, 0.0],
                    color: [0.0, 0.0, 1.0],
                },
                VertexColored {
                    position: [0.5, -0.5, 0.0],
                    color: [0.0, 1.0, 0.0],
                },
            ],
        }]);

        self.renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &shapes,
            self.width,
            self.height,
        );
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
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

            let shapes = Vec::from([Shape {
                indices: vec![0, 1, 2],
                vertices: vec![
                    VertexColored {
                        position: [0.0, 0.5, 0.0],
                        color: [1.0, 0.0, 0.0],
                    },
                    VertexColored {
                        position: [-0.5, -0.5, 0.0],
                        color: [0.0, 0.0, 1.0],
                    },
                    VertexColored {
                        position: [0.5, -0.5, 0.0],
                        color: [0.0, 1.0, 0.0],
                    },
                ],
            }]);

            self.renderer.render(&mut render_pass, &shapes);
        }

        // submit will accept anything that implements IntoIter
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
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

        WM_PROTOCOLS,
        WM_DELETE_WINDOW,
    }
}
