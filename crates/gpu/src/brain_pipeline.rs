/// A GPU-friendly representation of a CTRNN Node.
#[repr(C)]
#[allow(missing_docs)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuCtrnnNode {
    pub state: f32,
    pub time_constant: f32,
    pub bias: f32,
    pub activation: u32,
    pub first_synapse: u32,
    pub synapse_count: u32,
}

/// A GPU-friendly representation of a CTRNN Synapse.
#[repr(C)]
#[allow(missing_docs)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuCtrnnSynapse {
    pub source: u32,
    pub target: u32,
    pub weight: f32,
    pub _padding: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct BrainConfigUniform {
    dt: f32,
    _padding: [f32; 3],
}

/// Wrapper around the brain WGSL compute pipelines.
pub struct BrainComputePipeline {
    integrate_pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,

    // Persistent, geometrically-grown GPU buffers — recreated only when the
    // population outgrows current capacity, instead of every single tick.
    // Any stale tail data beyond the live node/synapse count is harmless:
    // `integrate_nodes`'s entry guard only skips indices beyond capacity
    // (not beyond the live count), but phantom tail nodes are self-contained
    // — nothing reads their state, and the CPU readback only copies back the
    // live byte range.
    node_capacity: usize,
    synapse_capacity: usize,
    nodes_buffer: Option<wgpu::Buffer>,
    synapses_buffer: Option<wgpu::Buffer>,
    config_buffer: Option<wgpu::Buffer>,
    staging_buffer: Option<wgpu::Buffer>,
    bind_group: Option<wgpu::BindGroup>,
}

