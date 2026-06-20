//! # Phylon Application
//!
//! The main binary entry point for the Phylon simulation.
//!
//! ## Responsibilities
//!
//! 1. Parse CLI arguments and locate the config file.
//! 2. Initialise structured logging via `tracing_subscriber`.
//! 3. Load [`PhylonConfig`] from `data/default.ron` (falls back to defaults).
//! 4. Create a `winit` [`EventLoop`] and application window.
//! 5. Initialise a `wgpu` surface on the window.
//! 6. Create a [`SimulationScheduler`].
//! 7. Run the event loop — advancing the scheduler on each `AboutToWait` and
//!    presenting a cleared frame on each `RedrawRequested`.
//!
//! ## Architecture note
//!
//! The `app` crate is the **composition root** — the only crate permitted to
//! depend on everything. All other crates are decoupled from each other via
//! the dependency rules in `docs/02_crate_dependency_graph.md`.

use anyhow::Result;

use crate::app::PhylonApp;

use crate::systems::*;

impl PhylonApp {
    /// Advances the simulation and renders one frame.
    pub(crate) fn render(&mut self) -> Result<()> {
        let Some(gpu) = self.gpu.as_ref() else {
            return Ok(());
        };
        let Some(physics_compute) = self.physics_compute.as_ref() else {
            return Ok(());
        };

        const DT: f32 = 0.016; // Fixed 60 Hz timestep

        // 1. Camera Tracking
        if let Some(tracked) = self.tracked_entity {
            if let Ok(node) = self
                .world
                .ecs
                .query::<&physics::ParticleNode>()
                .get(&self.world.ecs, tracked)
            {
                // Smoothly follow the target
                self.camera_pos = self.camera_pos.lerp(node.position, 0.1);
            } else {
                // Entity no longer exists (e.g. died), drop tracking
                self.tracked_entity = None;
            }
        }

        if self.total_sim_time > 1.0 && !self.is_paused {
            println!("SIMULATING PAUSE");
            self.is_paused = true;
        }

        // Only step simulation if we're in the simulation state and not paused
        if self.app_state == ui::AppState::Simulation && !self.is_paused {
            self.accumulated_time += self.simulation_speed;
        }

        let mut physics_duration_ms = 0.0;
        let mut diffusion_duration_ms = 0.0;

        let ticks_this_frame = self.accumulated_time.floor() as u32;
        let ticks_to_run = ticks_this_frame.min(self.max_ticks_per_frame);
        self.accumulated_time -= ticks_this_frame as f32;

        for _ in 0..ticks_to_run {
            self.total_sim_time += DT;

            // 2. Run Biology Systems (Sensing, Brain, Behavior)
            use bevy_ecs::system::RunSystemOnce;
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
                0.016,
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

            // 6. Run remaining biological systems
            self.world.ecs.run_system_once(ecology::food_spawner_system);
            self.world.ecs.run_system_once(ecology::foraging_system);
            self.world
                .ecs
                .run_system_once(metabolism::metabolism_system);
            self.world
                .ecs
                .run_system_once(reproduction::reproduction_system);
            self.world.ecs.run_system_once(process_births_system);
            self.world.ecs.run_system_once(process_deaths_system);
            if let Some(mut events) = self
                .world
                .ecs
                .get_resource_mut::<bevy_ecs::event::Events<reproduction::BirthRequest>>()
            {
                events.update();
            }

            if let Some(diffusion_compute) = self.diffusion_compute.as_mut() {
                // 5. Gather diffusion emitters and run compute
                let (diff_rate, dec_rate) = {
                    let mut diffusion_config =
                        self.world.ecs.resource_mut::<diffusion::DiffusionConfig>();

                    // Diurnal modulation
                    diffusion_config.global_time += 0.016;
                    // Oscillate decay rate between 0.5x and 1.5x of base
                    let diurnal_mod = 1.0 + 0.5 * (diffusion_config.global_time * 0.1).sin();
                    diffusion_config.decay_rate = diffusion_config.base_decay_rate * diurnal_mod;

                    (diffusion_config.diffusion_rate, diffusion_config.decay_rate)
                };
                let mut query_emitters = self.world.ecs.query::<&diffusion::Emitter>();
                let mut gpu_emitters = Vec::new();

                let screen_w = gpu.config.width as f32;
                let screen_h = gpu.config.height as f32;

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
                        dt: 0.016, // fixed timestep
                        emitter_count: gpu_emitters.len() as u32,
                    },
                    &gpu_emitters,
                    gpu.query_set.as_ref(),
                );

                if let (Some(qs), Some(rb), Some(readback)) =
                    (&gpu.query_set, &gpu.resolve_buffer, &gpu.readback_buffer)
                {
                    let mut encoder =
                        gpu.device
                            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: Some("Timestamps"),
                            });
                    encoder.resolve_query_set(qs, 0..4, rb, 0);
                    encoder.copy_buffer_to_buffer(rb, 0, readback, 0, 32);
                    gpu.queue.submit(Some(encoder.finish()));

