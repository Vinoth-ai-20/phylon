/// Configuration for the splat compute shader.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SplatConfig {
    /// Number of emitters to process.
    pub emitter_count: u32,
    /// Padding.
    pub _pad1: u32,
    /// Padding.
    pub _pad2: u32,
    /// Padding.
    pub _pad3: u32,
}

/// Represents a single splat/emitter on the GPU.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuSplat {
    /// The grid position (x, y).
    pub grid_pos: [f32; 2],
    /// The intensity/value of the splat.
    pub value: f32,
    /// The radius of the splat in grid coordinates.
    pub grid_radius: f32,
}

/// Configuration for the field rendering colormap.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FieldConfig {
    /// Minimum value to map to the start of the colormap.
    pub min_val: f32,
    /// Maximum value to map to the end of the colormap.
    pub max_val: f32,
    /// Camera position (x, y).
    pub camera_pos: [f32; 2],
    /// Camera zoom level.
    pub camera_zoom: f32,
    /// Padding to align screen_size to 8 bytes (as required by WGSL `vec2<f32>`).
    pub _pad0: u32,
    /// Screen dimensions (width, height).
    pub screen_size: [f32; 2],
    /// Colormap index.
    pub colormap: u32,
    /// Padding.
    pub _pad: u32,
}
/// Renders the scalar diffusion field as a background overlay.
pub struct FieldRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// The buffer storing the field configuration.
    pub config_buffer: wgpu::Buffer,
}

impl FieldRenderer {
    /// Creates a new FieldRenderer.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("FieldOverlayShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("field_overlay.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("FieldBindGroupLayout"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("FieldPipelineLayout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("FieldPipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
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
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Use linear filtering to smooth the lower-resolution compute grid over the screen
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("FieldSampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let config_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("FieldConfigBuffer"),
            size: std::mem::size_of::<FieldConfig>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            config_buffer,
        }
    }

    /// Renders the field into the specified render pass.
    pub fn render(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        field_texture_view: &wgpu::TextureView,
        viewport: Option<[u32; 4]>,
        clear_color: wgpu::Color,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("FieldBindGroup"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(field_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.config_buffer.as_entire_binding(),
                },
            ],
        });

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("FieldRenderPass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if let Some([x, y, w, h]) = viewport {
            if w > 0 && h > 0 {
                rpass.set_viewport(x as f32, y as f32, w as f32, h as f32, 0.0, 1.0);
                rpass.set_scissor_rect(x, y, w, h);
            }
        }

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &bind_group, &[]);
        rpass.draw(0..3, 0..1);
    }

    /// Updates the field configuration buffer.
    pub fn update_config(&self, queue: &wgpu::Queue, config: FieldConfig) {
        queue.write_buffer(&self.config_buffer, 0, bytemuck::bytes_of(&config));
    }
}

/// Compute pipeline for rasterizing points to a grid texture.
pub struct SplatComputePipeline {
    /// The compute pipeline.
    pub pipeline: wgpu::ComputePipeline,
    /// The bind group layout for splatting.
    pub bind_group_layout: wgpu::BindGroupLayout,
    /// The output texture.
    pub texture: wgpu::Texture,
    /// The view into the output texture.
    pub view: wgpu::TextureView,
    /// The configuration buffer.
    pub config_buffer: wgpu::Buffer,
    /// The splat data buffer.
    pub splat_buffer: wgpu::Buffer,
    /// The cached bind group.
    pub bind_group: wgpu::BindGroup,
    /// The current capacity of the splat buffer (number of items).
    pub splat_capacity: usize,
    /// Cached zeroed data for clearing the texture.
    pub empty_data: Vec<u8>,
    /// The width of the texture.
    pub width: u32,
    /// The height of the texture.
    pub height: u32,
}

impl SplatComputePipeline {
    /// Creates a new splat compute pipeline.
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SplatComputeShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("splat_compute.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SplatBindGroupLayout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
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
            label: Some("SplatPipelineLayout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("SplatComputePipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
            compilation_options: Default::default(),
            cache: None,
        });

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SplatTexture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let config_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SplatConfigBuffer"),
            size: std::mem::size_of::<SplatConfig>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let splat_capacity = 10000;
        let splat_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SplatDataBuffer"),
            size: (splat_capacity * std::mem::size_of::<GpuSplat>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SplatBindGroup"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: config_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: splat_buffer.as_entire_binding(),
                },
            ],
        });

        let empty_data = vec![0u8; (width * height * 4) as usize];

        Self {
            pipeline,
            bind_group_layout,
            texture,
            view,
            config_buffer,
            splat_buffer,
            bind_group,
            splat_capacity,
            empty_data,
            width,
            height,
        }
    }

    /// Executes the compute pass to splat data onto the texture.
    pub fn step(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, splats: &[GpuSplat]) {
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.empty_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(self.width * 4),
                rows_per_image: Some(self.height),
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SplatComputeEncoder"),
        });

        if splats.is_empty() {
            queue.submit(Some(encoder.finish()));
            return;
        }

        let config = SplatConfig {
            emitter_count: splats.len() as u32,
            _pad1: 0,
            _pad2: 0,
            _pad3: 0,
        };

        queue.write_buffer(&self.config_buffer, 0, bytemuck::bytes_of(&config));

        if splats.len() > self.splat_capacity {
            self.splat_capacity = splats.len().next_power_of_two().max(10000);
            self.splat_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("SplatDataBuffer"),
                size: (self.splat_capacity * std::mem::size_of::<GpuSplat>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("SplatBindGroup"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.config_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.splat_buffer.as_entire_binding(),
                    },
                ],
            });
        }

        queue.write_buffer(&self.splat_buffer, 0, bytemuck::cast_slice(splats));

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("SplatComputePass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.pipeline);
            cpass.set_bind_group(0, &self.bind_group, &[]);
            let workgroup_count_x = (self.width as f32 / 16.0).ceil() as u32;
            let workgroup_count_y = (self.height as f32 / 16.0).ceil() as u32;
            cpass.dispatch_workgroups(workgroup_count_x, workgroup_count_y, 1);
        }

        queue.submit(Some(encoder.finish()));
    }
}
