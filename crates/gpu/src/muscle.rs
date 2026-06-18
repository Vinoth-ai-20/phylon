use wgpu::util::DeviceExt;

/// A spring represented for GPU compute.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuSpring {
    /// Node A entity index
    pub node_a: u32,
    /// Node B entity index
    pub node_b: u32,
    /// Current rest length
    pub rest_length: f32,
    /// Base length
    pub base_length: f32,
    /// Spring stiffness
    pub stiffness: f32,
    /// Spring damping
    pub damping: f32,
    /// Muscle actuation amplitude
    pub actuation_amplitude: f32,
    /// Muscle actuation phase
    pub actuation_phase: f32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct TimeUniform {
    t: f32,
}

/// The compute pipeline for muscle actuation.
pub struct MuscleComputePipeline {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl MuscleComputePipeline {
    /// Creates a new MuscleComputePipeline.
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("MuscleComputeShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("muscle_actuation.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MuscleBindGroupLayout"),
            entries: &[
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
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("MuscleComputePipelineLayout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("MuscleComputePipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            pipeline,
            bind_group_layout,
        }
    }

    /// Dispatches the compute shader and blocks to read back the updated springs.
    pub fn compute_and_readback(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        springs: &[GpuSpring],
        time: f32,
    ) -> Vec<GpuSpring> {
        if springs.is_empty() {
            return Vec::new();
        }

        let springs_bytes = bytemuck::cast_slice(springs);
        let storage_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MuscleStorageBuffer"),
            contents: springs_bytes,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });

        let time_uniform = TimeUniform { t: time };
        let time_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MuscleTimeBuffer"),
            contents: bytemuck::cast_slice(&[time_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MuscleBindGroup"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: storage_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: time_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("MuscleComputeEncoder"),
        });

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("MuscleComputePass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);

            let workgroup_count = ((springs.len() as f32) / 64.0).ceil() as u32;
            cpass.dispatch_workgroups(workgroup_count, 1, 1);
        }

        // Create staging buffer to read back
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MuscleStagingBuffer"),
            size: springs_bytes.len() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        encoder.copy_buffer_to_buffer(
            &storage_buffer,
            0,
            &staging_buffer,
            0,
            springs_bytes.len() as wgpu::BufferAddress,
        );

        queue.submit(Some(encoder.finish()));

        // Synchronous readback (blocking)
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

        device.poll(wgpu::Maintain::Wait);

        if receiver.recv().unwrap().is_ok() {
            let data = buffer_slice.get_mapped_range();
            let result: Vec<GpuSpring> = bytemuck::cast_slice(&data).to_vec();
            drop(data);
            staging_buffer.unmap();
            result
        } else {
            panic!("failed to map muscle staging buffer");
        }
    }
}
