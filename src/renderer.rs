use wgpu::util::DeviceExt;

use crate::painter::MeshHandle;
use crate::shapes::Mesh;
use crate::Vertex;
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
        }
    }

    pub fn render<'rp>(&'rp self, render_pass: &mut wgpu::RenderPass<'rp>, meshes: &[Mesh]) {
        let mut index_buffer_slices = self.index_buffer.slices.iter();
        let mut vertex_buffer_slices = self.vertex_buffer.slices.iter();
        for mesh in meshes {
            let index_buffer_slice = index_buffer_slices.next().unwrap();
            let vertex_buffer_slice = vertex_buffer_slices.next().unwrap();

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

            render_pass.draw_indexed(0..mesh.indices.len() as u32, 0, 0..1);
        }
    }

    pub fn update_textures(&mut self, queue: &wgpu::Queue, window_width: u32, window_height: u32) {}

    pub fn update_buffers(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        meshes: &[Mesh],
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
            for mesh in meshes {
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
