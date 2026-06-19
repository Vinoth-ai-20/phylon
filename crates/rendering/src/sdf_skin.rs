use wgpu::util::DeviceExt;

/// A single bone for SDF skin rendering.
///
/// Each bone corresponds to a `Rigid` spring between two `ParticleNode`s.
/// Its world-space endpoints and radius are used to compute the capsule SDF.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SdfBoneInstance {
    /// World-space endpoint A (e.g. the node with smaller index).
    pub pos_a: [f32; 2],
    /// World-space endpoint B.
    pub pos_b: [f32; 2],
    /// Capsule skin radius in world units.
    pub radius: f32,
    /// RGB tint (used for future per-organism colouring).
    pub color: [f32; 3],
}

impl SdfBoneInstance {
    // Vertex attributes: locations 1-4 (location 0 is reserved for built-ins)
    const ATTRIBS: [wgpu::VertexAttribute; 4] =
        wgpu::vertex_attr_array![1 => Float32x2, 2 => Float32x2, 3 => Float32, 4 => Float32x3];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SdfBoneInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Renders the organic SDF skin for all bones using a two-pass
/// accumulate-then-threshold technique.
///
/// ## Pass 1 — Accumulation
/// Each bone is rendered as a world-space AABB quad into a single-channel
/// `Rgba16Float` intermediate texture using **additive blending**.  The
/// fragment shader computes the capsule SDF and writes a density contribution
/// that is ≤ 0 outside the capsule and smoothly positive inside it.
///
/// ## Pass 2 — Composite
/// A single full-screen triangle samples the accumulated density texture.
/// Pixels where `density ≥ 1.0` are considered "inside" the organism skin.
/// `smoothstep(0.7, 1.0, density)` produces the final alpha value, yielding
/// an anti-aliased edge without visible seams at bone joints (where two bone
/// quads overlap, density simply sums to > 1, remaining fully opaque).
pub struct SdfSkinRenderer {
    // ── Accumulation pipeline ──────────────────────────────────────────────
    accum_pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,

    // ── Composite pipeline ─────────────────────────────────────────────────
    composite_pipeline: wgpu::RenderPipeline,
    composite_bind_group_layout: wgpu::BindGroupLayout,
    accum_sampler: wgpu::Sampler,

