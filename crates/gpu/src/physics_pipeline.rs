use wgpu::util::DeviceExt;

/// A GPU-friendly representation of a ParticleNode.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuParticleNode {
    /// Position of the node.
    pub position: [f32; 2],
    /// Velocity of the node.
    pub velocity: [f32; 2],
    /// Accumulated force on the node.
    pub force: [f32; 2],
    /// Mass of the node.
    pub mass: f32,
    /// ID of the organism this node belongs to.
    pub organism_id: u32,
}

/// A GPU-friendly representation of a Spring constraint.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuPhysicsSpring {
    /// Index of the first node.
    pub node_a: u32,
    /// Index of the second node.
    pub node_b: u32,
    /// Constraint type (0 = Elastic, 1 = Rigid, 2 = Passive).
    pub constraint_type: u32,
    /// Current rest length.
    pub rest_length: f32,
    /// Base rest length.
    pub base_length: f32,
    /// Stiffness (k).
    pub stiffness: f32,
    /// Damping.
    pub damping: f32,
    /// Actuation amplitude.
    pub actuation_amplitude: f32,
    /// Actuation phase.
    pub actuation_phase: f32,
    /// Strain before breaking.
    pub breaking_strain: f32,
    /// Is this a fin? (1 for yes, 0 for no)
    pub is_fin: u32,
    /// Padding.
    pub _padding_2: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct PhysicsConfigUniform {
    dt: f32,
    time: f32,
    _padding: [u32; 2],
}

/// # GPU Physics Compute Pipeline
///
/// ## 1. What Happens
/// The `PhysicsComputePipeline` wraps the WebGPU WGSL shaders responsible for resolving
/// the soft-body spring physics equations for all organisms simultaneously.
///
/// ## 2. Why It Happens
/// A simulation with 500 organisms, each having 10 nodes and 25 springs, requires
/// ~12,500 spring evaluations per tick. CPU-based physics engines (like Rapier) struggle
/// with thousands of soft-body constraints. Moving the $O(N)$ math to a GPU Compute
/// Shader allows the engine to run the Symplectic Euler and Position-Based Dynamics (PBD)
/// at 60 FPS without blocking the CPU.
///
/// ## 3. How It Happens
/// The pipeline manages 5 distinct compute passes:
/// 1. `MuscleActuation`: Modifies rest lengths based on Sine oscillators.
/// 2. `ComputeForces`: Calculates Hooke's Law for elastic springs.
/// 3. `Integrate`: Applies $F=MA$ and updates velocities (Symplectic Euler).
/// 4. `PbdProjection`: Solves distance constraints for rigid segments.
/// 5. `ApplyPbd`: Updates final positions.
pub struct PhysicsComputePipeline {
    muscle_actuation_pipeline: wgpu::ComputePipeline,
    compute_forces_pipeline: wgpu::ComputePipeline,
    integrate_pipeline: wgpu::ComputePipeline,
    pbd_projection_pipeline: wgpu::ComputePipeline,
    apply_pbd_pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl PhysicsComputePipeline {
    /// Compiles and initializes the physics compute shaders.
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PhysicsComputeShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("physics.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PhysicsBindGroupLayout"),
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
                // springs
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
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
                // atomic_forces_x
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // atomic_forces_y
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PhysicsComputePipelineLayout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let compute_forces_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("ComputeForcesPipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: "compute_forces",
                compilation_options: Default::default(),
                cache: None,
            });

