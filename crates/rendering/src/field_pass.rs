use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct FieldParams {
    width: u32,
    height: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct QuadVertex {
    position: [f32; 2],
    uv: [f32; 2],
}

impl QuadVertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] = [
        wgpu::VertexAttribute {
            offset: 0,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32x2,
        },
    ];
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<QuadVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

const FIELD_VERTICES: &[QuadVertex] = &[
    QuadVertex {
        position: [-0.5, -0.5],
        uv: [0.0, 1.0],
    },
    QuadVertex {
        position: [0.5, -0.5],
        uv: [1.0, 1.0],
    },
    QuadVertex {
        position: [-0.5, 0.5],
        uv: [0.0, 0.0],
    },
    QuadVertex {
        position: [0.5, 0.5],
        uv: [1.0, 0.0],
    },
];
const FIELD_INDICES: &[u16] = &[0, 1, 2, 2, 1, 3];

pub struct FieldPass {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    params_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_groups_cache: Option<[wgpu::BindGroup; 2]>,
}

impl FieldPass {
    pub fn new(
        device: &wgpu::Device,
        _config: &wgpu::SurfaceConfiguration,
        _camera_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Field Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/rendering/field.wgsl").into(),
            ),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Field Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let actual_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Field Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Field Render Pipeline"),
            layout: Some(&actual_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[QuadVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        use wgpu::util::DeviceExt;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Field Vertex Buffer"),
            contents: bytemuck::cast_slice(FIELD_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Field Index Buffer"),
            contents: bytemuck::cast_slice(FIELD_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Field Params Buffer"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            size: std::mem::size_of::<FieldParams>() as u64,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            params_buffer,
            bind_group_layout,
            bind_groups_cache: None,
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        field: &diffusion::DiffusionField,
        camera_buffer: &wgpu::Buffer,
    ) {
        let params = FieldParams {
            width: field.width,
            height: field.height,
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        if self.bind_groups_cache.is_none() {
            let create_bg = |buffer: &wgpu::Buffer| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Field Dynamic Bind Group"),
                    layout: &self.bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: camera_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: self.params_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: buffer.as_entire_binding(),
                        },
                    ],
                })
            };
            self.bind_groups_cache = Some([create_bg(&field.buffer_a), create_bg(&field.buffer_b)]);
        }
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        field: &'a diffusion::DiffusionField,
    ) {
        let bg_idx = if field.flip { 1 } else { 0 };
        if let Some(cache) = &self.bind_groups_cache {
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &cache[bg_idx], &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..FIELD_INDICES.len() as u32, 0, 0..1);
        }
    }
}