    // ── Size-dependent accumulation texture ───────────────────────────────
    composite_bind_group: wgpu::BindGroup,
    current_width: u32,
    current_height: u32,
}

/// The texture format used for the intermediate density accumulation target.
const ACCUM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

impl SdfSkinRenderer {
    /// Creates a new `SdfSkinRenderer`.
    ///
    /// `surface_format` must be the swapchain format (used for the composite
    /// pipeline's colour target).
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        // ── Camera uniform ─────────────────────────────────────────────────
        let camera_matrix: [[f32; 4]; 4] = glam::Mat4::IDENTITY.to_cols_array_2d();
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SdfCameraBuffer"),
            contents: bytemuck::cast_slice(&camera_matrix),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SdfCameraBindGroupLayout"),
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
            label: Some("SdfCameraBindGroup"),
            layout: &camera_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // ── Accumulation pipeline ──────────────────────────────────────────
        let accum_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SdfAccumShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("sdf_accum.wgsl").into()),
        });

        let accum_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("SdfAccumPipelineLayout"),
                bind_group_layouts: &[&camera_bgl],
                push_constant_ranges: &[],
            });

        let accum_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("SdfAccumPipeline"),
            layout: Some(&accum_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &accum_shader,
                entry_point: "vs_accum",
                buffers: &[SdfBoneInstance::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &accum_shader,
                entry_point: "fs_accum",
                targets: &[Some(wgpu::ColorTargetState {
                    format: ACCUM_FORMAT,
                    // Additive blending: accumulated density = sum of all bone contributions
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // ── Composite pipeline ─────────────────────────────────────────────
        let composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SdfCompositeShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("sdf_composite.wgsl").into()),
        });

        let composite_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SdfCompositeBGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
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

        let composite_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("SdfCompositePipelineLayout"),
                bind_group_layouts: &[&composite_bgl],
                push_constant_ranges: &[],
            });

        let composite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("SdfCompositePipeline"),
            layout: Some(&composite_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &composite_shader,
                entry_point: "vs_composite",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &composite_shader,
                entry_point: "fs_composite",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let accum_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SdfAccumSampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let (_, accum_view) = Self::create_accum_texture(device, width, height);
        let composite_bind_group =
            Self::create_composite_bind_group(device, &composite_bgl, &accum_view, &accum_sampler);

        Self {
            accum_pipeline,
            camera_buffer,
            camera_bind_group,
            composite_pipeline,
            composite_bind_group_layout: composite_bgl,
            accum_sampler,
            composite_bind_group,
            current_width: width,
            current_height: height,
        }
    }

    fn create_accum_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SdfAccumTexture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: ACCUM_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        (tex, view)
    }

    fn create_composite_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        accum_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SdfCompositeBindGroup"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(accum_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    /// Recreates the accumulation texture when the surface is resized.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == self.current_width && height == self.current_height {
            return;
        }
        let (_, view) = Self::create_accum_texture(device, width, height);
        self.composite_bind_group = Self::create_composite_bind_group(
            device,
            &self.composite_bind_group_layout,
            &view,
            &self.accum_sampler,
        );
        self.current_width = width;
        self.current_height = height;
    }

    /// Renders the organic SDF skin for the given bone list onto `target_view`.
    ///
    /// `target_view` must be the current swapchain surface view. The renderer
    /// loads the existing colour data (field background + nodes already drawn)
    /// and alpha-composites the skin on top.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_view: &wgpu::TextureView,
        bones: &[SdfBoneInstance],
        screen_size: [f32; 2],
        camera_pos: common::Vec2,
        camera_zoom: f32,
        viewport: Option<[u32; 4]>,
    ) {
        if bones.is_empty() {
            return;
        }

        // Resize accumulation texture if the surface changed size
        self.resize(device, screen_size[0] as u32, screen_size[1] as u32);

        // Update camera matrix
        let w = screen_size[0] / 2.0 / camera_zoom;
        let h = screen_size[1] / 2.0 / camera_zoom;
        let mut proj = glam::Mat4::orthographic_rh(-w, w, -h, h, -1.0, 1.0);
        proj *= glam::Mat4::from_translation(glam::Vec3::new(-camera_pos.x, -camera_pos.y, 0.0));
        queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&proj.to_cols_array_2d()),
        );

        let bone_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SdfBoneBuffer"),
            contents: bytemuck::cast_slice(bones),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SdfEncoder"),
        });

        // ── Pass 1: accumulate density into offscreen texture ──────────────
        {
            // We need a fresh view each time since resize() may have invalidated it.
            let (_, accum_view) =
                Self::create_accum_texture(device, self.current_width, self.current_height);
            // Recreate composite bind group with the fresh view
            self.composite_bind_group = Self::create_composite_bind_group(
                device,
                &self.composite_bind_group_layout,
                &accum_view,
                &self.accum_sampler,
            );

            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SdfAccumPass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &accum_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), // Clear density to 0
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rpass.set_pipeline(&self.accum_pipeline);
            rpass.set_bind_group(0, &self.camera_bind_group, &[]);
            rpass.set_vertex_buffer(0, bone_buffer.slice(..));
            rpass.draw(0..4, 0..bones.len() as u32);
        }

        // ── Pass 2: composite onto swapchain ──────────────────────────────
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SdfCompositePass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Composite onto existing frame
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

            rpass.set_pipeline(&self.composite_pipeline);
            rpass.set_bind_group(0, &self.composite_bind_group, &[]);
            rpass.draw(0..3, 0..1); // Full-screen triangle
        }

        queue.submit(Some(encoder.finish()));
    }
}
