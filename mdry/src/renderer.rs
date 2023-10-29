use glyphon::{Resolution, SwashCache, TextArea, TextBounds};
use wgpu::util::DeviceExt;

use crate::shapes::Mesh;
use crate::VertexColored;
use std::num::NonZeroU64;
use std::ops::Range;

const SCALE_FACTOR: Option<&str> = option_env!("SCALE_FACTOR");

#[derive(Debug)]
struct SlicedBuffer {
    buffer: wgpu::Buffer,
    slices: Vec<Range<usize>>,
    capacity: wgpu::BufferAddress,
}

#[derive(Debug)]
pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    index_buffer: SlicedBuffer,
    vertex_buffer: SlicedBuffer,
    uniform_buffer: wgpu::Buffer,
    scale_factor: f32,
    uniform_bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
}

/// Uniform buffer used when rendering.
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct UniformBuffer {
    screen_size_in_points: [f32; 2],
    // Uniform buffers need to be at least 16 bytes in WebGL.
    // See https://github.com/gfx-rs/wgpu/issues/2072
    _padding: [u32; 2],
}

impl Renderer {
    pub async fn new<'a>(output_color_format: wgpu::TextureFormat, device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[UniformBuffer {
                screen_size_in_points: [0.0, 0.0],
                _padding: Default::default(),
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Uniform Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(std::mem::size_of::<UniformBuffer>() as _),
                        ty: wgpu::BufferBindingType::Uniform,
                    },
                    count: None,
                }],
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&uniform_bind_group_layout],
                push_constant_ranges: &[],
            });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[VertexColored::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_color_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        const VERTEX_BUFFER_START_CAPACITY: wgpu::BufferAddress =
            (std::mem::size_of::<VertexColored>() * 1024) as _;
        const INDEX_BUFFER_START_CAPACITY: wgpu::BufferAddress =
            (std::mem::size_of::<u32>() * 1024 * 3) as _;

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Index Buffer"),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            size: INDEX_BUFFER_START_CAPACITY,
            mapped_at_creation: false,
        });

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vertex Buffer"),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            size: VERTEX_BUFFER_START_CAPACITY,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            index_buffer: SlicedBuffer {
                buffer: index_buffer,
                slices: Vec::with_capacity(64),
                capacity: INDEX_BUFFER_START_CAPACITY,
            },
            vertex_buffer: SlicedBuffer {
                buffer: vertex_buffer,
                slices: Vec::with_capacity(64),
                capacity: VERTEX_BUFFER_START_CAPACITY,
            },
            scale_factor: SCALE_FACTOR
                .map(|s| s.parse::<f32>().unwrap_or(1.0))
                .unwrap_or(1.0),
            uniform_buffer,
            uniform_bind_group,
            texture_bind_group_layout,
        }
    }

    // pub fn resize(&mut self, width: u32, height: u32) {
    //     if width > 0 && height > 0 {
    //         self.width = width;
    //         self.height = height;
    //         self.config.width = width;
    //         self.config.height = height;
    //         self.surface.configure(&self.device, &self.config);
    //         self.text_renderer
    //             .resize(width as f32, height as f32, self.window.display_scale);
    //     }
    // }

    /// Render/draw the provided meshes
    pub fn render<'rp>(&'rp self, render_pass: &mut wgpu::RenderPass<'rp>) {
        let index_buffer_slices = self.index_buffer.slices.iter();
        let vertex_buffer_slices = self.vertex_buffer.slices.iter();
        for (index_buffer_slice, vertex_buffer_slice) in
            index_buffer_slices.zip(vertex_buffer_slices)
        {
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            render_pass.set_index_buffer(
                self.index_buffer
                    .buffer
                    .slice(index_buffer_slice.start as u64..index_buffer_slice.end as u64),
                wgpu::IndexFormat::Uint32,
            );
            render_pass.set_vertex_buffer(
                0,
                self.vertex_buffer
                    .buffer
                    .slice(vertex_buffer_slice.start as u64..vertex_buffer_slice.end as u64),
            );

            let len = (index_buffer_slice.len() / std::mem::size_of::<u32>()) - 1;

            render_pass.draw_indexed(0..len as u32 + 1, 0, 0..1);
        }
    }

    // pub fn update_textures(&mut self, queue: &wgpu::Queue, window_width: u32, window_height: u32) {}

    pub fn update_buffers(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        meshes: Vec<Mesh>,
        window_width: u32,
        window_height: u32,
    ) {
        let (vertex_count, index_count) = {
            meshes.iter().fold((0, 0), |acc, mesh| {
                (acc.0 + mesh.vertices.len(), acc.1 + mesh.indices.len())
            })
        };

        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[UniformBuffer {
                screen_size_in_points: [
                    window_width as f32 / self.scale_factor,
                    window_height as f32 / self.scale_factor,
                ],
                _padding: Default::default(),
            }]),
        );

        if index_count > 0 {
            self.index_buffer.slices.clear();
            let required_index_buffer_size = (std::mem::size_of::<u32>() * index_count) as u64;

            if self.index_buffer.capacity < required_index_buffer_size {
                // Resize index buffer if needed.
                self.index_buffer.capacity =
                    (self.index_buffer.capacity * 2).max(required_index_buffer_size);
                self.index_buffer.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Index Buffer"),
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    size: self.index_buffer.capacity,
                    mapped_at_creation: false,
                });
            }

            let mut index_buffer_staging = queue
                .write_buffer_with(
                    &self.index_buffer.buffer,
                    0,
                    NonZeroU64::new(required_index_buffer_size).unwrap(),
                )
                .expect("Failed to create staging buffer for index data");
            let mut index_offset = 0;
            for mesh in &meshes {
                let size = mesh.indices.len() * std::mem::size_of::<u32>();
                let slice = index_offset..(size + index_offset);
                index_buffer_staging[slice.clone()]
                    .copy_from_slice(bytemuck::cast_slice(&mesh.indices));
                self.index_buffer.slices.push(slice);
                index_offset += size;
            }
        }

        if vertex_count > 0 {
            self.vertex_buffer.slices.clear();
            let required_vertex_buffer_size =
                (std::mem::size_of::<VertexColored>() * vertex_count) as u64;
            if self.vertex_buffer.capacity < required_vertex_buffer_size {
                // Resize vertex buffer if needed.
                self.vertex_buffer.capacity =
                    (self.vertex_buffer.capacity * 2).max(required_vertex_buffer_size);
                self.vertex_buffer.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Vertex Buffer"),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    size: self.vertex_buffer.capacity,
                    mapped_at_creation: false,
                });
            }

            let mut vertex_buffer_staging = queue
                .write_buffer_with(
                    &self.vertex_buffer.buffer,
                    0,
                    NonZeroU64::new(required_vertex_buffer_size).unwrap(),
                )
                .expect("Failed to create staging buffer for vertex data");
            let mut vertex_offset = 0;
            for mesh in meshes {
                let size = mesh.vertices.len() * std::mem::size_of::<VertexColored>();
                let slice = vertex_offset..(size + vertex_offset);
                vertex_buffer_staging[slice.clone()]
                    .copy_from_slice(bytemuck::cast_slice(&mesh.vertices));
                self.vertex_buffer.slices.push(slice);
                vertex_offset += size;
            }
        }
    }
}

