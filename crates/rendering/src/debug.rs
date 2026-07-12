use wgpu::util::DeviceExt;

/// The depth format shared with `OrganismRenderer`'s depth buffer — badges
/// are depth-tested against it, so both renderers must agree on the format.
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// An instance for the debug renderer.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DebugInstance {
    /// World position A
    pub pos_a: [f32; 3],
    /// World position B (if equal to pos_a, renders a circle)
    pub pos_b: [f32; 3],
    /// Color (RGBA)
    pub color: [f32; 4],
    /// Radius or line thickness
    pub radius: f32,
    /// Type to distinguish components or set max_radius (0=Head, 1=Torso, 2=Muscle, 3=Tail, 4=Fin, 99=Line)
    pub segment_type: u32,
}

impl DebugInstance {
    const ATTRIBS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        1 => Float32x3, // pos_a
        2 => Float32x3, // pos_b
        3 => Float32x4, // color
        4 => Float32,   // radius
        5 => Uint32,    // segment_type
    ];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<DebugInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuCamera {
    view_proj: [[f32; 4]; 4],
    right: [f32; 3],
    _pad0: f32,
    up: [f32; 3],
    _pad1: f32,
}

/// The debug renderer.
///
/// Renders Health/Disease/Category badges and colony-link markers as
/// camera-facing billboards, depth-tested against — but not writing into —
/// `OrganismRenderer`'s shared depth buffer (via its `depth_view()`
/// accessor), so badges correctly hide behind nearer organisms instead of
/// always drawing flat-on-top regardless of occlusion.
pub struct DebugRenderer {
    pipeline: wgpu::RenderPipeline,
    camera_bind_group: wgpu::BindGroup,
    camera_buffer: wgpu::Buffer,

    // Persistent, geometrically-grown vertex buffer for the instance list —
    // replaces recreating a fresh buffer every render call.
    instance_capacity: usize,
    instance_buffer: Option<wgpu::Buffer>,
}

impl DebugRenderer {
    /// Creates a new DebugRenderer.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DebugShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("debug_quad.wgsl").into()),
        });

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("CameraBuffer"),
            contents: bytemuck::bytes_of(&GpuCamera {
                view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
                right: [1.0, 0.0, 0.0],
                _pad0: 0.0,
                up: [0.0, 1.0, 0.0],
                _pad1: 0.0,
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("CameraBindGroupLayout"),
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
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("CameraBindGroup"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("DebugPipelineLayout"),
            bind_group_layouts: &[&camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("DebugPipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[DebugInstance::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            // Test against, but never write, the scene depth buffer shared
            // with `OrganismRenderer` — badges hide behind nearer geometry
            // but never occlude each other by draw order alone.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            camera_bind_group,
            camera_buffer,
            instance_capacity: 0,
            instance_buffer: None,
        }
    }

    /// Renders instances.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        instances: &[DebugInstance],
        depth_view: &wgpu::TextureView,
        view_proj: glam::Mat4,
        camera_right: glam::Vec3,
        camera_up: glam::Vec3,
        viewport: Option<[u32; 4]>,
    ) {
        if instances.is_empty() {
            return;
        }

        queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&GpuCamera {
                view_proj: view_proj.to_cols_array_2d(),
                right: camera_right.into(),
                _pad0: 0.0,
                up: camera_up.into(),
                _pad1: 0.0,
            }),
        );

        if instances.len() > self.instance_capacity || self.instance_buffer.is_none() {
            self.instance_capacity = instances.len().max(self.instance_capacity * 2).max(256);
            self.instance_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("InstanceBuffer"),
                size: (self.instance_capacity * std::mem::size_of::<DebugInstance>())
                    as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let instance_buffer = self.instance_buffer.as_ref().unwrap();
        queue.write_buffer(instance_buffer, 0, bytemuck::cast_slice(instances));

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("DebugRenderEncoder"),
        });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("DebugRenderPass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // We load here because field_renderer cleared it
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Some([vx, vy, vw, vh]) = viewport {
                if vw > 0 && vh > 0 {
                    rpass.set_viewport(vx as f32, vy as f32, vw as f32, vh as f32, 0.0, 1.0);
                    rpass.set_scissor_rect(vx, vy, vw, vh);
                }
            }

            rpass.set_pipeline(&self.pipeline);
            rpass.set_bind_group(0, &self.camera_bind_group, &[]);
            rpass.set_vertex_buffer(0, instance_buffer.slice(..));
            rpass.draw(0..4, 0..instances.len() as u32);
        }

        queue.submit(Some(encoder.finish()));
    }
}
