//! # Phylon Application
//!
//! The main binary entry point for the Phylon simulation.
//!
//! ## Responsibilities
//!
//! 1. Parse CLI arguments and locate the config file.
//! 2. Initialise structured logging via `tracing_subscriber`.
//! 3. Load `PhylonConfig` from `data/default.ron` (falls back to defaults).
//! 4. Create a `winit` `EventLoop` and application window.
//! 5. Initialise a `wgpu` surface on the window.
//! 6. Create a `SimulationScheduler`.
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

impl PhylonApp {
    /// Advances the simulation and renders one frame.
    pub(crate) fn render(&mut self) -> Result<()> {
        if self.gpu.is_none() || self.physics_compute.is_none() {
            return Ok(());
        }

        const DT: f32 = 0.016; // Fixed 60 Hz timestep

        // 1. Camera Tracking
        if let Some(tracked) = self.ui.tracked_entity {
            if let Ok(node) = self
                .world
                .ecs
                .query::<&physics::ParticleNode>()
                .get(&self.world.ecs, tracked)
            {
                // Smoothly follow the target
                self.ui.camera_pos = self.ui.camera_pos.lerp(node.position, 0.1);
            } else {
                // Entity no longer exists (e.g. died), drop tracking
                self.ui.tracked_entity = None;
            }
        }

        // Only step simulation if we're in the simulation state and not paused
        if self.ui.app_state == ui::AppState::Simulation && !self.ui.is_paused {
            self.accumulated_time += self.simulation_speed;
        }

        let mut physics_duration_ms = 0.0;
        let mut diffusion_duration_ms = 0.0;

        let ticks_this_frame = self.accumulated_time.floor() as u32;
        let ticks_to_run = ticks_this_frame.min(self.max_ticks_per_frame);
        self.accumulated_time -= ticks_this_frame as f32;

        for _ in 0..ticks_to_run {
            let (phys_ms, diff_ms) = self.update_simulation();
            physics_duration_ms += phys_ms;
            diffusion_duration_ms += diff_ms;
        }

        if ticks_to_run > 0 {
            if let Some(gpu) = self.gpu.as_ref() {
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

                    // Override the CPU accumulated timings with the GPU timings (multiplied by ticks to reflect total frame cost)
                    if timestamps[1] > timestamps[0] {
                        let tick_ms =
                            (timestamps[1] - timestamps[0]) as f64 * period as f64 / 1_000_000.0;
                        physics_duration_ms = tick_ms * ticks_to_run as f64;
                    }
                    if timestamps[3] > timestamps[2] {
                        let tick_ms =
                            (timestamps[3] - timestamps[2]) as f64 * period as f64 / 1_000_000.0;
                        diffusion_duration_ms = tick_ms * ticks_to_run as f64;
                    }

                    drop(data);
                    readback.unmap();
                }
            }
        }

        if let Some(diffusion_compute) = self.diffusion_compute.as_mut() {
            if let Some(gpu) = self.gpu.as_ref() {
                if let Some(field_data) = diffusion_compute.try_read_field(&gpu.device) {
                    let mut cpu_field = self.world.ecs.resource_mut::<diffusion::CpuFieldState>();
                    cpu_field.data = field_data;
                }
            }
        }

        // Record analytics — read entity_count before mutably borrowing the resource
        let mut counts = analytics::PopulationCounts::default();
        let mut diet_query = self.world.ecs.query::<&ecology::Diet>();
        for diet in diet_query.iter(&self.world.ecs) {
            match diet {
                ecology::Diet::Producer => counts.producers += 1,
                ecology::Diet::Herbivore => counts.herbivores += 1,
                ecology::Diet::Carnivore => counts.carnivores += 1,
                ecology::Diet::Omnivore => counts.omnivores += 1,
                ecology::Diet::Decomposer => counts.decomposers += 1,
            }
        }

        let mut food_query = self.world.ecs.query::<&ecology::FoodPellet>();
        counts.food_pellets = food_query.iter(&self.world.ecs).count();

        let mut mineral_query = self.world.ecs.query::<&ecology::MineralPellet>();
        counts.minerals = mineral_query.iter(&self.world.ecs).count();

