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
    active_node_count: u32,
    active_spring_count: u32,
}

/// Broad-phase spatial grid dimensions for the steric-hindrance repulsion
/// pass. MUST match `GRID_DIM`/`GRID_CELL_CAPACITY` in `physics.wgsl` — they
/// size a fixed pair of buffers (not grown with population, since the grid
/// covers a fixed world area regardless of how many organisms are in it).
const GRID_DIM: u32 = 128;
const GRID_CELL_CAPACITY: u32 = 64;

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
    bin_nodes_pipeline: wgpu::ComputePipeline,
    integrate_pipeline: wgpu::ComputePipeline,
    pbd_projection_pipeline: wgpu::ComputePipeline,
    apply_pbd_pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,

    // Fixed-size spatial-grid buffers for the repulsion broad-phase —
    // allocated once (grid covers a fixed world area, independent of
    // population) and never resized.
    cell_counts_buffer: wgpu::Buffer,
    cell_nodes_buffer: wgpu::Buffer,

    // Persistent, geometrically-grown GPU buffers — recreated only when the
    // population outgrows current capacity, instead of every single tick.
    // Buffers may be larger than the live node/spring count; the shader is
    // told the live counts via `PhysicsConfigUniform` and must never rely on
    // `arrayLength()` (which reflects capacity) for loop bounds.
    node_capacity: usize,
    spring_capacity: usize,
    nodes_buffer: Option<wgpu::Buffer>,
    springs_buffer: Option<wgpu::Buffer>,
    config_buffer: Option<wgpu::Buffer>,
    atomic_forces_x: Option<wgpu::Buffer>,
    atomic_forces_y: Option<wgpu::Buffer>,
    staging_buffer: Option<wgpu::Buffer>,
    bind_group: Option<wgpu::BindGroup>,
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
                // cell_counts (spatial grid, broad-phase repulsion)
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // cell_nodes (spatial grid, broad-phase repulsion)
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
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

        let bin_nodes_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("BinNodesPipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "bin_nodes",
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

        // Fixed-size spatial grid buffers — the grid covers a fixed world
        // area (see GRID_DIM/GRID_CELL_CAPACITY doc comment above) so these
        // never need to grow with population, unlike the node/spring buffers.
        let cell_count = (GRID_DIM * GRID_DIM) as wgpu::BufferAddress;
        let cell_counts_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PhysicsCellCountsBuffer"),
            size: cell_count * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let cell_nodes_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PhysicsCellNodesBuffer"),
            size: cell_count * (GRID_CELL_CAPACITY as wgpu::BufferAddress) * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            muscle_actuation_pipeline,
            compute_forces_pipeline,
            bin_nodes_pipeline,
            integrate_pipeline,
            pbd_projection_pipeline,
            apply_pbd_pipeline,
            bind_group_layout,
            cell_counts_buffer,
            cell_nodes_buffer,
            node_capacity: 0,
            spring_capacity: 0,
            nodes_buffer: None,
            springs_buffer: None,
            config_buffer: None,
            atomic_forces_x: None,
            atomic_forces_y: None,
            staging_buffer: None,
            bind_group: None,
        }
    }

    /// # Physics Step Dispatch & Integration
    ///
    /// ## 1. What Happens
    /// `compute_step` marshals the CPU-side `GpuParticleNode` and `GpuPhysicsSpring` arrays
    /// into GPU Storage Buffers, dispatches the compute workloads, and performs a **blocking**
    /// readback to the CPU. Kept for callers that need same-tick results (e.g. tests); the
    /// simulation's hot path uses [`Self::dispatch`] + [`Self::resolve`] instead, which lets the
    /// GPU work for tick N overlap with CPU work and get collected at the start of tick N+1
    /// instead of stalling the CPU immediately after submission.
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
    #[allow(dead_code)]
    pub fn compute_step(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        nodes: &[GpuParticleNode],
        springs: &[GpuPhysicsSpring],
        dt: f32,
        global_time: f32,
        query_set: Option<&wgpu::QuerySet>,
    ) -> Vec<GpuParticleNode> {
        let pending = self.dispatch(device, queue, nodes, springs, dt, global_time, query_set);
        self.resolve(device, pending)
    }

    /// Submits the physics compute passes and kicks off an asynchronous
    /// buffer readback, returning immediately without blocking on the GPU.
    ///
    /// Call [`Self::resolve`] on the returned [`PendingPhysicsReadback`] once
    /// the GPU has plausibly finished (the simulation loop does this at the
    /// start of the *next* tick) to collect the updated node data.
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        nodes: &[GpuParticleNode],
        springs: &[GpuPhysicsSpring],
        dt: f32,
        global_time: f32,
        query_set: Option<&wgpu::QuerySet>,
    ) -> PendingPhysicsReadback {
        if nodes.is_empty() || springs.is_empty() {
            if let Some(qs) = query_set {
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                encoder.write_timestamp(qs, 0);
                encoder.write_timestamp(qs, 1);
                queue.submit(Some(encoder.finish()));
            }
            return PendingPhysicsReadback::Ready(nodes.to_vec());
        }

        self.ensure_capacity(device, nodes.len(), springs.len());

        let nodes_buffer = self.nodes_buffer.as_ref().unwrap();
        let springs_buffer = self.springs_buffer.as_ref().unwrap();
        let config_buffer = self.config_buffer.as_ref().unwrap();
        let atomic_forces_x = self.atomic_forces_x.as_ref().unwrap();
        let atomic_forces_y = self.atomic_forces_y.as_ref().unwrap();
        let staging_buffer = self.staging_buffer.as_ref().unwrap();
        let bind_group = self.bind_group.as_ref().unwrap();

        let nodes_bytes = bytemuck::cast_slice(nodes);
        let springs_bytes = bytemuck::cast_slice(springs);
        queue.write_buffer(nodes_buffer, 0, nodes_bytes);
        queue.write_buffer(springs_buffer, 0, springs_bytes);

        let config = PhysicsConfigUniform {
            dt,
            time: global_time,
            active_node_count: nodes.len() as u32,
            active_spring_count: springs.len() as u32,
        };
        queue.write_buffer(config_buffer, 0, bytemuck::cast_slice(&[config]));

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("PhysicsEncoder"),
        });

        // Zero out atomics (whole capacity — cheap GPU-side clear, and any
        // stale tail values beyond the live count are never read since the
        // shader gates on `config.active_node_count`).
        encoder.clear_buffer(atomic_forces_x, 0, None);
        encoder.clear_buffer(atomic_forces_y, 0, None);
        // Zero the spatial grid's per-cell counters before rebinning below.
        encoder.clear_buffer(&self.cell_counts_buffer, 0, None);

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
            cpass.set_bind_group(0, bind_group, &[]);
            cpass.dispatch_workgroups(spring_workgroups, 1, 1);
        }

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("ComputeForcesPass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.compute_forces_pipeline);
            cpass.set_bind_group(0, bind_group, &[]);
            cpass.dispatch_workgroups(spring_workgroups, 1, 1);
        }

        {
            // Bin nodes into the spatial grid so `integrate`'s repulsion pass
            // can query a bounded 3x3-cell neighborhood instead of scanning
            // every node in the simulation (was the dominant O(N^2) cost at
            // high entity counts).
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("BinNodesPass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.bin_nodes_pipeline);
            cpass.set_bind_group(0, bind_group, &[]);
            cpass.dispatch_workgroups(node_workgroups, 1, 1);
        }

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("IntegratePass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.integrate_pipeline);
            cpass.set_bind_group(0, bind_group, &[]);
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
                cpass.set_bind_group(0, bind_group, &[]);
                cpass.dispatch_workgroups(spring_workgroups, 1, 1);
            }
            {
                let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("ApplyPbdPass"),
                    timestamp_writes: None,
                });
                cpass.set_pipeline(&self.apply_pbd_pipeline);
                cpass.set_bind_group(0, bind_group, &[]);
                cpass.dispatch_workgroups(node_workgroups, 1, 1);
            }
        }

        if let Some(qs) = query_set {
            encoder.write_timestamp(qs, 1);
        }

        let byte_len = nodes_bytes.len();
        encoder.copy_buffer_to_buffer(
            nodes_buffer,
            0,
            staging_buffer,
            0,
            byte_len as wgpu::BufferAddress,
        );

        queue.submit(Some(encoder.finish()));

        let (sender, receiver) = std::sync::mpsc::channel();
        staging_buffer
            .slice(..byte_len as wgpu::BufferAddress)
            .map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

        PendingPhysicsReadback::Mapping { receiver, byte_len }
    }

    /// Grows (never shrinks) the persistent GPU buffers/bind group to hold
    /// at least `node_count`/`spring_count` entries, doubling capacity each
    /// time to amortize reallocation across many ticks of gradual population
    /// growth rather than reallocating on every single birth/death.
    fn ensure_capacity(&mut self, device: &wgpu::Device, node_count: usize, spring_count: usize) {
        let needs_growth = node_count > self.node_capacity || spring_count > self.spring_capacity;
        if !needs_growth && self.bind_group.is_some() {
            return;
        }

        if needs_growth {
            self.node_capacity = node_count.max(self.node_capacity * 2).max(256);
            self.spring_capacity = spring_count.max(self.spring_capacity * 2).max(256);
        }

        let node_bytes =
            (self.node_capacity * std::mem::size_of::<GpuParticleNode>()) as wgpu::BufferAddress;
        let spring_bytes =
            (self.spring_capacity * std::mem::size_of::<GpuPhysicsSpring>()) as wgpu::BufferAddress;

        self.nodes_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PhysicsNodesBuffer"),
            size: node_bytes,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        self.springs_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PhysicsSpringsBuffer"),
            size: spring_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        self.atomic_forces_x = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("AtomicForcesX"),
            size: (self.node_capacity * 4) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        self.atomic_forces_y = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("AtomicForcesY"),
            size: (self.node_capacity * 4) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        self.staging_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PhysicsStagingBuffer"),
            size: node_bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        if self.config_buffer.is_none() {
            self.config_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("PhysicsConfigBuffer"),
                size: std::mem::size_of::<PhysicsConfigUniform>() as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PhysicsBindGroup"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.nodes_buffer.as_ref().unwrap().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.springs_buffer.as_ref().unwrap().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.config_buffer.as_ref().unwrap().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.atomic_forces_x.as_ref().unwrap().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.atomic_forces_y.as_ref().unwrap().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.cell_counts_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.cell_nodes_buffer.as_entire_binding(),
                },
            ],
        }));
    }

    /// Blocks (briefly — see [`Self::dispatch`]) until the GPU work behind a
    /// [`PendingPhysicsReadback`] is complete, then returns the updated nodes.
    pub fn resolve(
        &self,
        device: &wgpu::Device,
        pending: PendingPhysicsReadback,
    ) -> Vec<GpuParticleNode> {
        match pending {
            PendingPhysicsReadback::Ready(nodes) => nodes,
            PendingPhysicsReadback::Mapping { receiver, byte_len } => {
                device.poll(wgpu::Maintain::Wait);
                if receiver.recv().unwrap().is_ok() {
                    let staging_buffer = self
                        .staging_buffer
                        .as_ref()
                        .expect("staging buffer must exist after a Mapping dispatch");
                    let data = staging_buffer
                        .slice(..byte_len as wgpu::BufferAddress)
                        .get_mapped_range();
                    let result: Vec<GpuParticleNode> = bytemuck::cast_slice(&data).to_vec();
                    drop(data);
                    staging_buffer.unmap();
                    result
                } else {
                    panic!("failed to map physics staging buffer");
                }
            }
        }
    }
}

/// A physics readback that's either already resolved (empty-input fast path)
/// or in flight on the GPU, to be collected later via
/// [`PhysicsComputePipeline::resolve`].
pub enum PendingPhysicsReadback {
    /// No GPU work was dispatched; the result is the (unchanged) input.
    Ready(Vec<GpuParticleNode>),
    /// A staging buffer is being asynchronously mapped for read access;
    /// `byte_len` is how many bytes (from the start) are valid to read.
    Mapping {
        /// Fires once `map_async` completes.
        receiver: std::sync::mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
        /// Number of valid bytes at the start of the pipeline's staging buffer.
        byte_len: usize,
    },
}
