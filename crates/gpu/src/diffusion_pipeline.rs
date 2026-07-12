//! GPU compute pipeline for 2D field diffusion.

/// Number of texture-array layers the diffusion field carries: Pheromones,
/// Energy, O2, CO2, and Morphogen — see `diffusion::FieldLayer`. A single
/// named constant so every hardcoded shape (texture depth, layer-view count,
/// dispatch z-extent, uniform array size, staging buffer size) stays in
/// lockstep when this ever changes again, rather than requiring an audit of
/// scattered literal `4`s/`5`s.
pub const LAYER_COUNT: u32 = 5;

/// Configuration for a single diffusion layer (e.g. Pheromones, Energy, O2, CO2, Morphogen).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LayerConfig {
    /// Diffusion rate (D)
    pub diffusion_rate: f32,
    /// Decay rate (λ)
    pub decay_rate: f32,
    /// Number of active emitters for this layer
    pub emitter_count: u32,
    /// Offset into the global emitter buffer for this layer
    pub emitter_offset: u32,
}

/// Uniforms for the diffusion compute shader.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DiffusionUniforms {
    /// Timestep
    pub dt: f32,
    /// Padding for alignment.
    pub _pad1: u32,
    /// Padding for alignment.
    pub _pad2: u32,
    /// Padding for alignment.
    pub _pad3: u32,
    /// Config for each of the [`LAYER_COUNT`] layers: Pheromones, Energy,
    /// O2, CO2, Morphogen.
    pub layers: [LayerConfig; LAYER_COUNT as usize],
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
    /// Pre-created 2D views for each layer of the current read texture.
    layer_views_a: Vec<wgpu::TextureView>,
    layer_views_b: Vec<wgpu::TextureView>,
    bind_group_a: wgpu::BindGroup,
    bind_group_b: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,

    // Persistent, geometrically-grown emitter buffer — replaces recreating
    // a fresh buffer + bind group every tick. `bind_group_a`/`bind_group_b`
    // are rebuilt only when `emitter_capacity` grows (see
    // `ensure_emitter_capacity`), same pattern as the physics/brain
    // pipelines.
    emitter_capacity: usize,
    emitter_buffer: wgpu::Buffer,
    /// Keeps track of which texture is currently the "read" texture.
    /// If true, texture A is read and B is written. If false, B is read and A is written.
    pub read_a: bool,
    width: u32,
    height: u32,

    staging_buffers: [wgpu::Buffer; 2],
    has_been_mapped: [bool; 2],
    frame_index: usize,
    ready_tx: std::sync::mpsc::Sender<usize>,
    ready_rx: std::sync::mpsc::Receiver<usize>,
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
                        view_dimension: wgpu::TextureViewDimension::D2Array,
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
                        view_dimension: wgpu::TextureViewDimension::D2Array,
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
                depth_or_array_layers: LAYER_COUNT,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
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
        let view_desc = wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        };
        let view_a = texture_a.create_view(&view_desc);
        let view_b = texture_b.create_view(&view_desc);

        let mut layer_views_a = Vec::with_capacity(LAYER_COUNT as usize);
        let mut layer_views_b = Vec::with_capacity(LAYER_COUNT as usize);
        for i in 0..LAYER_COUNT {
            let layer_desc = wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: i,
                array_layer_count: Some(1),
                ..Default::default()
            };
            layer_views_a.push(texture_a.create_view(&layer_desc));
            layer_views_b.push(texture_b.create_view(&layer_desc));
        }

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DiffusionUniforms"),
            size: std::mem::size_of::<DiffusionUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Persistent emitter buffer, grown geometrically as needed (see
        // `ensure_emitter_capacity`) instead of recreated every tick.
        let emitter_capacity = 256usize;
        let emitter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DiffusionEmittersBuffer"),
            size: (emitter_capacity * std::mem::size_of::<GpuEmitter>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
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
                    resource: emitter_buffer.as_entire_binding(),
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
                    resource: emitter_buffer.as_entire_binding(),
                },
            ],
        });

        let staging_buffer_size = (width * height * LAYER_COUNT * 4) as wgpu::BufferAddress;
        let staging_buffers = [
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("DiffusionStagingBuffer0"),
                size: staging_buffer_size,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("DiffusionStagingBuffer1"),
                size: staging_buffer_size,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        ];

        let (ready_tx, ready_rx) = std::sync::mpsc::channel();

        Self {
            pipeline,
            bind_group_layout,
            texture_a,
            texture_b,
            view_a,
            view_b,
            layer_views_a,
            layer_views_b,
            bind_group_a,
            bind_group_b,
            uniform_buffer,
            emitter_capacity,
            emitter_buffer,
            read_a: true,
            width,
            height,
            staging_buffers,
            has_been_mapped: [false, false],
            frame_index: 0,
            ready_tx,
            ready_rx,
        }
    }

    /// Returns the active texture view for rendering a specific layer (field).
    pub fn current_layer_view(&self, layer: u32) -> &wgpu::TextureView {
        let index = layer as usize;
        if self.read_a {
            &self.layer_views_a[index]
        } else {
            &self.layer_views_b[index]
        }
    }

    /// Dispatches the compute shader to step the diffusion simulation.
    pub fn step(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        uniforms: DiffusionUniforms,
        emitters: &[GpuEmitter],
        query_set: Option<&wgpu::QuerySet>,
    ) {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        self.ensure_emitter_capacity(device, emitters.len());
        if !emitters.is_empty() {
            queue.write_buffer(&self.emitter_buffer, 0, bytemuck::cast_slice(emitters));
        }

        let active_bind_group = if self.read_a {
            &self.bind_group_a
        } else {
            &self.bind_group_b
        };

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("DiffusionComputeEncoder"),
        });

        if let Some(qs) = query_set {
            encoder.write_timestamp(qs, 2);
        }

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("DiffusionComputePass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.pipeline);
            cpass.set_bind_group(0, active_bind_group, &[]);

            cpass.dispatch_workgroups(self.width / 16, self.height / 16, LAYER_COUNT);
        }

        if let Some(qs) = query_set {
            encoder.write_timestamp(qs, 3);
        }

        // The compute pass wrote to the "other" texture (view_b if read_a, view_a if !read_a)
        let output_texture = if self.read_a {
            &self.texture_b
        } else {
            &self.texture_a
        };

        // Prepare the staging buffer
        let buf_idx = self.frame_index % 2;
        if self.has_been_mapped[buf_idx] {
            self.staging_buffers[buf_idx].unmap();
        }

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: output_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &self.staging_buffers[buf_idx],
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.width * 4),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: LAYER_COUNT,
            },
        );

        queue.submit(Some(encoder.finish()));

        // Start mapping the buffer we just copied into
        self.has_been_mapped[buf_idx] = true;
        let tx = self.ready_tx.clone();
        let slice = self.staging_buffers[buf_idx].slice(..);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            if result.is_ok() {
                let _ = tx.send(buf_idx);
            }
        });

        // Swap ping-pong direction
        self.read_a = !self.read_a;
        self.frame_index += 1;
    }

    /// Grows (never shrinks) the persistent emitter buffer to hold at least
    /// `emitter_count` entries, doubling capacity each time. Rebuilds both
    /// ping-pong bind groups only when the buffer is actually replaced.
    fn ensure_emitter_capacity(&mut self, device: &wgpu::Device, emitter_count: usize) {
        if emitter_count <= self.emitter_capacity {
            return;
        }

        self.emitter_capacity = emitter_count.max(self.emitter_capacity * 2).max(256);
        self.emitter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DiffusionEmittersBuffer"),
            size: (self.emitter_capacity * std::mem::size_of::<GpuEmitter>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.bind_group_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DiffusionBindGroupA"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.emitter_buffer.as_entire_binding(),
                },
            ],
        });

        self.bind_group_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DiffusionBindGroupB"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.emitter_buffer.as_entire_binding(),
                },
            ],
        });
    }

    /// Tries to read the latest available field state from the GPU.
    pub fn try_read_field(&self, device: &wgpu::Device) -> Option<Vec<f32>> {
        // Drain the channel to get the latest mapped buffer
        let mut latest_idx = None;
        while let Ok(idx) = self.ready_rx.try_recv() {
            latest_idx = Some(idx);
        }

        if let Some(idx) = latest_idx {
            // Wgpu requires the main thread to poll before the buffer state officially updates to Mapped,
            // even if the map_async callback already fired on a background thread.
            device.poll(wgpu::Maintain::Wait);

            let slice = self.staging_buffers[idx].slice(..);
            let data = slice.get_mapped_range();
            let floats: &[f32] = bytemuck::cast_slice(&data);
            let vec = floats.to_vec();
            drop(data);
            // We DO NOT unmap here. We leave it mapped. It will be unmapped at the top of step()
            // when it's this buffer's turn to be written to again.
            Some(vec)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors `crates/app/src/app.rs`'s `init_gpu_headless` — the same
    /// adapter/device request shape the real app uses when it runs without
    /// a window, reused here so this test exercises a real wgpu device, not
    /// a mock.
    fn headless_device() -> (wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .expect("no suitable GPU adapter found for this headless test");
        pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("DiffusionPipelineTestDevice"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .expect("failed to create wgpu device for this headless test")
    }

    /// Exercises a real wgpu device, a real compute dispatch, and a real
    /// readback rather than mocking any of it — GPU-touching logic like this
    /// can silently break in ways a pure-CPU unit test can't catch. Proves
    /// `LAYER_COUNT = 5` actually round-trips end to end (texture depth,
    /// dispatch z-extent, and the uniform layer array all have to agree — a
    /// mismatch between any of these would silently corrupt data or panic
    /// inside the shader, not just fail a plain value comparison), and that
    /// an emission targeted only at the Morphogen layer (index 4) produces
    /// activity there without leaking into layers 0-3 — the GPU-side half of
    /// the same "no cross-channel bleed" requirement the CPU-side
    /// `diffusion` crate test proves for `CpuFieldState`.
    #[test]
    fn diffusion_pipeline_steps_all_5_layers_and_the_morphogen_layer_stays_isolated() {
        let (device, queue) = headless_device();
        let mut pipeline = DiffusionComputePipeline::new(&device, 64, 64);

        let emitters = [GpuEmitter {
            grid_pos: [32.0, 32.0],
            value: 5.0,
            grid_radius: 8.0,
        }];
        let mut layers = [LayerConfig {
            diffusion_rate: 0.0,
            decay_rate: 0.0,
            emitter_count: 0,
            emitter_offset: 0,
        }; LAYER_COUNT as usize];
        layers[4] = LayerConfig {
            diffusion_rate: 0.3,
            decay_rate: 0.0,
            emitter_count: 1,
            emitter_offset: 0,
        };

        pipeline.step(
            &device,
            &queue,
            DiffusionUniforms {
                dt: 1.0,
                _pad1: 0,
                _pad2: 0,
                _pad3: 0,
                layers,
            },
            &emitters,
            None,
        );

        // `step` only starts the async readback; force it to complete
        // before asking for it, unlike the real app's per-frame
        // best-effort `try_read_field` poll.
        device.poll(wgpu::Maintain::Wait);
        let field = pipeline
            .try_read_field(&device)
            .expect("readback should be available after an explicit device.poll(Wait)");

        let layer_size = 64 * 64;
        assert_eq!(field.len(), layer_size * LAYER_COUNT as usize);

        let center_idx = 32 * 64 + 32;
        for layer in 0..4usize {
            let value = field[layer * layer_size + center_idx];
            assert_eq!(
                value, 0.0,
                "layer {layer} should be untouched by a Morphogen-only emitter"
            );
        }
        let morphogen_value = field[4 * layer_size + center_idx];
        assert!(
            morphogen_value > 0.0,
            "Morphogen layer should show activity near its own emitter, got {morphogen_value}"
        );
    }
}
