use wgpu::util::DeviceExt;

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
        }
    }

    /// Dispatches the compute shader to integrate CTRNNs.
    pub fn compute_step(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        nodes: &mut [GpuCtrnnNode],
        synapses: &[GpuCtrnnSynapse],
        dt: f32,
    ) {
        if nodes.is_empty() {
            return;
        }

        // 1. Create buffers
        let nodes_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Brain Nodes Buffer"),
            contents: bytemuck::cast_slice(nodes),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });

        let empty_synapses = [GpuCtrnnSynapse {
            source: 0,
            target: 0,
            weight: 0.0,
            _padding: 0,
        }];
        let syn_slice = if synapses.is_empty() {
            &empty_synapses
        } else {
            synapses
        };

        let synapses_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Brain Synapses Buffer"),
            contents: bytemuck::cast_slice(syn_slice),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let config = BrainConfigUniform {
            dt,
            _padding: [0.0; 3],
        };
        let config_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Brain Config Buffer"),
            contents: bytemuck::cast_slice(&[config]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BrainBindGroup"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: nodes_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: synapses_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: config_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Brain Compute Encoder"),
        });

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Brain Integration Pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.integrate_pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            let workgroup_count = (nodes.len() as u32).div_ceil(64);
            cpass.dispatch_workgroups(workgroup_count, 1, 1);
        }

        // Staging buffer for readback
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Brain Readback Buffer"),
            size: std::mem::size_of_val(nodes) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        encoder.copy_buffer_to_buffer(&nodes_buffer, 0, &staging_buffer, 0, staging_buffer.size());

        let _submission_index = queue.submit(Some(encoder.finish()));

        // Map and read back
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

        device.poll(wgpu::Maintain::Wait);

        if receiver.recv().unwrap().is_ok() {
            let data = buffer_slice.get_mapped_range();
            nodes.copy_from_slice(bytemuck::cast_slice(&data));
            drop(data);
            staging_buffer.unmap();
        } else {
            panic!("failed to map brain staging buffer");
        }
    }
}
