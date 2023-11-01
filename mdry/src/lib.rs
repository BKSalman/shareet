use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};

use ::x11rb::protocol::Event;
use glyphon::{Attrs, FontSystem, Metrics, Shaping, SwashCache, TextArea, TextAtlas};
use renderer::{
    measure_text, CachedText, Font, ManagedText, Renderer, TextCacheKey, TextRenderer, TextTypes,
};
use shapes::{Mesh, Shape};
use wgpu::MultisampleState;
use window::Window;

use crate::renderer::TextInner;

pub mod x11rb {
    pub use x11rb::protocol::Event;
    pub use x11rb::*;
}

pub mod color;
pub mod renderer;
pub mod shapes;
pub mod window;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexColored {
    position: [f32; 3],
    color: [f32; 3],
}

impl VertexColored {
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
    text_renderer: TextRenderer,
    clear_background: Option<crate::color::Color>,
    texts: Vec<TextTypes>,
    meshes: Vec<Mesh>,
    /// kind of a stupid way to measure the text size
    measure_text_buffer: glyphon::Buffer,
    text_cache: HashMap<TextCacheKey, glyphon::Buffer>,
    default_font: Font,
}

impl<'a> State<'a> {
    // Creating some of the wgpu types requires async code
    pub async fn new(window: Window<'a>) -> State<'a> {
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
        let surface_format = preferred_framebuffer_format(&surface_caps.formats).unwrap();
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

        let mut font_system = FontSystem::new();
        let text_cache = SwashCache::new();
        let mut atlas = TextAtlas::new(&device, &queue, surface_format);
        let text_renderer =
            glyphon::TextRenderer::new(&mut atlas, &device, MultisampleState::default(), None);

        let mut measure_text_buffer = glyphon::Buffer::new(&mut font_system, Metrics::new(1., 1.));

        measure_text_buffer.set_size(&mut font_system, width as f32, height as f32);

        let text_renderer = TextRenderer {
            renderer: text_renderer,
            cache: text_cache,
            font_system,
            atlas,
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
            text_renderer,
            clear_background: None,
            texts: Vec::new(),
            meshes: Vec::new(),
            measure_text_buffer,
            text_cache: HashMap::new(),
            default_font: Font::DEFAULT,
        }
    }

    pub fn create_meshes(shapes: Vec<Shape>) -> Vec<Mesh> {
        shapes
            .iter()
            .map(|shape| match shape {
                Shape::Rect(rect) => {
                    let color = rect.color.rgb_f32();
                    Mesh {
                        indices: vec![0, 1, 2, 0, 2, 3],
                        vertices: vec![
                            VertexColored {
                                position: [rect.x, rect.y, 0.],
                                color,
                            },
                            VertexColored {
                                position: [rect.x, rect.y + rect.height as f32, 0.],
                                color,
                            },
                            VertexColored {
                                position: [
                                    rect.x + rect.width as f32,
                                    rect.y + rect.height as f32,
                                    0.,
                                ],
                                color,
                            },
                            VertexColored {
                                position: [rect.x + rect.width as f32, rect.y, 0.],
                                color,
                            },
                        ],
                    }
                }
                Shape::Triangle(triangle) => {
                    let color = triangle.color.rgb_f32();
                    Mesh {
                        indices: vec![0, 1, 2],
                        vertices: vec![
                            VertexColored {
                                position: [triangle.a.0, triangle.a.1, 0.],
                                color,
                            },
                            VertexColored {
                                position: [triangle.b.0, triangle.b.1, 0.],
                                color,
                            },
                            VertexColored {
                                position: [triangle.c.0, triangle.c.1, 0.],
                                color,
                            },
                        ],
                    }
                }
                Shape::Circle(circle) => {
                    let color = circle.color.rgb_f32();
                    let (vertices, indices) =
                        create_circle_vertices(circle.radius, 30, color, circle.x, circle.y);
                    Mesh { indices, vertices }
                }
            })
            .collect()
    }

    pub fn create_mesh(shape: Shape) -> Mesh {
        match shape {
            Shape::Rect(rect) => {
                let color = rect.color.rgb_f32();
                Mesh {
                    indices: vec![0, 1, 2, 0, 2, 3],
                    vertices: vec![
                        VertexColored {
                            position: [rect.x, rect.y, 0.],
                            color,
                        },
                        VertexColored {
                            position: [rect.x, rect.y + rect.height as f32, 0.],
                            color,
                        },
                        VertexColored {
                            position: [rect.x + rect.width as f32, rect.y + rect.height as f32, 0.],
                            color,
                        },
                        VertexColored {
                            position: [rect.x + rect.width as f32, rect.y, 0.],
                            color,
                        },
                    ],
                }
            }
            Shape::Triangle(triangle) => {
                let color = triangle.color.rgb_f32();
                Mesh {
                    indices: vec![0, 1, 2],
                    vertices: vec![
                        VertexColored {
                            position: [triangle.a.0, triangle.a.1, 0.],
                            color,
                        },
                        VertexColored {
                            position: [triangle.b.0, triangle.b.1, 0.],
                            color,
                        },
                        VertexColored {
                            position: [triangle.c.0, triangle.c.1, 0.],
                            color,
                        },
                    ],
                }
            }
            Shape::Circle(circle) => {
                let color = circle.color.rgb_f32();
                let (vertices, indices) =
                    create_circle_vertices(circle.radius, 30, color, circle.x, circle.y);
                Mesh { indices, vertices }
            }
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

    pub fn update(&mut self) -> Result<(), wgpu::SurfaceError> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Update Render Encoder"),
            });

        #[derive(Debug)]
        enum Allocation {
            Managed(Option<Arc<TextInner>>),
            Cached(TextCacheKey),
        }

        let texts = std::mem::take(&mut self.texts);
        let allocations: Vec<Allocation> = texts
            .iter()
            .map(|t| match t {
                TextTypes::Managed { text } => {
                    let text = text.upgrade();
                    Allocation::Managed(text)
                }
                TextTypes::Cached(text) => {
                    let key = TextCacheKey {
                        content: text.content.clone(),
                        font_size: text.font_size.to_bits(),
                        line_height: text.line_height.to_bits(),
                        font: text.font,
                        bounds: text.bounds,
                        shaping: text.shaping,
                    };
                    if let Some(_) = self.text_cache.get(&key) {
                        Allocation::Cached(key)
                    } else {
                        let mut buffer = glyphon::Buffer::new(
                            &mut self.text_renderer.font_system,
                            Metrics::new(text.font_size, text.line_height),
                        );

                        buffer.set_size(
                            &mut self.text_renderer.font_system,
                            self.width as f32,
                            self.height as f32,
                        );

                        buffer.set_text(
                            &mut self.text_renderer.font_system,
                            &text.content,
                            Attrs::new().color(text.color.into()),
                            text.shaping,
                        );

                        self.text_cache.insert(key.clone(), buffer);
                        Allocation::Cached(key)
                    }
                }
            })
            .collect();

        let texts = texts
            .iter()
            .zip(allocations.iter())
            .filter_map(|(text, allocation)| match text {
                TextTypes::Managed { .. } => {
                    let Allocation::Managed(Some(text)) = allocation else {
                        return None;
                    };

                    Some(TextArea {
                        buffer: &text.buffer,
                        left: text.x,
                        top: text.y,
                        scale: self.window.display_scale,
                        bounds: text.bounds,
                        default_color: text.color.into(),
                    })
                }
                TextTypes::Cached(text) => {
                    let Allocation::Cached(key) = allocation else {
                            return None;
                        };
                    let buffer = self.text_cache.get(key).expect("Get cached buffer");

                    Some(TextArea {
                        buffer,
                        left: text.x,
                        top: text.y,
                        scale: self.window.display_scale,
                        bounds: text.bounds,
                        default_color: text.color.into(),
                    })
                }
            })
            .collect();

        self.text_renderer
            .prepare(&self.device, &self.queue, self.width, self.height, texts)?;

        let meshes = std::mem::take(&mut self.meshes);

        self.renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            meshes,
            self.width,
            self.height,
        );

