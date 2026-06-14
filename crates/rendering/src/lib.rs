//! wgpu rendering pipeline for debugging and visualisation.

use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use physics::Position;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct EntityInstance {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

impl EntityInstance {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] = [
        wgpu::VertexAttribute {
            offset: 0,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
            shader_location: 2,
            format: wgpu::VertexFormat::Float32x4,
        },
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<EntityInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

// A simple quad
const VERTICES: &[[f32; 2]] = &[[-0.5, -0.5], [0.5, -0.5], [-0.5, 0.5], [0.5, 0.5]];

const INDICES: &[u16] = &[0, 1, 2, 2, 1, 3];

pub struct DebugRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    camera_bind_group_layout: wgpu::BindGroupLayout,
    instance_buffer: wgpu::Buffer,
    instance_capacity: u32,
    instance_count: u32,
}

impl DebugRenderer {
    pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Debug Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/rendering/debug_dot.wgsl").into(),
            ),
        });

        // Setup camera

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Buffer"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            size: std::mem::size_of::<CameraUniform>() as u64,
            mapped_at_creation: false,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&camera_bind_group_layout],
                push_constant_ranges: &[],
            });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Debug Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2],
                    },
                    EntityInstance::desc(),
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
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
            label: Some("Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let initial_capacity = 10_000;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instance Buffer"),
            size: (initial_capacity * std::mem::size_of::<EntityInstance>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            camera_buffer,
            camera_bind_group,
            camera_bind_group_layout,
            instance_buffer,
            instance_capacity: initial_capacity as u32,
            instance_count: 0,
        }
    }

    pub fn camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.camera_bind_group_layout
    }

    pub fn camera_buffer(&self) -> &wgpu::Buffer {
        &self.camera_buffer
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        world: &mut world::PhylonWorld,
        config: &wgpu::SurfaceConfiguration,
    ) {
        puffin::profile_function!();

        // Update Camera
        let aspect_ratio = config.width as f32 / config.height as f32;
        // View size 1000 units wide
        let width = 1000.0;
        let height = width / aspect_ratio;

        let proj = Mat4::orthographic_rh(
            -width / 2.0,
            width / 2.0,
            -height / 2.0,
            height / 2.0,
            -1.0,
            1.0,
        );
        let view_proj = proj; // No camera translation for now

        let camera_uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
        };
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera_uniform));

        // Extract instances
        let mut instances = Vec::new();
        // In Phase 1 we only look for position and give a hardcoded color based on EntityId
        for (id, pos) in world.ecs.query_mut::<&Position>() {
            let hue = (id.to_bits().get() % 360) as f32 / 360.0;
            let color = Self::hsv_to_rgb(hue, 0.8, 1.0);

            instances.push(EntityInstance {
                position: pos.0.into(),
                color: [color[0], color[1], color[2], 1.0],
            });
        }

        self.instance_count = instances.len() as u32;

        if self.instance_count > 0 {
            if self.instance_count > self.instance_capacity {
                self.instance_capacity = self.instance_count.next_power_of_two();
                self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Instance Buffer"),
                    size: (self.instance_capacity as usize * std::mem::size_of::<EntityInstance>())
                        as wgpu::BufferAddress,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        }
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if self.instance_count > 0 {
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..INDICES.len() as u32, 0, 0..self.instance_count);
        }
    }

    fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
        let h_i = (h * 6.0) as i32;
        let f = h * 6.0 - h_i as f32;
        let p = v * (1.0 - s);
        let q = v * (1.0 - f * s);
        let t = v * (1.0 - (1.0 - f) * s);

        match h_i % 6 {
            0 => [v, t, p],
            1 => [q, v, p],
            2 => [p, v, t],
            3 => [p, q, v],
            4 => [t, p, v],
            _ => [v, p, q],
        }
    }
}

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

pub struct FieldRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    params_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_groups_cache: Option<[wgpu::BindGroup; 2]>,
}

impl FieldRenderer {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        camera_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Field Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/rendering/field_overlay.wgsl").into(),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Field Pipeline Layout"),
            bind_group_layouts: &[camera_layout], // Wait, the shader uses @group(0) for everything! So we need a combined layout, or we can use our `bind_group_layout` which expects camera to be at binding 0!
            push_constant_ranges: &[],
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
                    format: config.format,
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