        let muscle_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("MuscleActuationShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("muscle_actuation.wgsl").into()),
        });

        let muscle_actuation_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("MuscleActuationPipeline"),
                layout: Some(&pipeline_layout),
                module: &muscle_shader,
                entry_point: "main",
                compilation_options: Default::default(),
                cache: None,
            });

        let integrate_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("IntegratePipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "integrate",
            compilation_options: Default::default(),
            cache: None,
        });

        let pbd_projection_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("PbdProjectionPipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: "pbd_projection",
                compilation_options: Default::default(),
                cache: None,
            });

        let apply_pbd_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ApplyPbdPipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "apply_pbd",
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            muscle_actuation_pipeline,
            compute_forces_pipeline,
            integrate_pipeline,
            pbd_projection_pipeline,
            apply_pbd_pipeline,
            bind_group_layout,
        }
    }

    /// # Physics Step Dispatch & Integration
    ///
    /// ## 1. What Happens
    /// `compute_step` marshals the CPU-side `GpuParticleNode` and `GpuPhysicsSpring` arrays
    /// into GPU Storage Buffers, dispatches the compute workloads, and performs an asynchronous
    /// readback to the CPU.
    ///
    /// ## 2. Why It Happens
    /// The ECS needs the updated physics positions to render the organisms and calculate
    /// collision/foraging distance checks. While the GPU is incredibly fast at math, transferring
    /// data across the PCIe bus is slow. We pack all nodes into a flat $1D$ array to minimize
    /// buffer mapping overhead.
    ///
    /// ## 3. How It Happens
    /// To resolve rigid structural constraints without exploding the simulation, we use
    /// a Position-Based Dynamics (PBD) approach. The standard Hooke's Law integration runs first,
    /// followed by a 3-iteration Gauss-Seidel style projection loop:
    ///
    /// $$ \Delta \vec{p}_1 = \frac{w_1}{w_1 + w_2} (|\vec{p}_1 - \vec{p}_2| - d) \frac{\vec{p}_1 - \vec{p}_2}{|\vec{p}_1 - \vec{p}_2|} $$
    ///
    /// The final mapped buffer is cast back to a `Vec<GpuParticleNode>` using `bytemuck` and returned.
    #[allow(clippy::too_many_arguments)]
    pub fn compute_step(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        nodes: &[GpuParticleNode],
        springs: &[GpuPhysicsSpring],
        dt: f32,
        global_time: f32,
        query_set: Option<&wgpu::QuerySet>,
    ) -> Vec<GpuParticleNode> {
        if nodes.is_empty() || springs.is_empty() {
            if let Some(qs) = query_set {
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                encoder.write_timestamp(qs, 0);
                encoder.write_timestamp(qs, 1);
                queue.submit(Some(encoder.finish()));
            }
            return nodes.to_vec();
        }

        let nodes_bytes = bytemuck::cast_slice(nodes);
        let springs_bytes = bytemuck::cast_slice(springs);

        let nodes_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PhysicsNodesBuffer"),
            contents: nodes_bytes,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });

        let springs_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PhysicsSpringsBuffer"),
            contents: springs_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let config = PhysicsConfigUniform {
            dt,
            time: global_time,
            _padding: [0; 2],
        };
        let config_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PhysicsConfigBuffer"),
            contents: bytemuck::cast_slice(&[config]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let atomic_forces_x = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("AtomicForcesX"),
            size: (nodes.len() * 4) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let atomic_forces_y = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("AtomicForcesY"),
            size: (nodes.len() * 4) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PhysicsBindGroup"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: nodes_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: springs_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: config_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: atomic_forces_x.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: atomic_forces_y.as_entire_binding(),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("PhysicsEncoder"),
        });

        // Zero out atomics
        encoder.clear_buffer(&atomic_forces_x, 0, None);
        encoder.clear_buffer(&atomic_forces_y, 0, None);

        let node_workgroups = ((nodes.len() as f32) / 64.0).ceil() as u32;
        let spring_workgroups = ((springs.len() as f32) / 64.0).ceil() as u32;

        if let Some(qs) = query_set {
            encoder.write_timestamp(qs, 0);
        }

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("MuscleActuationPass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.muscle_actuation_pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(spring_workgroups, 1, 1);
        }

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("ComputeForcesPass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.compute_forces_pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(spring_workgroups, 1, 1);
        }

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("IntegratePass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.integrate_pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(node_workgroups, 1, 1);
        }

        // PBD loop (3 iterations)
        for _ in 0..3 {
            {
                let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("PbdProjectionPass"),
                    timestamp_writes: None,
                });
                cpass.set_pipeline(&self.pbd_projection_pipeline);
                cpass.set_bind_group(0, &bind_group, &[]);
                cpass.dispatch_workgroups(spring_workgroups, 1, 1);
            }
            {
                let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("ApplyPbdPass"),
                    timestamp_writes: None,
                });
                cpass.set_pipeline(&self.apply_pbd_pipeline);
                cpass.set_bind_group(0, &bind_group, &[]);
                cpass.dispatch_workgroups(node_workgroups, 1, 1);
            }
        }

        if let Some(qs) = query_set {
            encoder.write_timestamp(qs, 1);
        }

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PhysicsStagingBuffer"),
            size: nodes_bytes.len() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        encoder.copy_buffer_to_buffer(
            &nodes_buffer,
            0,
            &staging_buffer,
            0,
            nodes_bytes.len() as wgpu::BufferAddress,
        );

        queue.submit(Some(encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

        device.poll(wgpu::Maintain::Wait);

        if receiver.recv().unwrap().is_ok() {
            let data = buffer_slice.get_mapped_range();
            let result: Vec<GpuParticleNode> = bytemuck::cast_slice(&data).to_vec();
            drop(data);
            staging_buffer.unmap();
            result
        } else {
            panic!("failed to map physics staging buffer");
        }
    }
}
