use crate::app::PhylonApp;
use crate::systems::*;
use bevy_ecs::system::RunSystemOnce;

impl PhylonApp {
    /// # Discrete Biological Update Loop
    ///
    /// ## 1. What Happens
    /// The `update_simulation` method advances the entire biological, ecological, and physical
    /// state of the world by exactly one discrete timestep (`DT`, the configured
    /// [`common::TickRate`]). It orchestrates the strict ordering of sensing, brain evaluation,
    /// behavior, physics integration, and spatial diffusion.
    ///
    /// ## 2. Why It Happens
    /// Strict deterministic execution ordering is ecologically critical. If physics ran before
    /// sensing, an organism's brain would evaluate stale spatial data. Furthermore, performance
    /// dictates that heavy tensor operations (like CTRNN brain evaluation) must be batched and
    /// uploaded to the GPU compute shaders rather than evaluated linearly on the CPU.
    ///
    /// ## 3. How It Happens
    /// The ECS `World` executes systems sequentially:
    /// 1. **GPU readback resolution**: physics/brain results dispatched *last* tick are collected
    ///    and written into the ECS first, so this tick's systems see them as their starting state.
    /// 2. **Neural plasticity**: neuromodulator channels update from this tick's metabolic state,
    ///    then Hebbian weight adaptation (and, periodically, pruning) runs against the CTRNN node
    ///    states just resolved in step 1 — see `organisms::hebbian_plasticity_system`.
    /// 3. **Biology**: Organism growth and sensory data gathering.
    /// 4. **Neural Compute**: Batched ECS `Brain` data is mapped to `GpuCtrnnNode` buffers and
    ///    dispatched to the GPU for numerical integration via Euler's method — asynchronously;
    ///    the result is collected at the start of the *next* tick (see step 1):
    ///    $$ y_{i}(t + DT) = y_{i}(t) + \frac{DT}{\tau_i} \left( -y_i + \sum_{j} w_{ji} \sigma(y_j + \theta_j) + I_i \right) $$
    /// 5. **Behavior & Physics**: Node forces are accumulated and integrated into velocity/position
    ///    vectors, dispatched asynchronously the same way as brain compute.
    /// 6. **Spatial Dynamics**: Pheromones and gases diffuse across the `texture_2d_array`.
    ///
    /// Dispatching brain/physics asynchronously means their GPU work for tick N overlaps with
    /// tick N's CPU-side ECS systems instead of stalling the CPU immediately after submission;
    /// the tradeoff is a one-tick lag between when a value is computed and when dependent systems
    /// observe it (e.g. `behavior_system` acts on brain state that's one tick behind the neural
    /// integration dispatched this same tick) — at the default 60 Hz `tick_rate` this is ~16ms.
    pub(crate) fn update_simulation(&mut self) -> (f64, f64) {
        let mut physics_duration_ms = 0.0;
        let mut diffusion_duration_ms = 0.0;
        let dt = self.world.ecs.resource::<common::TickRate>().dt();
        self.total_sim_time += dt;

        // 0. Resolve GPU work dispatched last tick before anything reads
        // positions/brain state this tick.
        self.resolve_pending_physics();
        self.resolve_pending_brain();

        // 0.5. Neural plasticity: update neuromodulator channels from this
        // tick's metabolic state, then apply Hebbian weight adaptation (and,
        // periodically, pruning) using the node states just resolved above —
        // see `organisms::hebbian_plasticity_system`'s doc comment for why
        // this must run right after the brain readback, not before it.
        self.world
            .ecs
            .run_system_once(organisms::neuromodulator_system);
        self.world
            .ecs
            .run_system_once(organisms::hebbian_plasticity_system);

        // 1. Run Biology Systems (Sensing, Brain, Behavior)
        self.world.ecs.run_system_once(organisms::growth_system);
        // Life-stage transitions (Phase 4, P4-L1): only affects organisms
        // not currently mid-growth, so this can safely run right after
        // `growth_system` in the same tick without racing it — see
        // `organisms::life_stage_system`'s doc comment.
        self.world.ecs.run_system_once(organisms::life_stage_system);
        // Rebuilds the shared food/mineral/corpse spatial grids once for
        // this tick — must run before both sensing_system and
        // ecology::foraging_system, which otherwise would each rebuild the
        // same 3 grids from the same data independently.
        self.world
            .ecs
            .run_system_once(ecology::build_resource_grids_system);
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

        if let (Some(gpu), Some(brain_compute)) = (self.gpu.as_ref(), self.brain_compute.as_mut()) {
            let pending = brain_compute.dispatch(
                &gpu.device,
                &gpu.queue,
                &gpu_brain_nodes,
                &gpu_brain_synapses,
                dt,
            );
            self.pending_brain = Some((pending, brain_offsets));
        }

        self.world.ecs.run_system_once(behavior::behavior_system);
        self.world
            .ecs
            .run_system_once(behavior::physiological_state_update_system);
        // Runs right after physiological state so a pack-adopted Hunting
        // state isn't a full tick stale relative to the Fleeing/Foraging
        // states it must not override — see `pack_hunting_system`'s doc
        // comment.
        self.world
            .ecs
            .run_system_once(organisms::pack_hunting_system);

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
                organism_id: node.organism_id,
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

        // 4. Dispatch GPU Physics (async — collected at the start of next tick)
        if let (Some(gpu), Some(physics_compute)) =
            (self.gpu.as_ref(), self.physics_compute.as_mut())
        {
            let dispatch_start = std::time::Instant::now();
            let global_time = self
                .world
                .ecs
                .resource::<diffusion::DiffusionConfig>()
                .global_time;
            let pending = physics_compute.dispatch(
                &gpu.device,
                &gpu.queue,
                &gpu_nodes,
                &gpu_springs,
                dt,
                global_time,
                gpu.query_set.as_ref(),
            );
            physics_duration_ms += dispatch_start.elapsed().as_secs_f64() * 1000.0;
            self.pending_physics = Some((pending, node_entities));
        }

        // 5. Run remaining biological systems
        self.world
            .ecs
            .run_system_once(metabolism::day_night_cycle_system);
        self.world
            .ecs
            .run_system_once(metabolism::atmosphere_homeostasis_system);
        self.world.ecs.run_system_once(ecology::food_spawner_system);
        self.world
            .ecs
            .run_system_once(ecology::photosynthesis_system);
        self.world
            .ecs
            .run_system_once(organisms::systems::producer_growth_system);
        self.world.ecs.run_system_once(ecology::foraging_system);
        self.world.ecs.run_system_once(ecology::corpse_decay_system);
        self.world
            .ecs
            .run_system_once(ecology::fungal_network_system);
        self.world.ecs.run_system_once(organisms::flocking_system);
        self.world.ecs.run_system_once(organisms::biofilm_system);
        // Progression before spread: this tick's incubation/recovery rolls
        // and ATP drain happen first, then a freshly-Infectious organism
        // can transmit starting next tick — not the same tick it converted.
        self.world
            .ecs
            .run_system_once(ecology::disease_progression_system);
        self.world
            .ecs
            .run_system_once(ecology::disease_spread_system);
        // Per-segment immune response (Phase 4, P4-F5): spreads the
        // organism-wide `Infection` progressed just above out into each
        // segment's own severity/resistance state.
        self.world
            .ecs
            .run_system_once(organisms::segment_infection_system);
        // Intra-body transport (Phase 4, P4-F3) right before metabolism, so
        // resources gained this tick (foraging/photosynthesis, above) can
        // reach a segment's local pool before `metabolism_system` respires
        // from it — see `organisms::transport_system`'s doc comment.
        self.world.ecs.run_system_once(organisms::transport_system);
        // Endocrine diffusion (Phase 4, P4-F4): propagates the head's
        // `Neuromodulators` reading (last updated this tick at step 0.5,
        // above) out to every segment's own `HormoneLevel` along the same
        // Body Graph edges transport just used.
        self.world
            .ecs
            .run_system_once(organisms::endocrine_diffusion_system);
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
        // Phase 4, P4-E1: the first real `events::PhylonEvent` consumer,
        // plus expiring this tick's timed effects — both must run after
        // `process_deaths_system` (this tick's producer, above).
        self.world.ecs.run_system_once(interaction_event_log_system);
        self.world.ecs.run_system_once(expire_timed_effects_system);
        self.world
            .ecs
            .run_system_once(crate::analytics_bridge::analytics_bridge_system);
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
        if let Some(mut phylon_events) = self
            .world
            .ecs
            .get_resource_mut::<bevy_ecs::event::Events<events::PhylonEvent>>()
        {
            phylon_events.update();
        }

        if let (Some(gpu), Some(diffusion_compute)) =
            (self.gpu.as_ref(), self.diffusion_compute.as_mut())
        {
            // 5. Gather diffusion emitters and run compute
            let (diff_rate, dec_rate) = {
                let mut diffusion_config =
                    self.world.ecs.resource_mut::<diffusion::DiffusionConfig>();

                // Diurnal modulation
                diffusion_config.global_time += dt;
                // Oscillate decay rate between 0.5x and 1.5x of base
                let diurnal_mod = 1.0 + 0.5 * (diffusion_config.global_time * 0.1).sin();
                diffusion_config.decay_rate = diffusion_config.base_decay_rate * diurnal_mod;

                (diffusion_config.diffusion_rate, diffusion_config.decay_rate)
            };
            let mut gpu_emitters = Vec::new();

            // We use a fixed logical bound instead of screen width, so scaling works correctly
            let bounds_extents = 1500.0;
            let to_grid = |pos: common::Vec2, radius: f32| -> (f32, f32, f32) {
                let grid_x = (pos.x / bounds_extents) * 128.0 + 128.0;
                let grid_y = (-pos.y / bounds_extents) * 128.0 + 128.0;
                let grid_radius = (radius / bounds_extents) * 128.0;
                (grid_x, grid_y, grid_radius)
            };

            // Layer 0: Pheromones
            let pheromones_offset = gpu_emitters.len() as u32;
            let mut query_signals = self
                .world
                .ecs
                .query::<(&physics::ParticleNode, &diffusion::SignalEmitter)>();
            for (node, signal) in query_signals.iter(&self.world.ecs) {
                let (gx, gy, gr) = to_grid(node.position, signal.radius);
                gpu_emitters.push(gpu::diffusion_pipeline::GpuEmitter {
                    grid_pos: [gx, gy],
                    value: signal.value,
                    grid_radius: gr,
                });
            }
            let pheromones_count = gpu_emitters.len() as u32 - pheromones_offset;

            // Layer 1: Energy
            let energy_offset = gpu_emitters.len() as u32;
            let mut query_emitters = self.world.ecs.query::<&diffusion::Emitter>();
            for emitter in query_emitters.iter(&self.world.ecs) {
                if emitter.layer == diffusion::FieldLayer::Energy {
                    let (gx, gy, gr) = to_grid(emitter.position, emitter.radius);
                    gpu_emitters.push(gpu::diffusion_pipeline::GpuEmitter {
                        grid_pos: [gx, gy],
                        value: emitter.value,
                        grid_radius: gr,
                    });
                }
            }
            let energy_count = gpu_emitters.len() as u32 - energy_offset;

            // Layer 2: O2
            let o2_offset = gpu_emitters.len() as u32;
            let mut query_chem = self.world.ecs.query::<(
                &physics::ParticleNode,
                &metabolism::ChemicalEconomy,
                &metabolism::Metabolism,
            )>();
            for (node, _chem, meta) in query_chem.iter(&self.world.ecs) {
                let (gx, gy, gr) = to_grid(node.position, 15.0);
                // Simplistic baseline: plants emit O2, animals consume it.
                let net_o2 = if meta.is_plant { 1.5 } else { -1.0 };
                gpu_emitters.push(gpu::diffusion_pipeline::GpuEmitter {
                    grid_pos: [gx, gy],
                    value: net_o2 * meta.mass,
                    grid_radius: gr,
                });
            }
            let o2_count = gpu_emitters.len() as u32 - o2_offset;

            // Layer 3: CO2
            let co2_offset = gpu_emitters.len() as u32;
            for (node, _chem, meta) in query_chem.iter(&self.world.ecs) {
                let (gx, gy, gr) = to_grid(node.position, 15.0);
                // Simplistic baseline: plants consume CO2, animals emit it.
                let net_co2 = if meta.is_plant { -1.5 } else { 1.0 };
                gpu_emitters.push(gpu::diffusion_pipeline::GpuEmitter {
                    grid_pos: [gx, gy],
                    value: net_co2 * meta.mass,
                    grid_radius: gr,
                });
            }
            // Corpses emit CO2
            let mut query_dead = self.world.ecs.query_filtered::<(&physics::ParticleNode, &metabolism::Metabolism), bevy_ecs::query::With<metabolism::Dead>>();
            for (node, meta) in query_dead.iter(&self.world.ecs) {
                let (gx, gy, gr) = to_grid(node.position, 20.0);
                gpu_emitters.push(gpu::diffusion_pipeline::GpuEmitter {
                    grid_pos: [gx, gy],
                    value: 2.0 * meta.mass, // Corpses release a lot of CO2 as they decay
                    grid_radius: gr,
                });
            }
            let co2_count = gpu_emitters.len() as u32 - co2_offset;

            let layer_configs = [
                gpu::diffusion_pipeline::LayerConfig {
                    diffusion_rate: diff_rate,
                    decay_rate: dec_rate,
                    emitter_count: pheromones_count,
                    emitter_offset: pheromones_offset,
                },
                gpu::diffusion_pipeline::LayerConfig {
                    diffusion_rate: 0.05,
                    decay_rate: dec_rate * 0.5,
                    emitter_count: energy_count,
                    emitter_offset: energy_offset,
                },
                gpu::diffusion_pipeline::LayerConfig {
                    diffusion_rate: 0.8,
                    decay_rate: 0.005,
                    emitter_count: o2_count,
                    emitter_offset: o2_offset,
                },
                gpu::diffusion_pipeline::LayerConfig {
                    diffusion_rate: 0.8,
                    decay_rate: 0.005,
                    emitter_count: co2_count,
                    emitter_offset: co2_offset,
                },
            ];

            let diffusion_start = std::time::Instant::now();
            diffusion_compute.step(
                &gpu.device,
                &gpu.queue,
                gpu::diffusion_pipeline::DiffusionUniforms {
                    dt,
                    _pad1: 0,
                    _pad2: 0,
                    _pad3: 0,
                    layers: layer_configs,
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

    /// Collects the physics GPU readback dispatched last tick (if any) and
    /// writes the updated positions/velocities into the ECS. A brief
    /// `device.poll(Wait)` is still required to actually collect the mapped
    /// buffer, but because a full tick's worth of CPU work has run since the
    /// dispatch, the GPU has almost always already finished — unlike the
    /// previous same-tick blocking readback, which stalled immediately.
    fn resolve_pending_physics(&mut self) {
        let Some((pending, node_entities)) = self.pending_physics.take() else {
            return;
        };
        let Some(gpu) = self.gpu.as_ref() else {
            return;
        };
        let Some(physics_compute) = self.physics_compute.as_ref() else {
            return;
        };
        let updated_nodes = physics_compute.resolve(&gpu.device, pending);
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

    /// Collects the brain (CTRNN) GPU readback dispatched last tick (if any)
    /// and writes the integrated node states into the ECS. Same non-blocking
    /// rationale as [`Self::resolve_pending_physics`].
    fn resolve_pending_brain(&mut self) {
        let Some((pending, brain_offsets)) = self.pending_brain.take() else {
            return;
        };
        let Some(gpu) = self.gpu.as_ref() else {
            return;
        };
        let Some(brain_compute) = self.brain_compute.as_ref() else {
            return;
        };
        let gpu_brain_nodes = brain_compute.resolve(&gpu.device, pending);
        let mut query = self.world.ecs.query::<&mut brain::Brain>();
        for (entity, start_node, len) in brain_offsets {
            if let Ok(mut brain) = query.get_mut(&mut self.world.ecs, entity) {
                for i in 0..len {
                    brain.nodes[i].state = gpu_brain_nodes[(start_node as usize) + i].state;
                }
            }
        }
    }
}
