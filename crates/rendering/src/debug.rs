use wgpu::util::DeviceExt;

/// An instance for the debug renderer.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DebugInstance {
    /// World position A
    pub pos_a: [f32; 2],
    /// World position B (if equal to pos_a, renders a circle)
    pub pos_b: [f32; 2],
    /// Color (RGBA)
    pub color: [f32; 4],
    /// Radius or line thickness
    pub radius: f32,
    /// Type to distinguish components or set max_radius (0=Head, 1=Torso, 2=Muscle, 3=Tail, 4=Fin, 99=Line)
    pub segment_type: u32,
}

impl DebugInstance {
    const ATTRIBS: [wgpu::VertexAttribute; 5] = [
        wgpu::VertexAttribute {
            offset: 0,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
            shader_location: 2,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: (std::mem::size_of::<[f32; 2]>() * 2) as wgpu::BufferAddress,
            shader_location: 3,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: (std::mem::size_of::<[f32; 2]>() * 2 + std::mem::size_of::<[f32; 4]>())
                as wgpu::BufferAddress,
            shader_location: 4,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: (std::mem::size_of::<[f32; 2]>() * 2
                + std::mem::size_of::<[f32; 4]>()
                + std::mem::size_of::<f32>()) as wgpu::BufferAddress,
            shader_location: 5,
            format: wgpu::VertexFormat::Uint32,
        },
    ];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<DebugInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// The debug renderer.
pub struct DebugRenderer {
    pipeline: wgpu::RenderPipeline,
    camera_bind_group: wgpu::BindGroup,
    camera_buffer: wgpu::Buffer,
}

impl DebugRenderer {
    /// Creates a new DebugRenderer.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DebugShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("debug_quad.wgsl").into()),
        });

        // Setup camera uniform buffer
        let camera_matrix: [[f32; 4]; 4] = glam::Mat4::IDENTITY.to_cols_array_2d();

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("CameraBuffer"),
            contents: bytemuck::cast_slice(&camera_matrix),
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
            immediate_size: 0,
            label: Some("DebugPipelineLayout"),
            bind_group_layouts: &[Some(&camera_bind_group_layout)],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            multiview_mask: None,
            label: Some("DebugPipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[DebugInstance::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
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
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),

            cache: None,
        });

        Self {
            pipeline,
            camera_bind_group,
            camera_buffer,
        }
    }

    /// Renders instances.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        instances: &[DebugInstance],
        screen_size: [f32; 2],
        camera_pos: common::Vec2,
        camera_zoom: f32,
        viewport: Option<[u32; 4]>,
    ) {
        if instances.is_empty() {
            return;
        }

        // Orthographic projection mapping screen coordinates to clip space.
        let w = screen_size[0] / 2.0 / camera_zoom;
        let h = screen_size[1] / 2.0 / camera_zoom;
        let mut proj = glam::Mat4::orthographic_rh(-w, w, -h, h, -1.0, 1.0);
        proj *= glam::Mat4::from_translation(glam::Vec3::new(-camera_pos.x, -camera_pos.y, 0.0));
        let view_proj: [[f32; 4]; 4] = proj.to_cols_array_2d();
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&view_proj));

        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("InstanceBuffer"),
            contents: bytemuck::cast_slice(instances),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("DebugRenderEncoder"),
        });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                multiview_mask: None,
                label: Some("DebugRenderPass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // We load here because field_renderer cleared it
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
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