pub struct TextRenderer {
    pub(crate) renderer: glyphon::TextRenderer,
    pub(crate) cache: SwashCache,
    pub(crate) font_system: glyphon::FontSystem,
    pub(crate) atlas: glyphon::TextAtlas,
}

#[derive(Debug)]
pub struct Text {
    pub x: f32,
    pub y: f32,
    pub color: glyphon::Color,
    pub content: String,
    pub bounds: TextBounds,
    pub buffer: glyphon::Buffer,
}

impl Text {
    pub fn add_offset(&mut self, offset: f32) {
        self.x += offset;
        self.bounds.left += offset as i32;
        self.bounds.right += offset as i32;
    }
}

impl TextRenderer {
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        texts: Vec<TextArea>,
    ) -> Result<(), wgpu::SurfaceError> {
        self.renderer
            .prepare(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                Resolution { width, height },
                texts,
                &mut self.cache,
            )
            .unwrap();
        Ok(())
    }

    pub fn render<'rp>(
        &'rp self,
        render_pass: &mut wgpu::RenderPass<'rp>,
    ) -> Result<(), glyphon::RenderError> {
        self.renderer.render(&self.atlas, render_pass)?;

        Ok(())
    }

    pub fn trim(&mut self) {
        self.atlas.trim();
    }
}

pub fn measure_text(buffer: &glyphon::Buffer) -> (f32, f32) {
    let (width, total_lines) = buffer
        .layout_runs()
        .fold((0.0, 0usize), |(width, total_lines), run| {
            (run.line_w.max(width), total_lines + 1)
        });

    (width, total_lines as f32 * buffer.metrics().line_height)
}