impl BrainComputePipeline {
    /// Compiles and initializes the brain compute shaders.
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("BrainComputeShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("brain.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("BrainBindGroupLayout"),
            entries: &[
                // nodes
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // synapses
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // config
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
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
            label: Some("BrainComputePipelineLayout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let integrate_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("BrainIntegratePipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "integrate_nodes",
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            integrate_pipeline,
            bind_group_layout,
            node_capacity: 0,
            synapse_capacity: 0,
            nodes_buffer: None,
            synapses_buffer: None,
            config_buffer: None,
            staging_buffer: None,
            bind_group: None,
        }
    }

    /// Dispatches the compute shader to integrate CTRNNs and blocks for the
    /// result. Kept for callers needing same-tick results; the simulation's
    /// hot path uses [`Self::dispatch`] + [`Self::resolve`] so the GPU work
    /// for tick N can overlap with CPU work instead of stalling immediately.
    #[allow(dead_code)]
    pub fn compute_step(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        nodes: &mut [GpuCtrnnNode],
        synapses: &[GpuCtrnnSynapse],
        dt: f32,
    ) {
        let pending = self.dispatch(device, queue, nodes, synapses, dt);
        let result = self.resolve(device, pending);
        nodes.copy_from_slice(&result);
    }

    /// Submits the CTRNN integration pass and kicks off an asynchronous
    /// buffer readback, returning immediately without blocking on the GPU.
    pub fn dispatch(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        nodes: &[GpuCtrnnNode],
        synapses: &[GpuCtrnnSynapse],
        dt: f32,
    ) -> PendingBrainReadback {
        if nodes.is_empty() {
            return PendingBrainReadback::Ready(Vec::new());
        }

        self.ensure_capacity(device, nodes.len(), synapses.len());

        let nodes_buffer = self.nodes_buffer.as_ref().unwrap();
        let synapses_buffer = self.synapses_buffer.as_ref().unwrap();
        let config_buffer = self.config_buffer.as_ref().unwrap();
        let staging_buffer = self.staging_buffer.as_ref().unwrap();
        let bind_group = self.bind_group.as_ref().unwrap();

        queue.write_buffer(nodes_buffer, 0, bytemuck::cast_slice(nodes));
        if !synapses.is_empty() {
            queue.write_buffer(synapses_buffer, 0, bytemuck::cast_slice(synapses));
        }

        let config = BrainConfigUniform {
            dt,
            _padding: [0.0; 3],
        };
        queue.write_buffer(config_buffer, 0, bytemuck::cast_slice(&[config]));

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Brain Compute Encoder"),
        });

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Brain Integration Pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.integrate_pipeline);
            cpass.set_bind_group(0, bind_group, &[]);
            let workgroup_count = (nodes.len() as u32).div_ceil(64);
            cpass.dispatch_workgroups(workgroup_count, 1, 1);
        }

        let byte_len = std::mem::size_of_val(nodes);
        encoder.copy_buffer_to_buffer(
            nodes_buffer,
            0,
            staging_buffer,
            0,
            byte_len as wgpu::BufferAddress,
        );

        let _submission_index = queue.submit(Some(encoder.finish()));

        // Map and read back (async — see `resolve`)
        let (sender, receiver) = std::sync::mpsc::channel();
        staging_buffer
            .slice(..byte_len as wgpu::BufferAddress)
            .map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

        PendingBrainReadback::Mapping { receiver, byte_len }
    }

    /// Grows (never shrinks) the persistent GPU buffers/bind group to hold
    /// at least `node_count`/`synapse_count` entries, doubling capacity each
    /// time to amortize reallocation across many ticks.
    fn ensure_capacity(&mut self, device: &wgpu::Device, node_count: usize, synapse_count: usize) {
        let needs_growth = node_count > self.node_capacity || synapse_count > self.synapse_capacity;
        if !needs_growth && self.bind_group.is_some() {
            return;
        }

        if needs_growth {
            self.node_capacity = node_count.max(self.node_capacity * 2).max(256);
            self.synapse_capacity = synapse_count.max(self.synapse_capacity * 2).max(256);
        }

        let node_bytes =
            (self.node_capacity * std::mem::size_of::<GpuCtrnnNode>()) as wgpu::BufferAddress;
        let synapse_bytes =
            (self.synapse_capacity * std::mem::size_of::<GpuCtrnnSynapse>()) as wgpu::BufferAddress;

        self.nodes_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Brain Nodes Buffer"),
            size: node_bytes,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));
        self.synapses_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Brain Synapses Buffer"),
            size: synapse_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        self.staging_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Brain Readback Buffer"),
            size: node_bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        if self.config_buffer.is_none() {
            self.config_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Brain Config Buffer"),
                size: std::mem::size_of::<BrainConfigUniform>() as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BrainBindGroup"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.nodes_buffer.as_ref().unwrap().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.synapses_buffer.as_ref().unwrap().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.config_buffer.as_ref().unwrap().as_entire_binding(),
                },
            ],
        }));
    }

    /// Blocks (briefly — see [`Self::dispatch`]) until the GPU work behind a
    /// [`PendingBrainReadback`] is complete, then returns the updated nodes.
    pub fn resolve(
        &self,
        device: &wgpu::Device,
        pending: PendingBrainReadback,
    ) -> Vec<GpuCtrnnNode> {
        match pending {
            PendingBrainReadback::Ready(nodes) => nodes,
            PendingBrainReadback::Mapping { receiver, byte_len } => {
                device.poll(wgpu::Maintain::Wait);
                if receiver.recv().unwrap().is_ok() {
                    let staging_buffer = self
                        .staging_buffer
                        .as_ref()
                        .expect("staging buffer must exist after a Mapping dispatch");
                    let data = staging_buffer
                        .slice(..byte_len as wgpu::BufferAddress)
                        .get_mapped_range();
                    let result: Vec<GpuCtrnnNode> = bytemuck::cast_slice(&data).to_vec();
                    drop(data);
                    staging_buffer.unmap();
                    result
                } else {
                    panic!("failed to map brain staging buffer");
                }
            }
        }
    }
}

/// A brain (CTRNN) readback that's either already resolved (empty-input fast
/// path) or in flight on the GPU, to be collected later via
/// [`BrainComputePipeline::resolve`].
pub enum PendingBrainReadback {
    /// No GPU work was dispatched; the result is empty.
    Ready(Vec<GpuCtrnnNode>),
    /// A staging buffer is being asynchronously mapped for read access;
    /// `byte_len` is how many bytes (from the start) are valid to read.
    Mapping {
        /// Fires once `map_async` completes.
        receiver: std::sync::mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
        /// Number of valid bytes at the start of the pipeline's staging buffer.
        byte_len: usize,
    },
}
