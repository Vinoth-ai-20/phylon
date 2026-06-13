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
    instance_buffer: Option<wgpu::Buffer>,
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

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            camera_buffer,
            camera_bind_group,
            instance_buffer: None,
            instance_count: 0,
        }
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
            use wgpu::util::DeviceExt;
            self.instance_buffer = Some(device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Instance Buffer"),
                    contents: bytemuck::cast_slice(&instances),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        }
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if self.instance_count > 0 {
            if let Some(instance_buffer) = &self.instance_buffer {
                render_pass.set_pipeline(&self.pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
                render_pass
                    .set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..INDICES.len() as u32, 0, 0..self.instance_count);
            }
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