        let mut corpse_query = self.world.ecs.query::<&ecology::Corpse>();
        counts.corpses = corpse_query.iter(&self.world.ecs).count();

        if let Some(mut metrics) = self.world.ecs.get_resource_mut::<analytics::MetricsState>() {
            let sim_dt = (ticks_to_run as f64) * f64::from(DT);
            let real_dt = f64::from(DT); // Fixed render step for now
            metrics.record_frame(counts, sim_dt, real_dt);

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
        let mut hover_bones = Vec::new();
        let mut selected_bones = Vec::new();

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

        let selected_component = self.ui.selected_entity.map(&mut get_connected_component);
        let hovered_component = self.ui.hovered_entity.map(&mut get_connected_component);

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
                self.ui.debug_structural && (selected_component.is_none() || is_in_selected);

            if should_draw_debug {
                debug_instances.push(rendering::DebugInstance {
                    pos_a: [node.position.x, node.position.y],
                    pos_b: [node.position.x, node.position.y],
                    color: match node.segment_type {
                        0 => [1.0, 1.0, 1.0, 1.0],   // Head - Absolute White #FFFFFF
                        2 => [1.0, 0.2, 0.4, 1.0],   // Muscle - Actuation Pink #FF3366
                        3 => [1.0, 0.6, 0.0, 1.0],   // Tail - Terminal Orange #FF9900
                        4 => [0.0, 0.898, 1.0, 1.0], // Fin - Passive Cyan #00E5FF
                        _ => [0.0, 0.898, 1.0, 1.0], // Torso - Passive Cyan #00E5FF
                    },
                    radius: if node.segment_type == 4 {
                        3.0 * (self.ui.node_radius / 5.0)
                    } else {
                        self.ui.node_radius
                    },
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
                            radius: 12.0 * (self.ui.node_radius / 5.0), // Larger radius
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
            let is_in_hovered = hovered_component
                .as_ref()
                .is_some_and(|comp| comp.contains(&spring.node_a) && comp.contains(&spring.node_b));

            let mut highlight_radius = 8.0 * (self.ui.skin_thickness / 3.0);
            if spring.is_fin == 1 {
                highlight_radius = 4.0 * (self.ui.skin_thickness / 3.0);
            }
            if spring.constraint_type == physics::ConstraintType::Passive && spring.is_fin == 0 {
                highlight_radius = 4.0 * (self.ui.skin_thickness / 3.0);
            }
            if spring.constraint_type == physics::ConstraintType::Elastic {
                highlight_radius = 6.0 * (self.ui.skin_thickness / 3.0);
            }

            if let (Some(&pa), Some(&pb)) = (
                node_positions.get(&spring.node_a),
                node_positions.get(&spring.node_b),
            ) {
                if is_in_hovered {
                    hover_bones.push(rendering::SdfBoneInstance {
                        pos_a: pa,
                        pos_b: pb,
                        radius: highlight_radius,
                        color: [0.0, 1.0, 0.0],
                    });
                }
                if is_in_selected {
                    selected_bones.push(rendering::SdfBoneInstance {
                        pos_a: pa,
                        pos_b: pb,
                        radius: highlight_radius,
                        color: [1.0, 1.0, 1.0],
                    });
                }
            }

            let should_draw_debug =
                self.ui.debug_structural && (selected_component.is_none() || is_in_selected);
            let should_draw_sdf =
                !self.ui.debug_structural || (selected_component.is_some() && !is_in_selected);

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
                            radius: 4.0 * (self.ui.skin_thickness / 3.0),
                            color,
                        });
                    }
                    if should_draw_debug {
                        debug_instances.push(rendering::DebugInstance {
                            pos_a: pa,
                            pos_b: pb,
                            color: [0.533, 0.573, 0.690, 0.4],
                            radius: self.ui.bone_line_thickness,
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
                            radius: 6.0 * (self.ui.skin_thickness / 3.0),
                            color,
                        });
                    }
                    if should_draw_debug {
                        debug_instances.push(rendering::DebugInstance {
                            pos_a: pa,
                            pos_b: pb,
                            color: [0.533, 0.573, 0.690, 0.4],
                            radius: self.ui.bone_line_thickness,
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
                let radius =
                    (if spring.is_fin == 1 { 4.0 } else { 8.0 }) * (self.ui.skin_thickness / 3.0);

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
                        color: [0.533, 0.573, 0.690, 0.4],
                        radius: self.ui.bone_line_thickness,
                        segment_type: 99,
                    });
                }
            }
        }

        // Render food pellets (always shown in debug view)
        let mut query_food = self
            .world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &ecology::FoodPellet)>();
        for (entity, food) in query_food.iter(&self.world.ecs) {
            let pos = [food.position.x, food.position.y];
            let is_in_selected = selected_component
                .as_ref()
                .is_some_and(|c| c.contains(&entity));
            let should_draw_debug =
                self.ui.debug_structural && (selected_component.is_none() || is_in_selected);
            let should_draw_sdf =
                !self.ui.debug_structural || (selected_component.is_some() && !is_in_selected);

            if should_draw_debug {
                debug_instances.push(rendering::DebugInstance {
                    pos_a: pos,
                    pos_b: pos,
                    color: [1.0, 0.835, 0.310, 1.0], // #FFD54F
                    radius: 2.5,
                    segment_type: 0,
                });
            }
            if should_draw_sdf {
                sdf_bones.push(rendering::SdfBoneInstance {
                    pos_a: pos,
                    pos_b: pos,
                    radius: 2.5,
                    color: [1.0, 0.835, 0.310], // #FFD54F
                });
            }
            if hovered_component
                .as_ref()
                .is_some_and(|c| c.contains(&entity))
            {
                hover_bones.push(rendering::SdfBoneInstance {
                    pos_a: pos,
                    pos_b: pos,
                    radius: 2.5,
                    color: [0.0, 1.0, 0.0],
                });
            }
            if selected_component
                .as_ref()
                .is_some_and(|c| c.contains(&entity))
            {
                selected_bones.push(rendering::SdfBoneInstance {
                    pos_a: pos,
                    pos_b: pos,
                    radius: 2.5,
                    color: [1.0, 1.0, 1.0],
                });
            }
        }

        // Render mineral pellets
        let mut query_mineral = self
            .world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &ecology::MineralPellet)>();
        for (entity, mineral) in query_mineral.iter(&self.world.ecs) {
            let pos = [mineral.position.x, mineral.position.y];
            let is_in_selected = selected_component
                .as_ref()
                .is_some_and(|c| c.contains(&entity));
            let should_draw_debug =
                self.ui.debug_structural && (selected_component.is_none() || is_in_selected);
            let should_draw_sdf =
                !self.ui.debug_structural || (selected_component.is_some() && !is_in_selected);

            if should_draw_debug {
                debug_instances.push(rendering::DebugInstance {
                    pos_a: pos,
                    pos_b: pos,
                    color: [0.663, 0.761, 0.788, 1.0], // #A9C2C9
                    radius: 2.0,
                    segment_type: 0,
                });
            }
            if should_draw_sdf {
                sdf_bones.push(rendering::SdfBoneInstance {
                    pos_a: pos,
                    pos_b: pos,
                    radius: 2.0,
                    color: [0.663, 0.761, 0.788], // #A9C2C9
                });
            }
            if hovered_component
                .as_ref()
                .is_some_and(|c| c.contains(&entity))
            {
                hover_bones.push(rendering::SdfBoneInstance {
                    pos_a: pos,
                    pos_b: pos,
                    radius: 2.0,
                    color: [0.0, 1.0, 0.0],
                });
            }
            if selected_component
                .as_ref()
                .is_some_and(|c| c.contains(&entity))
            {
                selected_bones.push(rendering::SdfBoneInstance {
                    pos_a: pos,
                    pos_b: pos,
                    radius: 2.0,
                    color: [1.0, 1.0, 1.0],
                });
            }
        }

        // Render corpses
        let mut query_corpse = self
            .world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &ecology::Corpse)>();
        for (entity, corpse) in query_corpse.iter(&self.world.ecs) {
            let pos = [corpse.position.x, corpse.position.y];
            let is_in_selected = selected_component
                .as_ref()
                .is_some_and(|c| c.contains(&entity));
            let should_draw_debug =
                self.ui.debug_structural && (selected_component.is_none() || is_in_selected);
            let should_draw_sdf =
                !self.ui.debug_structural || (selected_component.is_some() && !is_in_selected);

            if should_draw_debug {
                debug_instances.push(rendering::DebugInstance {
                    pos_a: pos,
                    pos_b: pos,
                    color: [0.427, 0.404, 0.431, 1.0], // #6D676E
                    radius: 4.0,
                    segment_type: 0,
                });
            }
            if should_draw_sdf {
                sdf_bones.push(rendering::SdfBoneInstance {
                    pos_a: pos,
                    pos_b: pos,
                    radius: 4.0,
                    color: [0.427, 0.404, 0.431], // #6D676E
                });
            }
            if hovered_component
                .as_ref()
                .is_some_and(|c| c.contains(&entity))
            {
                hover_bones.push(rendering::SdfBoneInstance {
                    pos_a: pos,
                    pos_b: pos,
                    radius: 4.0,
                    color: [0.0, 1.0, 0.0],
                });
            }
            if selected_component
                .as_ref()
                .is_some_and(|c| c.contains(&entity))
            {
                selected_bones.push(rendering::SdfBoneInstance {
                    pos_a: pos,
                    pos_b: pos,
                    radius: 4.0,
                    color: [1.0, 1.0, 1.0],
                });
            }
        }

        let gpu = self.gpu.as_mut().unwrap();

        // Prepare render frame
        let output = match gpu.surface.as_ref().unwrap().get_current_texture() {
            Ok(tex) => tex,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                gpu.surface
                    .as_ref()
                    .unwrap()
                    .configure(&gpu.device, gpu.config.as_ref().unwrap());
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
                    &mut self.ui.app_state,
                    &mut self.world,
                    self.ui.camera_pos,
                    self.ui.camera_zoom,
                    &mut self.ui.selected_entity,
                    &mut self.ui.tracked_entity,
                    &mut self.ui.debug_structural,
                    &mut self.ui.bone_line_thickness,
                    &mut self.ui.skin_thickness,
                    &mut self.ui.node_radius,
                    &mut self.ui.active_tab,
                    &mut self.simulation_speed,
                    &mut self.ui.is_paused,
                    &mut self.ui.show_about,
                    &mut self.ui.show_docs,
                    &mut self.ui.show_vision_cones,
                    self.ui.hovered_entity,
                    &mut self.ui.quit_confirm_time,
                    &mut self.ui.main_menu_confirm_time,
                    &mut self.ui.spectator_mode,
                    &mut self.ui.last_spectator_switch_time,
                );
                ui_actions.extend(acts);
                interaction = canvas_interact;

                // Render active toast if present
                if let Some((msg, progress)) = &self.ui.active_toast {
                    egui::Window::new("Progress")
                        .title_bar(false)
                        .resizable(false)
                        .collapsible(false)
                        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-20.0, -20.0))
                        .show(ctx, |ui| {
                            ui.label(egui::RichText::new(msg).strong());
                            ui.add_space(4.0);
                            ui.add(egui::ProgressBar::new(*progress).animate(true));
                        });
                }
            });

            scale = window.scale_factor() as f32;

            egui_state.handle_platform_output(window, output.platform_output.clone());

            let ui_rect = interaction.rect;

            let x = (ui_rect.min.x * scale).round() as u32;
            let y = (ui_rect.min.y * scale).round() as u32;
            let mut w = (ui_rect.width() * scale).round() as u32;
            let mut h = (ui_rect.height() * scale).round() as u32;

            if let Some(config) = gpu.config.as_ref() {
                if x + w > config.width {
                    w = config.width.saturating_sub(x);
                }
                if y + h > config.height {
                    h = config.height.saturating_sub(y);
                }
            }

            if w > 0 && h > 0 {
                central_rect_px = Some([x, y, w, h]);
                self.ui.canvas_rect = central_rect_px;
            }

            if let Some(pos) = interaction.hover_pos {
                self.ui.current_hover_pos = Some(common::Vec2::new(pos.x * scale, pos.y * scale));
            } else {
                self.ui.current_hover_pos = None;
            }

            full_output = Some(output);
            egui_context = Some(ctx);
        }

        // Process native interactions from the transparent canvas
        if interaction.zoom_delta != 1.0 && interaction.zoom_delta > 0.0 {
            self.ui.camera_zoom *= interaction.zoom_delta;
            self.ui.camera_zoom = self.ui.camera_zoom.clamp(0.1, 10.0);
        }

        if interaction.drag_delta.length_sq() > 0.0 {
            self.ui.camera_pos.x -= (interaction.drag_delta.x * scale) / self.ui.camera_zoom;
            self.ui.camera_pos.y += (interaction.drag_delta.y * scale) / self.ui.camera_zoom;
            // Only detach tracking if it's a genuine drag, not a trackpad micro-movement
            if interaction.drag_delta.length_sq() > 9.0 {
                self.ui.tracked_entity = None;
            }
        }

        if interaction.clicked {
            if let Some(pos) = interaction.click_pos {
                self.ui.pending_click = Some(common::Vec2::new(pos.x * scale, pos.y * scale));
            }
        }

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame"),
            });

        // Get sunlight for background color
        let mut clear_color = wgpu::Color {
            r: 0.012,
            g: 0.008,
            b: 0.047,
            a: 1.0,
        };
        if let Some(atmosphere) = self
            .world
            .ecs
            .get_resource::<metabolism::GlobalAtmosphere>()
        {
            let s = atmosphere.sunlight as f64;
            clear_color = wgpu::Color {
                r: 0.012 * (1.0 - s) + 0.102 * s,
                g: 0.008 * (1.0 - s) + 0.294 * s,
                b: 0.047 * (1.0 - s) + 0.467 * s,
                a: 1.0,
            };
        }

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
                clear_color,
            );
        }

        // Submit the field renderer (which clears the screen and draws the background) BEFORE
        // the other renderers, which rely on LoadOp::Load and submit their own encoders.
        gpu.queue.submit(std::iter::once(encoder.finish()));

        let (view_w, view_h) = central_rect_px
            .map(|[_, _, w, h]| (w as f32, h as f32))
            .unwrap_or_else(|| {
                gpu.config
                    .as_ref()
                    .map(|c| (c.width as f32, c.height as f32))
                    .unwrap_or((1280.0, 720.0))
            });

        // ── Organism rendering — always run sdf_renderer if there are bones ─────────
        if !sdf_bones.is_empty() {
            if let Some(sdf_renderer) = self.sdf_skin_renderer.as_mut() {
                sdf_renderer.render(
                    &gpu.device,
                    &gpu.queue,
                    &view,
                    &sdf_bones,
                    [view_w, view_h],
                    self.ui.camera_pos,
                    self.ui.camera_zoom,
                    central_rect_px,
                );
            }
        }

        if !hover_bones.is_empty() {
            if let Some(sdf_renderer) = self.sdf_skin_renderer.as_mut() {
                sdf_renderer.render_highlight(
                    &gpu.device,
                    &gpu.queue,
                    &view,
                    &hover_bones,
                    [0.0, 1.0, 0.0, 1.0],
                    [view_w, view_h],
                    self.ui.camera_pos,
                    self.ui.camera_zoom,
                    central_rect_px,
                );
            }
        }

        if !selected_bones.is_empty() {
            if let Some(sdf_renderer) = self.sdf_skin_renderer.as_mut() {
                let pulse = 0.6 + 0.4 * (self.total_sim_time * 3.0).sin();
                sdf_renderer.render_highlight(
                    &gpu.device,
                    &gpu.queue,
                    &view,
                    &selected_bones,
                    [1.0, 1.0, 1.0, pulse],
                    [view_w, view_h],
                    self.ui.camera_pos,
                    self.ui.camera_zoom,
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
                    self.ui.camera_pos,
                    self.ui.camera_zoom,
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
                size_in_pixels: [
                    gpu.config.as_ref().map(|c| c.width).unwrap_or(1280),
                    gpu.config.as_ref().map(|c| c.height).unwrap_or(720),
                ],
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

        self.handle_menu_actions(ui_actions);

        Ok(())
    }
}
