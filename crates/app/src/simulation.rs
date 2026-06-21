use crate::app::PhylonApp;
use crate::systems::*;
use bevy_ecs::system::RunSystemOnce;

const DT: f32 = 0.016; // Fixed 60 Hz timestep

impl PhylonApp {
    /// Advances the simulation by exactly one tick (DT).
    pub(crate) fn update_simulation(&mut self) -> (f64, f64) {
        let mut physics_duration_ms = 0.0;
        let mut diffusion_duration_ms = 0.0;
        self.total_sim_time += DT;

        // 1. Run Biology Systems (Sensing, Brain, Behavior)
        self.world.ecs.run_system_once(organisms::growth_system);
        self.world.ecs.run_system_once(sensing::sensing_system);

        // -- GPU CTRNN EVALUATION --
        let mut gpu_brain_nodes = Vec::new();
        let mut gpu_brain_synapses = Vec::new();
        let mut brain_offsets = Vec::new();

        let mut query = self.world.ecs.query::<(
            bevy_ecs::entity::Entity,
            &sensing::SensoryState,
            &mut brain::Brain,
        )>();
        for (entity, sensory, mut brain) in query.iter_mut(&mut self.world.ecs) {
            brain.set_inputs(&sensory.inputs);

            let start_node = gpu_brain_nodes.len() as u32;
            let start_syn = gpu_brain_synapses.len() as u32;

            for node in &brain.nodes {
                gpu_brain_nodes.push(gpu::brain_pipeline::GpuCtrnnNode {
                    state: node.state,
                    time_constant: node.time_constant,
                    bias: node.bias,
                    activation: node.activation,
                    first_synapse: start_syn + node.first_synapse,
                    synapse_count: node.synapse_count,
                });
            }

            for syn in &brain.synapses {
                gpu_brain_synapses.push(gpu::brain_pipeline::GpuCtrnnSynapse {
                    source: start_node + syn.source,
                    target: start_node + syn.target,
                    weight: syn.weight,
                    _padding: 0,
                });
            }

            brain_offsets.push((entity, start_node, brain.nodes.len()));
        }

        if let Some(gpu) = self.gpu.as_ref() {
            if let Some(brain_compute) = self.brain_compute.as_ref() {
                brain_compute.compute_step(
                    &gpu.device,
                    &gpu.queue,
                    &mut gpu_brain_nodes,
                    &gpu_brain_synapses,
                    DT,
                );
            }
        }

        // Readback integrated node state
        let mut query = self.world.ecs.query::<&mut brain::Brain>();
        for (entity, start_node, len) in brain_offsets {
            if let Ok(mut brain) = query.get_mut(&mut self.world.ecs, entity) {
                for i in 0..len {
                    brain.nodes[i].state = gpu_brain_nodes[(start_node as usize) + i].state;
                }
            }
        }

        self.world.ecs.run_system_once(behavior::behavior_system);

        // 2. Gather Nodes and build Entity -> Index map
        let mut entity_to_index = std::collections::HashMap::new();
        let mut gpu_nodes = Vec::new();
        let mut node_entities = Vec::new();

        let mut query_nodes = self
            .world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &physics::ParticleNode)>();
        for (entity, node) in query_nodes.iter(&self.world.ecs) {
            entity_to_index.insert(entity, gpu_nodes.len() as u32);
            gpu_nodes.push(gpu::physics_pipeline::GpuParticleNode {
                position: [node.position.x, node.position.y],
                velocity: [node.velocity.x, node.velocity.y],
                force: [node.force.x, node.force.y],
                mass: node.mass,
                _padding: 0,
            });
            node_entities.push(entity);
        }

        // 3. Gather Springs
        let mut query_springs = self.world.ecs.query::<&physics::Spring>();
        let mut gpu_springs = Vec::new();
        for spring in query_springs.iter(&self.world.ecs) {
            let Some(&idx_a) = entity_to_index.get(&spring.node_a) else {
                continue;
            };
            let Some(&idx_b) = entity_to_index.get(&spring.node_b) else {
                continue;
            };

            let constraint_type = match spring.constraint_type {
                physics::ConstraintType::Elastic => 0,
                physics::ConstraintType::Rigid => 1,
                physics::ConstraintType::Passive => 2,
                physics::ConstraintType::Rotational => 3,
            };

            gpu_springs.push(gpu::physics_pipeline::GpuPhysicsSpring {
                node_a: idx_a,
                node_b: idx_b,
                constraint_type,
                rest_length: spring.rest_length,
                base_length: spring.base_length,
                stiffness: spring.stiffness,
                damping: spring.damping,
                actuation_amplitude: spring.actuation_amplitude,
                actuation_phase: spring.actuation_phase,
                breaking_strain: spring.breaking_strain,
                is_fin: spring.is_fin,
                _padding_2: 0,
            });
        }