                    let slice = readback.slice(..);
                    slice.map_async(wgpu::MapMode::Read, |_| {});
                    gpu.device.poll(wgpu::Maintain::Wait);

                    let data = slice.get_mapped_range();
                    let byte_slice = data.as_ref();
                    let mut timestamps = [0u64; 4];
                    for i in 0..4 {
                        let mut bytes = [0u8; 8];
                        bytes.copy_from_slice(&byte_slice[i * 8..(i + 1) * 8]);
                        timestamps[i] = u64::from_ne_bytes(bytes);
                    }
                    let period = gpu.queue.get_timestamp_period();

                    if timestamps[1] > timestamps[0] {
                        physics_duration_ms =
                            (timestamps[1] - timestamps[0]) as f64 * period as f64 / 1_000_000.0;
                    }
                    if timestamps[3] > timestamps[2] {
                        diffusion_duration_ms =
                            (timestamps[3] - timestamps[2]) as f64 * period as f64 / 1_000_000.0;
                    }

                    drop(data);
                    readback.unmap();
                }

                if let Some(field_data) = diffusion_compute.try_read_field(&gpu.device) {
                    let mut cpu_field = self.world.ecs.resource_mut::<diffusion::CpuFieldState>();
                    cpu_field.data = field_data;
                }
                diffusion_duration_ms += diffusion_start.elapsed().as_secs_f64() * 1000.0;
            }
        }

        // Record analytics — read entity_count before mutably borrowing the resource
        let entity_count = self.world.ecs.entities().len() as usize;
        if let Some(mut metrics) = self.world.ecs.get_resource_mut::<analytics::MetricsState>() {
            let sim_dt = (ticks_to_run as f64) * f64::from(DT);
            let real_dt = f64::from(DT); // Fixed render step for now
            metrics.record_frame(entity_count, sim_dt, real_dt);

            if ticks_to_run > 0 {
                metrics.compute_profiles = vec![
                    analytics::PassTiming {
                        name: "Physics (Compute & PBD)".to_string(),
                        duration_ms: physics_duration_ms,
                    },
                    analytics::PassTiming {
                        name: "Diffusion Field".to_string(),
                        duration_ms: diffusion_duration_ms,
                    },
                ];
            }
        }

        // 6. Gather rendering instances
        let mut debug_instances = Vec::new();
        let mut sdf_bones = Vec::new();

        let mut get_connected_component = |entity: bevy_ecs::entity::Entity| {
            let mut adj: std::collections::HashMap<
                bevy_ecs::entity::Entity,
                Vec<bevy_ecs::entity::Entity>,
            > = std::collections::HashMap::new();
            let mut query_springs = self.world.ecs.query::<&physics::Spring>();
            for spring in query_springs.iter(&self.world.ecs) {
                adj.entry(spring.node_a).or_default().push(spring.node_b);
                adj.entry(spring.node_b).or_default().push(spring.node_a);
            }

            let mut queue = std::collections::VecDeque::new();
            let mut visited = std::collections::HashSet::new();
            queue.push_back(entity);
            visited.insert(entity);

            while let Some(curr) = queue.pop_front() {
                if let Some(neighbors) = adj.get(&curr) {
                    for neighbor in neighbors {
                        if visited.insert(*neighbor) {
                            queue.push_back(*neighbor);
                        }
                    }
                }
            }
            visited
        };

        let selected_component = self.selected_entity.map(&mut get_connected_component);

        // Build node position lookup for bone endpoint resolution
        let mut node_positions: std::collections::HashMap<bevy_ecs::entity::Entity, [f32; 2]> =
            std::collections::HashMap::new();

        let mut query_nodes_render = self.world.ecs.query::<(
            bevy_ecs::entity::Entity,
            &physics::ParticleNode,
            Option<&ecology::EcologicalCategory>,
        )>();
        for (entity, node, category) in query_nodes_render.iter(&self.world.ecs) {
            node_positions.insert(entity, [node.position.x, node.position.y]);

            let is_in_selected = selected_component
                .as_ref()
                .is_some_and(|comp| comp.contains(&entity));
            let should_draw_debug =
                self.debug_structural && (selected_component.is_none() || is_in_selected);

            if should_draw_debug {
                debug_instances.push(rendering::DebugInstance {
                    pos_a: [node.position.x, node.position.y],
                    pos_b: [node.position.x, node.position.y],
                    color: match node.segment_type {
                        0 => [0.9, 0.3, 0.3, 1.0], // Head — red
                        2 => [0.3, 0.5, 1.0, 1.0], // Muscle — blue
                        3 => [0.6, 0.6, 0.3, 1.0], // Tail — yellow-green
                        4 => [0.3, 0.9, 0.9, 1.0], // Fin — cyan
                        _ => [0.5, 0.5, 0.5, 1.0], // Torso — grey
                    },
                    radius: if node.segment_type == 4 { 3.0 } else { 5.0 },
                    segment_type: node.segment_type,
                });

                // Draw category ring around head
                if let Some(cat) = category {
                    let ring_color = match cat {
                        ecology::EcologicalCategory::Keystone => Some([1.0, 0.84, 0.0, 1.0]), // Gold
                        ecology::EcologicalCategory::Indicator => Some([0.0, 1.0, 1.0, 1.0]), // Cyan
                        ecology::EcologicalCategory::Endemic => Some([0.0, 0.5, 0.5, 1.0]), // Teal
                        ecology::EcologicalCategory::Invasive => Some([1.0, 0.0, 1.0, 1.0]), // Magenta
                        _ => None,
                    };
                    if let Some(col) = ring_color {
                        debug_instances.push(rendering::DebugInstance {
                            pos_a: [node.position.x, node.position.y],
                            pos_b: [node.position.x, node.position.y],
                            color: [col[0], col[1], col[2], 0.3], // Semi-transparent
                            radius: 12.0,                         // Larger radius
                            segment_type: 99,
                        });
                    }
                }
            }
        }

        // Collect springs for SDF capsule rendering.
        let mut query_springs_render = self
            .world
            .ecs
            .query::<(&physics::Spring, Option<&organisms::OrganismColor>)>();
        for (spring, opt_color) in query_springs_render.iter(&self.world.ecs) {
            let is_in_selected = selected_component
                .as_ref()
                .is_some_and(|comp| comp.contains(&spring.node_a) && comp.contains(&spring.node_b));
            let should_draw_debug =
                self.debug_structural && (selected_component.is_none() || is_in_selected);
            let should_draw_sdf =
                !self.debug_structural || (selected_component.is_some() && !is_in_selected);

            // Skip springs that have no associated organism color (e.g. broken/detached).
            if spring.constraint_type == physics::ConstraintType::Passive && spring.is_fin == 0 {
                // Passive tail bones: thin and dimmed
                if let (Some(&pa), Some(&pb)) = (
                    node_positions.get(&spring.node_a),
                    node_positions.get(&spring.node_b),
                ) {
                    let color = opt_color
                        .map(|c| {
                            let c = c.0;
                            [c[0] * 0.6, c[1] * 0.6, c[2] * 0.6]
                        })
                        .unwrap_or([0.4, 0.4, 0.4]);

                    if should_draw_sdf {
                        sdf_bones.push(rendering::SdfBoneInstance {
                            pos_a: pa,
                            pos_b: pb,
                            radius: 4.0,
                            color,
                        });
                    }
                    if should_draw_debug {
                        debug_instances.push(rendering::DebugInstance {
                            pos_a: pa,
                            pos_b: pb,
                            color: [color[0], color[1], color[2], 0.5],
                            radius: self.bone_line_thickness,
                            segment_type: 99,
                        });
                    }
                }
                continue;
            }

            if spring.constraint_type == physics::ConstraintType::Elastic {
                // Elastic muscle bones: medium weight
                if let (Some(&pa), Some(&pb)) = (
                    node_positions.get(&spring.node_a),
                    node_positions.get(&spring.node_b),
                ) {
                    let color = opt_color.map(|c| c.0).unwrap_or([0.5, 0.5, 0.8]);
                    if should_draw_sdf {
                        sdf_bones.push(rendering::SdfBoneInstance {
                            pos_a: pa,
                            pos_b: pb,
                            radius: 6.0,
                            color,
                        });
                    }
                    if should_draw_debug {
                        debug_instances.push(rendering::DebugInstance {
                            pos_a: pa,
                            pos_b: pb,
                            color: [color[0], color[1], color[2], 0.6],
                            radius: self.bone_line_thickness,
                            segment_type: 99,
                        });
                    }
                }
                continue;
            }

            if spring.constraint_type != physics::ConstraintType::Rigid
                && spring.constraint_type != physics::ConstraintType::Rotational
            {
                continue;
            }
            if let (Some(&pa), Some(&pb)) = (
                node_positions.get(&spring.node_a),
                node_positions.get(&spring.node_b),
            ) {
                // Determine bone thickness
                let radius = if spring.is_fin == 1 { 4.0 } else { 8.0 };

                let color = opt_color.map(|c| c.0).unwrap_or([0.8, 0.8, 0.8]);
                if should_draw_sdf {
                    sdf_bones.push(rendering::SdfBoneInstance {
                        pos_a: pa,
                        pos_b: pb,
                        radius,
                        color,
                    });
                }
                if should_draw_debug {
                    debug_instances.push(rendering::DebugInstance {
                        pos_a: pa,
                        pos_b: pb,
                        color: [color[0], color[1], color[2], 0.8],
                        radius: self.bone_line_thickness,
                        segment_type: 99,
                    });
                }
            }
        }

        // Render food pellets (always shown in debug view)
        let mut query_food = self.world.ecs.query::<&ecology::FoodPellet>();
        for food in query_food.iter(&self.world.ecs) {
            debug_instances.push(rendering::DebugInstance {
                pos_a: [food.position.x, food.position.y],
                pos_b: [food.position.x, food.position.y],
                color: [1.0, 0.8, 0.0, 1.0],
                radius: 2.5,
                segment_type: 0,
            });
        }

        // Render mineral pellets
        let mut query_mineral = self.world.ecs.query::<&ecology::MineralPellet>();
        for mineral in query_mineral.iter(&self.world.ecs) {
            debug_instances.push(rendering::DebugInstance {
                pos_a: [mineral.position.x, mineral.position.y],
                pos_b: [mineral.position.x, mineral.position.y],
                color: [1.0, 1.0, 1.0, 1.0], // Bright White
                radius: 2.0,
                segment_type: 0,
            });
        }

        // Render corpses
        let mut query_corpse = self.world.ecs.query::<&ecology::Corpse>();
        for corpse in query_corpse.iter(&self.world.ecs) {
            debug_instances.push(rendering::DebugInstance {
                pos_a: [corpse.position.x, corpse.position.y],
                pos_b: [corpse.position.x, corpse.position.y],
                color: [0.3, 0.3, 0.3, 1.0], // Dark Grey
                radius: 4.0,
                segment_type: 0,
            });
        }

        // Prepare render frame
        let output = match gpu.surface.get_current_texture() {
            Ok(tex) => tex,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                gpu.surface.configure(&gpu.device, &gpu.config);
                return Ok(());
            }
            Err(wgpu::SurfaceError::Timeout) => return Ok(()),
            Err(e) => return Err(anyhow::anyhow!("surface error: {e}")),
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut central_rect_px = None;

        let mut full_output = None;
        let mut egui_context = None;

        let mut interaction = ui::CanvasInteraction::default();
        let mut scale = 1.0;

        let mut ui_actions = Vec::new();

        if let (Some(egui_state), Some(window)) = (&mut self.egui_state, &self.window) {
            let raw_input = egui_state.take_egui_input(window);
            let ctx = egui_state.egui_ctx().clone();

            let output = ctx.run(raw_input, |ctx| {
                let (canvas_interact, acts) = ui::render_ui(
                    ctx,
                    &mut self.app_state,
                    &mut self.world,
                    self.camera_pos,
                    self.camera_zoom,
                    &mut self.selected_entity,
                    &mut self.tracked_entity,
                    &mut self.debug_structural,
                    &mut self.bone_line_thickness,
                    &mut self.active_tab,
                    &mut self.simulation_speed,
                    &mut self.is_paused,
                    &mut self.show_about,
                    &mut self.show_docs,
                    &mut self.show_vision_cones,
                    self.hovered_entity,
                    &mut self.quit_confirm_time,
                    &mut self.main_menu_confirm_time,
                );
                ui_actions.extend(acts);
                interaction = canvas_interact;
            });

            scale = window.scale_factor() as f32;

            egui_state.handle_platform_output(window, output.platform_output.clone());

            let ui_rect = interaction.rect;

            let x = (ui_rect.min.x * scale).round() as u32;
            let y = (ui_rect.min.y * scale).round() as u32;
            let mut w = (ui_rect.width() * scale).round() as u32;
            let mut h = (ui_rect.height() * scale).round() as u32;

            if x + w > gpu.config.width {
                w = gpu.config.width.saturating_sub(x);
            }
            if y + h > gpu.config.height {
                h = gpu.config.height.saturating_sub(y);
            }

            if w > 0 && h > 0 {
                central_rect_px = Some([x, y, w, h]);
                self.canvas_rect = central_rect_px;
            }

            if let Some(pos) = interaction.hover_pos {
                self.current_hover_pos = Some(common::Vec2::new(pos.x * scale, pos.y * scale));
            } else {
                self.current_hover_pos = None;
            }

            full_output = Some(output);
            egui_context = Some(ctx);
        }

        // Process native interactions from the transparent canvas
        if interaction.zoom_delta != 1.0 && interaction.zoom_delta > 0.0 {
            self.camera_zoom *= interaction.zoom_delta;
            self.camera_zoom = self.camera_zoom.clamp(0.1, 10.0);
        }

        if interaction.drag_delta.length_sq() > 0.0 {
            self.camera_pos.x -= (interaction.drag_delta.x * scale) / self.camera_zoom;
            self.camera_pos.y += (interaction.drag_delta.y * scale) / self.camera_zoom;
            // Only detach tracking if it's a genuine drag, not a trackpad micro-movement
            if interaction.drag_delta.length_sq() > 9.0 {
                self.tracked_entity = None;
            }
        }

        if interaction.clicked {
            if let Some(pos) = interaction.click_pos {
                self.pending_click = Some(common::Vec2::new(pos.x * scale, pos.y * scale));
            }
        }

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame"),
            });

        // Render the continuous diffusion field as the background (clearing the screen)
        if let (Some(field_renderer), Some(diffusion_compute)) = (
            self.field_renderer.as_ref(),
            self.diffusion_compute.as_ref(),
        ) {
            field_renderer.render(
                &gpu.device,
                &mut encoder,
                &view,
                diffusion_compute.current_texture_view(),
                central_rect_px,
            );
        }

        // Submit the field renderer (which clears the screen and draws the background) BEFORE
        // the other renderers, which rely on LoadOp::Load and submit their own encoders.
        gpu.queue.submit(std::iter::once(encoder.finish()));

        let (view_w, view_h) = central_rect_px
            .map(|[_, _, w, h]| (w as f32, h as f32))
            .unwrap_or((gpu.config.width as f32, gpu.config.height as f32));

        // ── Organism rendering — always run sdf_renderer if there are bones ─────────
        if !sdf_bones.is_empty() {
            if let Some(sdf_renderer) = self.sdf_skin_renderer.as_mut() {
                sdf_renderer.render(
                    &gpu.device,
                    &gpu.queue,
                    &view,
                    &sdf_bones,
                    [view_w, view_h],
                    self.camera_pos,
                    self.camera_zoom,
                    central_rect_px,
                );
            }
        }

        if !debug_instances.is_empty() {
            if let Some(debug_renderer) = self.debug_renderer.as_mut() {
                debug_renderer.render(
                    &gpu.device,
                    &gpu.queue,
                    &view,
                    &debug_instances,
                    [view_w, view_h],
                    self.camera_pos,
                    self.camera_zoom,
                    central_rect_px,
                );
            }
        }

        if let (Some(egui_renderer), Some(window), Some(output), Some(ctx)) = (
            &mut self.egui_renderer,
            &self.window,
            full_output,
            egui_context,
        ) {
            let clipped_primitives = ctx.tessellate(output.shapes, window.scale_factor() as f32);
            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [gpu.config.width, gpu.config.height],
                pixels_per_point: window.scale_factor() as f32,
            };

            let mut egui_encoder =
                gpu.device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("egui_encoder"),
                    });

            for (id, image_delta) in &output.textures_delta.set {
                egui_renderer.update_texture(&gpu.device, &gpu.queue, *id, image_delta);
            }

            egui_renderer.update_buffers(
                &gpu.device,
                &gpu.queue,
                &mut egui_encoder,
                &clipped_primitives,
                &screen_descriptor,
            );

            {
                let mut render_pass = egui_encoder
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("egui_render_pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    })
                    .forget_lifetime();
                egui_renderer.render(&mut render_pass, &clipped_primitives, &screen_descriptor);
            }

            gpu.queue.submit(std::iter::once(egui_encoder.finish()));

            for id in &output.textures_delta.free {
                egui_renderer.free_texture(id);
            }
        }

        output.present();

        if self.is_paused && self.total_sim_time > 1.0 {
            println!("SIMULATING SAVE WHILE PAUSED");
            ui_actions.push(ui::MenuAction::SaveState);
            // also exit after one test so we don't spam
            self.total_sim_time = -100.0;
        }
        self.handle_menu_actions(ui_actions);

        Ok(())
    }
}