        Ok(())
    }

    pub fn clear_background(&mut self, color: crate::color::Color) {
        self.clear_background = Some(color);
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

        if let Some(color) = self.clear_background.take() {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Background Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(color.into()),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Mesh Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            self.renderer.render(&mut render_pass);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Text Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            self.text_renderer.render(&mut render_pass).unwrap();
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        self.text_renderer.trim();

        Ok(())
    }

    /// draws a shape in an absolute position
    pub fn draw_shape_absolute(&mut self, shape: Shape) {
        match shape {
            Shape::Rect(rect) => {
                let color = rect.color.rgb_f32();
                self.meshes.push(Mesh {
                    indices: vec![0, 1, 2, 0, 2, 3],
                    vertices: vec![
                        VertexColored {
                            position: [rect.x as f32, rect.y as f32, 0.],
                            color,
                        },
                        VertexColored {
                            position: [rect.x as f32, rect.y as f32 + rect.height as f32, 0.],
                            color,
                        },
                        VertexColored {
                            position: [
                                rect.x as f32 + rect.width as f32,
                                rect.y as f32 + rect.height as f32,
                                0.,
                            ],
                            color,
                        },
                        VertexColored {
                            position: [rect.x as f32 + rect.width as f32, rect.y as f32, 0.],
                            color,
                        },
                    ],
                });
            }
            Shape::Triangle(triangle) => {
                let color = triangle.color.rgb_f32();
                self.meshes.push(Mesh {
                    indices: vec![0, 1, 2],
                    vertices: vec![
                        VertexColored {
                            position: [triangle.a.0 as f32, triangle.a.1 as f32, 0.],
                            color,
                        },
                        VertexColored {
                            position: [triangle.b.0 as f32, triangle.b.1 as f32, 0.],
                            color,
                        },
                        VertexColored {
                            position: [triangle.c.0 as f32, triangle.c.1 as f32, 0.],
                            color,
                        },
                    ],
                });
            }
            Shape::Circle(circle) => {
                let color = circle.color.rgb_f32();
                let (vertices, indices) =
                    create_circle_vertices(circle.radius, 30, color, circle.x, circle.y);
                self.meshes.push(Mesh { indices, vertices });
            }
        }
    }

    pub fn draw_text_absolute(&mut self, text: Arc<TextInner>) {
        self.texts.push(TextTypes::Managed {
            text: ManagedText {
                raw: Arc::downgrade(&text),
            },
        });
    }

    /// draw a text with a cached text buffer
    /// `[cache_text_buffer]` must be called to cache the text buffer
    /// this method will return `Err` if the buffer is not cached already
    ///
    /// this is useful when the text doesn't change
    /// so the buffer could be reused instead of recreating the buffer every draw
    pub fn draw_text_absolute_cached(
        &mut self,
        content: &str,
        x: f32,
        y: f32,
        color: crate::color::Color,
        font_size: f32,
    ) {
        self.texts.push(TextTypes::Cached(CachedText {
            x,
            y,
            content: content.to_string(),
            bounds: glyphon::TextBounds {
                left: x as i32,
                top: y as i32,
                right: self.width as i32,
                bottom: self.height as i32,
            },
            color,
            font_size,
            line_height: font_size,
            font: self.default_font,
            shaping: Shaping::Advanced,
        }));
    }

    pub fn measure_text(&mut self, text: &str, metrics: Metrics) -> (f32, f32) {
        self.measure_text_buffer
            .set_metrics(&mut self.text_renderer.font_system, metrics);

        self.measure_text_buffer.set_text(
            &mut self.text_renderer.font_system,
            text,
            Attrs::new().family(glyphon::Family::Monospace),
            Shaping::Advanced,
        );

        measure_text(&self.measure_text_buffer)
    }

    pub fn font_system_mut(&mut self) -> &mut FontSystem {
        &mut self.text_renderer.font_system
    }
}