        // 4. Compute GPU Physics
        if let (Some(gpu), Some(physics_compute)) =
            (self.gpu.as_ref(), self.physics_compute.as_ref())
        {
            let physics_start = std::time::Instant::now();
            let global_time = self
                .world
                .ecs
                .resource::<diffusion::DiffusionConfig>()
                .global_time;
            let updated_nodes = physics_compute.compute_step(
                &gpu.device,
                &gpu.queue,
                &gpu_nodes,
                &gpu_springs,
                DT,
                global_time,
                gpu.query_set.as_ref(),
            );
            physics_duration_ms += physics_start.elapsed().as_secs_f64() * 1000.0;

            // 5. Update ECS Nodes
            for (i, entity) in node_entities.iter().enumerate() {
                if let Some(mut node) = self.world.ecs.get_mut::<physics::ParticleNode>(*entity) {
                    node.position.x = updated_nodes[i].position[0];
                    node.position.y = updated_nodes[i].position[1];
                    node.velocity.x = updated_nodes[i].velocity[0];
                    node.velocity.y = updated_nodes[i].velocity[1];
                    // Clear forces for next tick
                    node.force = common::Vec2::new(0.0, 0.0);
                }
            }
        }

        // 6. Run remaining biological systems
        self.world.ecs.run_system_once(ecology::food_spawner_system);
        self.world
            .ecs
            .run_system_once(ecology::photosynthesis_system);
        self.world
            .ecs
            .run_system_once(organisms::systems::producer_growth_system);
        self.world.ecs.run_system_once(ecology::foraging_system);
        self.world
            .ecs
            .run_system_once(metabolism::metabolism_system);
        self.world
            .ecs
            .run_system_once(reproduction::reproduction_system);
        self.world.ecs.run_system_once(process_births_system);
        self.world.ecs.run_system_once(process_deaths_system);
        self.world.ecs.run_system_once(ecology::catastrophe_system);
        self.world
            .ecs
            .run_system_once(process_narrative_events_system);
        if let Some(mut events) = self
            .world
            .ecs
            .get_resource_mut::<bevy_ecs::event::Events<reproduction::BirthRequest>>()
        {
            events.update();
        }
        if let Some(mut hazard_events) = self
            .world
            .ecs
            .get_resource_mut::<bevy_ecs::event::Events<ecology::catastrophe::HazardSpawned>>()
        {
            hazard_events.update();
        }

        if let (Some(gpu), Some(diffusion_compute)) =
            (self.gpu.as_ref(), self.diffusion_compute.as_mut())
        {
            // 5. Gather diffusion emitters and run compute
            let (diff_rate, dec_rate) = {
                let mut diffusion_config =
                    self.world.ecs.resource_mut::<diffusion::DiffusionConfig>();

                // Diurnal modulation
                diffusion_config.global_time += DT;
                // Oscillate decay rate between 0.5x and 1.5x of base
                let diurnal_mod = 1.0 + 0.5 * (diffusion_config.global_time * 0.1).sin();
                diffusion_config.decay_rate = diffusion_config.base_decay_rate * diurnal_mod;

                (diffusion_config.diffusion_rate, diffusion_config.decay_rate)
            };
            let mut query_emitters = self.world.ecs.query::<&diffusion::Emitter>();
            let mut gpu_emitters = Vec::new();

            let screen_w = gpu.config.as_ref().map(|c| c.width).unwrap_or(1280) as f32;
            let screen_h = gpu.config.as_ref().map(|c| c.height).unwrap_or(720) as f32;

            for emitter in query_emitters.iter(&self.world.ecs) {
                // Map world space to 256x256 grid space
                let grid_x = (emitter.position.x / (screen_w * 0.5)) * 128.0 + 128.0;
                let grid_y = (-emitter.position.y / (screen_h * 0.5)) * 128.0 + 128.0;
                let grid_radius = (emitter.radius / (screen_w * 0.5)) * 128.0;

                gpu_emitters.push(gpu::diffusion_pipeline::GpuEmitter {
                    grid_pos: [grid_x, grid_y],
                    value: emitter.value,
                    grid_radius,
                });
            }

            let diffusion_start = std::time::Instant::now();
            diffusion_compute.step(
                &gpu.device,
                &gpu.queue,
                gpu::diffusion_pipeline::DiffusionUniforms {
                    diffusion_rate: diff_rate,
                    decay_rate: dec_rate,
                    dt: DT, // fixed timestep
                    emitter_count: gpu_emitters.len() as u32,
                },
                &gpu_emitters,
                gpu.query_set.as_ref(),
            );

            if let Some(field_data) = diffusion_compute.try_read_field(&gpu.device) {
                let mut cpu_field = self.world.ecs.resource_mut::<diffusion::CpuFieldState>();
                cpu_field.data = field_data;
            }
            diffusion_duration_ms += diffusion_start.elapsed().as_secs_f64() * 1000.0;
        }

        let mut completed_records = Vec::new();
        if let Some(mut tracker) = self
            .world
            .ecs
            .get_resource_mut::<evolution::LineageTracker>()
        {
            completed_records = tracker.extract_completed_records();
        }
        if !completed_records.is_empty() {
            self.storage.flush_lineages(&completed_records);
        }

        (physics_duration_ms, diffusion_duration_ms)
    }
}
