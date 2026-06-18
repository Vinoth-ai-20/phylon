//! GPU compute pipeline for 2D field diffusion.

use wgpu::util::DeviceExt;

/// Uniforms for the diffusion compute shader.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DiffusionUniforms {
    /// Diffusion rate (D)
    pub diffusion_rate: f32,
    /// Decay rate (λ)
    pub decay_rate: f32,
    /// Timestep
    pub dt: f32,
    /// Number of active emitters
    pub emitter_count: u32,
}

/// GPU representation of a spatial emitter.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuEmitter {
    /// Position in grid coordinates
    pub grid_pos: [f32; 2],
    /// Value to emit per tick
    pub value: f32,
    /// Emission radius in grid cells
    pub grid_radius: f32,
}

/// Computes diffusion on a 2D scalar field texture.
pub struct DiffusionComputePipeline {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    #[allow(dead_code)]
    texture_a: wgpu::Texture,
    #[allow(dead_code)]
    texture_b: wgpu::Texture,
    view_a: wgpu::TextureView,
    view_b: wgpu::TextureView,
    #[allow(dead_code)]
    bind_group_a: wgpu::BindGroup,
    #[allow(dead_code)]
    bind_group_b: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    /// Keeps track of which texture is currently the "read" texture.
    /// If true, texture A is read and B is written. If false, B is read and A is written.
    pub read_a: bool,
    width: u32,
    height: u32,
}

impl DiffusionComputePipeline {
    /// Creates the diffusion compute pipeline and its ping-pong textures.
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DiffusionComputeShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("diffusion_step.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DiffusionBindGroupLayout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    // field_in
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    // field_out
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    // config uniforms
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    // emitters storage buffer
                    binding: 3,
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
            label: Some("DiffusionComputePipelineLayout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("DiffusionComputePipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
            compilation_options: Default::default(),
            cache: None,
        });

        let texture_desc = wgpu::TextureDescriptor {
            label: Some("DiffusionTexture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        };

        let texture_a = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("DiffusionTextureA"),
            ..texture_desc
        });
        let texture_b = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("DiffusionTextureB"),
            ..texture_desc
        });
        let view_a = texture_a.create_view(&wgpu::TextureViewDescriptor::default());
        let view_b = texture_b.create_view(&wgpu::TextureViewDescriptor::default());

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DiffusionUniforms"),
            size: std::mem::size_of::<DiffusionUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Dummy emitter buffer (must be at least 1 byte if we bind it, let's just make it sized for 1 emitter)
        let dummy_emitters = [GpuEmitter {
            grid_pos: [0.0, 0.0],
            value: 0.0,
            grid_radius: 0.0,
        }];
        let dummy_emitter_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("DummyEmitterBuffer"),
            contents: bytemuck::cast_slice(&dummy_emitters),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DiffusionBindGroupA"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: dummy_emitter_buffer.as_entire_binding(),
                },
            ],
        });

        let bind_group_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DiffusionBindGroupB"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: dummy_emitter_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            pipeline,
            bind_group_layout,
            texture_a,
            texture_b,
            view_a,
            view_b,
            bind_group_a,
            bind_group_b,
            uniform_buffer,
            read_a: true,
            width,
            height,
        }
    }

    /// Returns the view of the texture that holds the *current* stable field state.
    pub fn current_texture_view(&self) -> &wgpu::TextureView {
        if self.read_a {
            &self.view_a
        } else {
            &self.view_b
        }
    }

    /// Dispatches the compute shader to step the diffusion simulation.
    pub fn step(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        uniforms: DiffusionUniforms,
        emitters: &[GpuEmitter],
    ) {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        // Create emitter buffer for this frame
        let emitter_buffer = if emitters.is_empty() {
            let dummy = [GpuEmitter {
                grid_pos: [0.0, 0.0],
                value: 0.0,
                grid_radius: 0.0,
            }];
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("EmitterBuffer"),
                contents: bytemuck::cast_slice(&dummy),
                usage: wgpu::BufferUsages::STORAGE,
            })
        } else {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("EmitterBuffer"),
                contents: bytemuck::cast_slice(emitters),
                usage: wgpu::BufferUsages::STORAGE,
            })
        };

        // Recreate the active bind group so we can attach the dynamic emitter buffer.
        // We only need to recreate the one we're about to use.
        let active_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ActiveDiffusionBindGroup"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(if self.read_a {
                        &self.view_a
                    } else {
                        &self.view_b
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(if self.read_a {
                        &self.view_b
                    } else {
                        &self.view_a
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: emitter_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("DiffusionEncoder"),
        });

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("DiffusionComputePass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.pipeline);
            cpass.set_bind_group(0, &active_bind_group, &[]);

            let workgroup_count_x = (self.width as f32 / 16.0).ceil() as u32;
            let workgroup_count_y = (self.height as f32 / 16.0).ceil() as u32;
            cpass.dispatch_workgroups(workgroup_count_x, workgroup_count_y, 1);
        }

        queue.submit(Some(encoder.finish()));

        // Swap ping-pong direction
        self.read_a = !self.read_a;
    }
}