fn create_circle_vertices(
    radius: f32,
    num_segments: u32,
    color: [f32; 3],
    x: f32,
    y: f32,
) -> (Vec<VertexColored>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Add the center vertex
    vertices.push(VertexColored {
        position: [x, y, 0.0],
        color,
    });

    let angle_increment = 2.0 * std::f32::consts::PI / num_segments as f32;

    for i in 0..num_segments {
        let angle = i as f32 * angle_increment;
        let angle_x = radius * angle.cos();
        let angle_y = radius * angle.sin();
        vertices.push(VertexColored {
            position: [angle_x + x, angle_y + y, 0.],
            color,
        });
        indices.push(0); // Index of the center vertex
        indices.push(i + 1); // Index of the outer vertex
        indices.push((i + 1) % num_segments + 1); // Index of the next outer vertex
    }

    (vertices, indices)
}

// stolen from egui
/// Find the framebuffer format that mdry prefers
///
/// # Errors
/// Returns [`WgpuError::NoSurfaceFormatsAvailable`] if the given list of formats is empty.
pub fn preferred_framebuffer_format(
    formats: &[wgpu::TextureFormat],
) -> Result<wgpu::TextureFormat, WgpuError> {
    for &format in formats {
        if matches!(
            format,
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
        ) {
            return Ok(format);
        }
    }

    formats
        .get(0)
        .copied()
        .ok_or(WgpuError::NoSurfaceFormatsAvailable)
}

#[derive(thiserror::Error, Debug)]
pub enum WgpuError {
    #[error("Failed to create wgpu adapter, no suitable adapter found.")]
    NoSuitableAdapterFound,

    #[error("There was no valid format for the surface at all.")]
    NoSurfaceFormatsAvailable,

    #[error(transparent)]
    RequestDeviceError(#[from] wgpu::RequestDeviceError),

    #[error(transparent)]
    CreateSurfaceError(#[from] wgpu::CreateSurfaceError),
}
