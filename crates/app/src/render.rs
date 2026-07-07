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
    /// # Main Frame Renderer and Time Integrator
    ///
    /// ## 1. What Happens
    /// The `render` method is called every time the OS requests a frame redraw. It handles camera
    /// tracking, accumulates delta time, triggers biological simulation ticks to catch up to real-time,
    /// and dispatches the final WGPU render passes (Splatting, Heatmaps, UI).
    ///
    /// ## 2. Why It Happens
    /// Simulation physics must run at a fixed, deterministic timestep (`DT`, the configured
    /// [`common::TickRate`]) to ensure biological processes (like energy decay or neuron
    /// membrane potentials) do not destabilize. However, monitor refresh rates fluctuate. This
    /// method decouples the render framerate from the biological tick rate using a
    /// fixed-timestep accumulator algorithm.
    ///
    /// ## 3. How It Happens
    /// The method utilizes an accumulator model:
    ///
    /// $$ t_{accum} = t_{accum} + (speed \times \Delta t_{frame}) $$
    ///
    /// While $t_{accum} \ge 1.0$, the engine calls `update_simulation()` to step the ECS forward by
    /// the fixed `DT` seconds, decrementing $t_{accum}$. Once caught up, it builds the WGPU
    /// `CommandEncoder`, executes the Gaussian Splat and Heatmap render passes, and renders the `egui`
    /// contexts.
    pub(crate) fn render(&mut self) -> Result<()> {
        if self.gpu.is_none() || self.physics_compute.is_none() {
            return Ok(());
        }

        let dt = self.world.ecs.resource::<common::TickRate>().dt();

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

        // Advance the accumulator by real elapsed wall-clock time (not by a
        // fixed per-redraw amount), so the tick rate tracks real time even
        // when frame time fluctuates or stalls.
        let now = std::time::Instant::now();
        let real_frame_dt = (now - self.last_frame_instant).as_secs_f32();
        self.last_frame_instant = now;

        // Guard against huge jumps (e.g. window was minimized/dragged) so we
        // don't try to catch up on minutes of missed simulation at once.
        let real_frame_dt = real_frame_dt.min(0.25);

        // Only step simulation if we're in the simulation state and not paused
        if self.app_state == ui::AppState::Simulation && !self.ui.is_paused {
            self.accumulated_time += (real_frame_dt / dt) * self.simulation_speed;
        }

        let mut physics_duration_ms = 0.0;
        let mut diffusion_duration_ms = 0.0;

        let ticks_this_frame = self.accumulated_time.floor() as u32;
        let ticks_to_run = ticks_this_frame.min(self.max_ticks_per_frame);
        self.accumulated_time -= ticks_this_frame as f32;

        // Wall-clock time budget: on top of the tick-count cap above, stop
        // running queued ticks once we've spent too long simulating this
        // frame. Without this, an overloaded simulation (tick cost > DT)
        // keeps trying to run up to `max_ticks_per_frame` ticks every frame
        // regardless of how long each one takes — e.g. 50 ticks at 20ms each
        // is a full second per rendered frame (~1 FPS). With the budget, the
        // simulation visibly falls behind real time under sustained load
        // instead of freezing the whole app; unrun ticks are credited back
        // to `accumulated_time` so they're retried next frame, not lost.
        const MAX_TICK_TIME_BUDGET: std::time::Duration = std::time::Duration::from_millis(20);
        let tick_budget_start = std::time::Instant::now();
        let mut ticks_run = 0u32;
        for _ in 0..ticks_to_run {
            let (phys_ms, diff_ms) = self.update_simulation();
            physics_duration_ms += phys_ms;
            diffusion_duration_ms += diff_ms;
            ticks_run += 1;
            if tick_budget_start.elapsed() > MAX_TICK_TIME_BUDGET {
                break;
            }
        }
        if ticks_run < ticks_to_run {
            self.accumulated_time += (ticks_to_run - ticks_run) as f32;
        }
        let ticks_to_run = ticks_run;

        // The timestamp-query readback below is a blocking `device.poll(Wait)`
        // purely for the profiling display — skip it entirely when the
        // Metrics panel isn't visible so we don't pay a GPU stall every frame
        // for numbers nobody's looking at.
        if ticks_to_run > 0 && self.ui.metrics_visible {
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

        // Record analytics. `smoothed_fps`/`smoothed_tps`/`sim_time` (updated
        // by `record_frame`/`record_env_perf` below) are always kept current
        // since the always-visible status bar reads them — but the ~4 full-
        // population ECS scans that feed the Metrics panel's demographic
        // history plots are skipped when that panel isn't visible (same
        // rationale as the GPU timestamp-query gating above): nobody's
        // looking at the graphs, and at high entity counts these scans
        // aren't free.
        let mut counts = analytics::PopulationCounts::default();
        if self.ui.metrics_visible {
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
        }

        let mut env_sunlight = 0.0;
        let mut env_o2 = 0.0;
        let mut env_co2 = 0.0;
        let mut env_temp = 22.0;
        if let Some(atmosphere) = self
            .world
            .ecs
            .get_resource::<metabolism::GlobalAtmosphere>()
        {
            env_sunlight = atmosphere.sunlight;
            env_o2 = atmosphere.o2;
            env_co2 = atmosphere.co2;
            env_temp = atmosphere.temp;
        }

        if let Some(mut metrics) = self.world.ecs.get_resource_mut::<analytics::MetricsState>() {
            let sim_dt = (ticks_to_run as f64) * f64::from(dt);
            let real_dt = f64::from(real_frame_dt);
            metrics.record_frame(counts, sim_dt, real_dt);

            // Calculate TPS
            let tps = if real_dt > 0.0 {
                (ticks_to_run as f64) / real_dt
            } else {
                0.0
            };

            // Get memory (cached to avoid extreme lag)
            thread_local! {
                static SYS: std::cell::RefCell<sysinfo::System> = std::cell::RefCell::new(sysinfo::System::new_all());
            }
            let memory_mb = SYS.with(|sys_cell| {
                let mut sys = sys_cell.borrow_mut();
                if let Ok(pid) = sysinfo::get_current_pid() {
                    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
                    if let Some(process) = sys.process(pid) {
                        return (process.memory() / 1024 / 1024) as f64;
                    }
                }
                0.0
            });

            metrics.record_env_perf(
                tps,
                memory_mb,
                env_sunlight as f64,
                env_o2 as f64,
                env_co2 as f64,
                env_temp as f64,
            );

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
        // `panel_hover_entity` (Phase 2, M9) lets a non-viewport panel (e.g.
        // the Lineage Explorer) drive the same highlight a viewport-hover
        // would — `hovered_entity` itself stays viewport-picking-only since
        // it's unconditionally overwritten every frame in `events.rs`.
        let hovered_component = self
            .ui
            .hovered_entity
            .or(self.ui.panel_hover_entity)
            .map(&mut get_connected_component);

        // Camera-frustum culling for the (potentially whole-population-sized)
        // SDF bone list: skip gathering/uploading/instancing bones that fall
        // entirely outside the visible viewport. Uses the last known canvas
        // rect (one frame stale at worst — negligible) since the current
        // frame's egui layout hasn't run yet at this point in `render()`.
        let (cull_w, cull_h) = self
            .ui
            .canvas_rect
            .map(|[_, _, w, h]| (w as f32, h as f32))
            .unwrap_or_else(|| {
                self.gpu
                    .as_ref()
                    .and_then(|g| g.config.as_ref())
                    .map(|c| (c.width as f32, c.height as f32))
                    .unwrap_or((1280.0, 720.0))
            });
        const CULL_MARGIN: f32 = 100.0; // generous slack for bone radius + node_radius
        let cull_half_w = cull_w / 2.0 / self.ui.camera_zoom + CULL_MARGIN;
        let cull_half_h = cull_h / 2.0 / self.ui.camera_zoom + CULL_MARGIN;
        let cull_min_x = self.ui.camera_pos.x - cull_half_w;
        let cull_max_x = self.ui.camera_pos.x + cull_half_w;
        let cull_min_y = self.ui.camera_pos.y - cull_half_h;
        let cull_max_y = self.ui.camera_pos.y + cull_half_h;
        let bone_visible = |pa: [f32; 2], pb: [f32; 2]| -> bool {
            let min_x = pa[0].min(pb[0]);
            let max_x = pa[0].max(pb[0]);
            let min_y = pa[1].min(pb[1]);
            let max_y = pa[1].max(pb[1]);
            max_x >= cull_min_x && min_x <= cull_max_x && max_y >= cull_min_y && min_y <= cull_max_y
        };

        // Build node position lookup for bone endpoint resolution
        let mut node_positions: std::collections::HashMap<bevy_ecs::entity::Entity, [f32; 2]> =
            std::collections::HashMap::new();

        // Per-node organism-id lookup (Phase 5, SX-3d) — `physics::ParticleNode.organism_id`
        // is already stored per node; no BFS/adjacency-map traversal is
        // needed to find colony (budding) links, only a same-tick
        // comparison of two nodes' `organism_id` on either end of a spring
        // (see the spring loop below). Built in the same query as
        // `node_positions` below, at no extra query cost.
        let mut entity_organism_id: std::collections::HashMap<bevy_ecs::entity::Entity, u32> =
            std::collections::HashMap::new();

        // Per-segment vitality lookup (Phase 5, SX-1c) — `metabolism::Health`
        // lives only on an organism's head entity (`organisms::spawning`), not
        // every segment, so this maps every segment entity in that organism's
        // `DevelopmentalGraph` to the same head-derived health fraction,
        // mirroring `render_physiology_overlay`'s existing graph-walk pattern
        // rather than inventing a new one. Absent from this map (sandbox
        // structures with no `Health`) defaults to `1.0` (fully vital) at the
        // lookup site below, not inserted here.
        let mut entity_health_fraction: std::collections::HashMap<bevy_ecs::entity::Entity, f32> =
            std::collections::HashMap::new();
        let mut query_health_graphs = self
            .world
            .ecs
            .query::<(&organisms::DevelopmentalGraph, &metabolism::Health)>();
        for (graph, health) in query_health_graphs.iter(&self.world.ecs) {
            let fraction = if health.max > 0.0 {
                (health.current / health.max).clamp(0.0, 1.0)
            } else {
                0.0
            };
            for graph_node in &graph.nodes {
                if let Some(seg_entity) = graph_node.entity {
                    entity_health_fraction.insert(seg_entity, fraction);
                }
            }
        }

        // Per-organism average infection severity lookup (Phase 5, SX-1d),
        // keyed by head entity (the only entity `ecology::disease::Infection`
        // lives on, mirroring `metabolism::Health`'s own placement) — walks
        // the same `DevelopmentalGraph` used above for Health, averaging
        // `SegmentInfection.severity` across segments rather than reading a
        // single value, since severity is a genuinely per-segment quantity
        // (P4-F5).
        let mut entity_avg_severity: std::collections::HashMap<bevy_ecs::entity::Entity, f32> =
            std::collections::HashMap::new();
        let mut query_segment_infection = self
            .world
            .ecs
            .query::<&ecology::disease::SegmentInfection>();
        let mut query_infection_graphs = self.world.ecs.query::<(
            bevy_ecs::entity::Entity,
            &organisms::DevelopmentalGraph,
            &ecology::disease::Infection,
        )>();
        for (head_entity, graph, _infection) in query_infection_graphs.iter(&self.world.ecs) {
            let mut total = 0.0f32;
            let mut count = 0u32;
            for graph_node in &graph.nodes {
                if let Some(seg_entity) = graph_node.entity {
                    if let Ok(seg_infection) =
                        query_segment_infection.get(&self.world.ecs, seg_entity)
                    {
                        total += seg_infection.severity;
                        count += 1;
                    }
                }
            }
            if count > 0 {
                entity_avg_severity.insert(head_entity, total / count as f32);
            }
        }

        let mut query_nodes_render = self.world.ecs.query::<(
            bevy_ecs::entity::Entity,
            &physics::ParticleNode,
            Option<&ecology::EcologicalCategory>,
            Option<&metabolism::Health>,
            Option<&ecology::disease::Infection>,
        )>();
        for (entity, node, category, health, infection) in query_nodes_render.iter(&self.world.ecs)
        {
            node_positions.insert(entity, [node.position.x, node.position.y]);
            entity_organism_id.insert(entity, node.organism_id);

            let is_in_selected = selected_component
                .as_ref()
                .is_some_and(|comp| comp.contains(&entity));
            let should_draw_debug =
                self.ui.debug_structural && (selected_component.is_none() || is_in_selected);

            // Low-health ring (Phase 5, SX-1c) — Primary tier, always visible,
            // not gated behind `debug_structural` (unlike the category ring
            // below, which stays debug-only). `Health` only exists on the
            // head entity (`organisms::spawning`), so this naturally draws
            // once per organism. Amber below 40%, red below 15%, per
            // `docs/design/biological_visual_language.md`'s Health entry;
            // nothing drawn above 40% — absence is the encoding for the
            // common healthy case, matching SX-1b's Idle precedent.
            if let Some(health) = health {
                let fraction = if health.max > 0.0 {
                    (health.current / health.max).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                if fraction < 0.40 {
                    let token = if fraction < 0.15 {
                        ui::theme::BAD
                    } else {
                        ui::theme::WARN
                    };
                    let [r, g, b, _] = token.to_normalized_gamma_f32();
                    debug_instances.push(rendering::DebugInstance {
                        pos_a: [node.position.x, node.position.y],
                        pos_b: [node.position.x, node.position.y],
                        color: [r, g, b, 0.6],
                        radius: 10.0 * (self.ui.node_radius / 5.0),
                        segment_type: 99,
                    });
                }
            }

            // Disease badge (Phase 5, SX-1d) — Primary tier, always visible,
            // offset up-and-left from the head position so it never blends
            // with the Health disk above into an ambiguous combined color
            // (both are opaque-ish filled disks on the shared
            // `debug_quad.wgsl` primitive — see this document's own note in
            // `biological_visual_language.md`'s Disease entry on why this
            // isn't a true concentric ring). No animation — a fully static
            // function of current `Infection.state`/severity, per this
            // milestone's explicit "no pulses" instruction.
            if let Some(infection) = infection {
                let avg_severity = entity_avg_severity.get(&entity).copied().unwrap_or(0.0);
                let health_fraction = health.map_or(1.0, |h| {
                    if h.max > 0.0 {
                        (h.current / h.max).clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                });
                let is_critical = avg_severity > 0.70 || health_fraction < 0.15;

                let (color, alpha, radius) = match infection.state {
                    ecology::disease::InfectionState::Incubating => {
                        // Faintest, smallest — biologically asymptomatic, but
                        // not literally invisible (this milestone's own brief
                        // asks that it be distinguishable at a glance).
                        ([0.6, 0.6, 0.65], 0.25, 4.0)
                    }
                    ecology::disease::InfectionState::Infectious if is_critical => {
                        // Critical: an intensified version of Infectious, not
                        // a separately-tracked simulation state — escalates
                        // toward `theme::BAD`'s hue.
                        let [r, g, b, _] = ui::theme::BAD.to_normalized_gamma_f32();
                        ([r, g, b], 0.85, 9.0)
                    }
                    ecology::disease::InfectionState::Infectious => {
                        let purple = ecology::Diet::Decomposer.standard_color();
                        (purple, 0.4 + avg_severity * 0.4, 5.0 + avg_severity * 3.0)
                    }
                    ecology::disease::InfectionState::Recovered => {
                        // Permanent "survived and immune" marker — solid,
                        // small, not severity-scaled (there's no ongoing
                        // severity for a recovered organism).
                        let [r, g, b, _] = ui::theme::GOOD.to_normalized_gamma_f32();
                        ([r, g, b], 0.5, 4.0)
                    }
                };
                let offset = 12.0 * (self.ui.node_radius / 5.0);
                debug_instances.push(rendering::DebugInstance {
                    pos_a: [node.position.x - offset, node.position.y - offset],
                    pos_b: [node.position.x - offset, node.position.y - offset],
                    color: [color[0], color[1], color[2], alpha],
                    radius,
                    segment_type: 99,
                });
            }

            if should_draw_debug {
                debug_instances.push(rendering::DebugInstance {
                    pos_a: [node.position.x, node.position.y],
                    pos_b: [node.position.x, node.position.y],
                    color: match node.segment_type {
                        0 => [1.000, 1.000, 1.000, 1.0], // Head - Absolute White #FFFFFF
                        2 => [1.000, 0.033, 0.133, 1.0], // Muscle - Actuation Pink #FF3366
                        3 => [1.000, 0.319, 0.000, 1.0], // Tail - Terminal Orange #FF9900
                        4 => [0.000, 0.784, 1.000, 1.0], // Fin - Passive Cyan #00E5FF
                        5 => [1.000, 0.000, 0.400, 1.0], // Vascular - Circulatory Magenta #FF0066
                        6 => [0.600, 0.200, 1.000, 1.0], // Ganglion - Neural Violet #9933FF
                        7 => [1.000, 0.843, 0.000, 1.0], // Germinal - Germ-line Gold #FFD700
                        _ => [0.000, 0.784, 1.000, 1.0], // Torso - Passive Cyan #00E5FF
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

        // Developmental growth visual (Phase 5, SX-2d): a just-formed segment
        // scales in from near-0 to full radius over a short fixed window,
        // rather than popping into existence at full size. `SpawnTick`
        // (reused, not a new component — see `organisms::systems::growth_system`'s
        // doc comment) is looked up per spring's `node_b`, which is always
        // the newer of the two endpoints under this codebase's own `Spring`
        // convention (`node_a` is always the pre-existing parent/anchor;
        // confirmed by reading every `Spring` construction site in
        // `growth_system`/`producer_growth_system`, not assumed).
        const GROWTH_FADE_IN_TICKS: u64 = 30; // 0.5s at 60Hz — brief, per the design doc
        let current_tick = self
            .world
            .ecs
            .get_resource::<metabolism::GlobalAtmosphere>()
            .map_or(0, |a| a.ticks);
        let mut query_spawn_ticks = self
            .world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &organisms::SpawnTick)>();
        let entity_growth_progress: std::collections::HashMap<bevy_ecs::entity::Entity, f32> =
            query_spawn_ticks
                .iter(&self.world.ecs)
                .map(|(e, spawn_tick)| {
                    let age_ticks = current_tick.saturating_sub(spawn_tick.0);
                    let progress = (age_ticks as f32 / GROWTH_FADE_IN_TICKS as f32).clamp(0.0, 1.0);
                    (e, progress)
                })
                .collect();

        // Spotlight mode (Phase 5, SX-5b) — dims every organism except the
        // selected entity, its connected body/colony (reusing the exact
        // `selected_component` BFS already computed above for selection
        // highlighting, not a second traversal), and any other organism
        // whose head falls within the selected organism's interaction
        // radius (`sensing::HeadVision.range`, falling back to `250.0` —
        // the same fallback SX-4c/5a's nearby-organism lookups already use,
        // for consistency). `None` (not gated at all) when the mode is off
        // or nothing is selected, so this costs nothing in the common case.
        let spotlight_dim_entities: Option<std::collections::HashSet<bevy_ecs::entity::Entity>> =
            if self.ui.spotlight_mode {
                self.ui.selected_entity.map(|selected| {
                    let selected_pos = node_positions.get(&selected).copied();
                    let radius = {
                        let mut vision_q = self.world.ecs.query::<&sensing::HeadVision>();
                        vision_q
                            .get(&self.world.ecs, selected)
                            .map(|v| v.range)
                            .unwrap_or(250.0)
                    };

                    let mut nearby_organism_ids: std::collections::HashSet<u32> =
                        std::collections::HashSet::new();
                    if let Some(&sel_pos) = selected_pos.as_ref() {
                        let mut diet_q = self
                            .world
                            .ecs
                            .query::<(&physics::ParticleNode, &ecology::Diet)>();
                        for (node, _diet) in diet_q.iter(&self.world.ecs) {
                            let d = ((node.position.x - sel_pos[0]).powi(2)
                                + (node.position.y - sel_pos[1]).powi(2))
                            .sqrt();
                            if d <= radius {
                                nearby_organism_ids.insert(node.organism_id);
                            }
                        }
                    }

                    // Every entity to *keep lit*: the BFS-connected body/
                    // colony, plus every segment belonging to a nearby
                    // organism_id.
                    let mut lit: std::collections::HashSet<bevy_ecs::entity::Entity> =
                        selected_component.clone().unwrap_or_default();
                    for (&e, &oid) in entity_organism_id.iter() {
                        if nearby_organism_ids.contains(&oid) {
                            lit.insert(e);
                        }
                    }

                    // Return the complement — everything to *dim* — since
                    // that's what the render loop below actually looks up
                    // per-bone (dimming is the exception, not the rule).
                    entity_organism_id
                        .keys()
                        .filter(|e| !lit.contains(e))
                        .copied()
                        .collect()
                })
            } else {
                None
            };
        const SPOTLIGHT_DIM_FACTOR: f32 = 0.15;
        let spotlight_factor = |e: bevy_ecs::entity::Entity| -> f32 {
            match &spotlight_dim_entities {
                Some(dim_set) if dim_set.contains(&e) => SPOTLIGHT_DIM_FACTOR,
                _ => 1.0,
            }
        };

        // Collect springs for SDF capsule rendering.
        let mut query_springs_render = self
            .world
            .ecs
            .query::<(&physics::Spring, Option<&organisms::OrganismColor>)>();
        for (spring, opt_color) in query_springs_render.iter(&self.world.ecs) {
            // Absent from the map (segments that existed before this
            // milestone shipped, or non-organism structures) defaults to
            // `1.0` — fully grown, not "always freshly spawned."
            let growth_scale = entity_growth_progress
                .get(&spring.node_b)
                .copied()
                .unwrap_or(1.0);

            // Colony/migration visualization (Phase 5, SX-3d) — a spring
            // whose two endpoints belong to different organisms *is* a
            // colony (budding) link, by the same definition
            // `analytics_bridge_system`'s colony-connectivity graph already
            // uses. Population-wide, always visible (not gated by
            // selection) — Priority 4 (Ecological status) per the Numeric
            // priority hierarchy, so drawn via `debug_instances` alongside
            // Health/Disease, beneath the Priority-1 selection/hover
            // highlight (unaffected — draw order for `debug_instances`
            // itself was already fixed at SX-1e).
            if let (Some(&org_a), Some(&org_b), Some(&pa), Some(&pb)) = (
                entity_organism_id.get(&spring.node_a),
                entity_organism_id.get(&spring.node_b),
                node_positions.get(&spring.node_a),
                node_positions.get(&spring.node_b),
            ) {
                if org_a != org_b {
                    let [r, g, b, _] = ui::theme::ACCENT.to_normalized_gamma_f32();
                    debug_instances.push(rendering::DebugInstance {
                        pos_a: pa,
                        pos_b: pb,
                        color: [r, g, b, 0.55],
                        radius: 5.0 * (self.ui.skin_thickness / 3.0),
                        segment_type: 99,
                    });
                }
            }

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
                        health: 1.0,
                    });
                }
                if is_in_selected {
                    selected_bones.push(rendering::SdfBoneInstance {
                        pos_a: pa,
                        pos_b: pb,
                        radius: highlight_radius,
                        color: [1.0, 1.0, 1.0],
                        health: 1.0,
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
                    let health = entity_health_fraction
                        .get(&spring.node_a)
                        .copied()
                        .unwrap_or(1.0)
                        * spotlight_factor(spring.node_a);

                    if should_draw_sdf && bone_visible(pa, pb) {
                        sdf_bones.push(rendering::SdfBoneInstance {
                            pos_a: pa,
                            pos_b: pb,
                            radius: 4.0 * (self.ui.skin_thickness / 3.0) * growth_scale,
                            color,
                            health,
                        });
                    }
                    if should_draw_debug {
                        debug_instances.push(rendering::DebugInstance {
                            pos_a: pa,
                            pos_b: pb,
                            color: [0.246, 0.287, 0.434, 0.4],
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
                    let health = entity_health_fraction
                        .get(&spring.node_a)
                        .copied()
                        .unwrap_or(1.0)
                        * spotlight_factor(spring.node_a);
                    if should_draw_sdf && bone_visible(pa, pb) {
                        sdf_bones.push(rendering::SdfBoneInstance {
                            pos_a: pa,
                            pos_b: pb,
                            radius: 6.0 * (self.ui.skin_thickness / 3.0) * growth_scale,
                            color,
                            health,
                        });
                    }
                    if should_draw_debug {
                        debug_instances.push(rendering::DebugInstance {
                            pos_a: pa,
                            pos_b: pb,
                            color: [0.246, 0.287, 0.434, 0.4],
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
                let radius = (if spring.is_fin == 1 { 4.0 } else { 8.0 })
                    * (self.ui.skin_thickness / 3.0)
                    * growth_scale;

                let color = opt_color.map(|c| c.0).unwrap_or([0.8, 0.8, 0.8]);
                let health = entity_health_fraction
                    .get(&spring.node_a)
                    .copied()
                    .unwrap_or(1.0)
                    * spotlight_factor(spring.node_a);
                if should_draw_sdf && bone_visible(pa, pb) {
                    sdf_bones.push(rendering::SdfBoneInstance {
                        pos_a: pa,
                        pos_b: pb,
                        radius,
                        color,
                        health,
                    });
                }
                if should_draw_debug {
                    debug_instances.push(rendering::DebugInstance {
                        pos_a: pa,
                        pos_b: pb,
                        color: [0.246, 0.287, 0.434, 0.4],
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
                    color: [1.000, 0.665, 0.078, 1.0], // #FFD54F
                    radius: 2.5,
                    segment_type: 0,
                });
            }
            if should_draw_sdf && bone_visible(pos, pos) {
                sdf_bones.push(rendering::SdfBoneInstance {
                    pos_a: pos,
                    pos_b: pos,
                    radius: 2.5,
                    color: [1.000, 0.665, 0.078], // #FFD54F
                    health: 1.0,
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
                    health: 1.0,
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
                    health: 1.0,
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
                    color: [0.397, 0.539, 0.584, 1.0], // #A9C2C9
                    radius: 2.0,
                    segment_type: 0,
                });
            }
            if should_draw_sdf && bone_visible(pos, pos) {
                sdf_bones.push(rendering::SdfBoneInstance {
                    pos_a: pos,
                    pos_b: pos,
                    radius: 2.0,
                    color: [0.397, 0.539, 0.584], // #A9C2C9
                    health: 1.0,
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
                    health: 1.0,
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
                    health: 1.0,
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
                    color: [0.153, 0.136, 0.156, 1.0], // #6D676E
                    radius: 4.0,
                    segment_type: 0,
                });
            }
            if should_draw_sdf && bone_visible(pos, pos) {
                sdf_bones.push(rendering::SdfBoneInstance {
                    pos_a: pos,
                    pos_b: pos,
                    radius: 4.0,
                    color: [0.153, 0.136, 0.156], // #6D676E
                    health: 1.0,
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
                    health: 1.0,
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
                    health: 1.0,
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
                let (canvas_interact, acts) =
                    ui::render_ui(ctx, &mut self.app_state, &mut self.world, &mut self.ui);
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
            r: 0.001,
            g: 0.001,
            b: 0.004,
            a: 1.0,
        };
        if let Some(atmosphere) = self
            .world
            .ecs
            .get_resource::<metabolism::GlobalAtmosphere>()
        {
            let s = atmosphere.sunlight as f64;
            clear_color = wgpu::Color {
                r: 0.001 * (1.0 - s) + 0.010 * s,
                g: 0.001 * (1.0 - s) + 0.070 * s,
                b: 0.004 * (1.0 - s) + 0.184 * s,
                a: 1.0,
            };
        }

        let heatmap_state = self
            .world
            .ecs
            .get_resource::<ui::HeatmapState>()
            .cloned()
            .unwrap_or_default();
        let mut field_view_to_render: Option<&wgpu::TextureView> = None;

        // Use the cropped central viewport (not the full window) so the
        // heatmap's world-space<->screen-space conversion matches the
        // organism (sdf_skin) projection below and the two don't drift
        // apart ("parallax") when panning with a sidebar/toolbar open.
        let (screen_w, screen_h) = central_rect_px
            .map(|[_, _, w, h]| (w as f32, h as f32))
            .unwrap_or_else(|| {
                gpu.config
                    .as_ref()
                    .map(|c| (c.width as f32, c.height as f32))
                    .unwrap_or((1280.0, 720.0))
            });

        // Half-extent of the simulation world in world-space units — must
        // match `field_overlay.wgsl`'s `world_bounds` (below) exactly, since
        // that shader maps screen->world->grid-UV assuming this same value.
        // The Glucose/ATP splat step below maps organism positions into grid
        // space using this same constant so the two stay in registration;
        // using the viewport's pixel size there instead (as it previously
        // did) scaled the mapping by an arbitrary, resize-dependent factor,
        // which is what made the heatmap appear misaligned/tiled well
        // outside the actual world bounds.
        const WORLD_BOUNDS: f32 = 1500.0;

        // For Glucose/ATP, min/max are recomputed fresh below from this
        // frame's actual values (rather than using `heatmap_state`'s stored
        // min/max, which default to a fixed 0.0..1.0 that nothing updates —
        // organism glucose/ATP commonly run into the tens of thousands, so
        // normalizing against 0..1 clipped everything to the top of the
        // colormap instead of showing a gradient).
        let mut dynamic_min = heatmap_state.min_val;
        let mut dynamic_max = heatmap_state.max_val;

        if heatmap_state.active != ui::ActiveHeatmap::None {
            match heatmap_state.active {
                ui::ActiveHeatmap::Pheromones => {
                    if let Some(diffusion) = self.diffusion_compute.as_ref() {
                        field_view_to_render = Some(diffusion.current_layer_view(0));
                    }
                }
                ui::ActiveHeatmap::EnergyDensity => {
                    if let Some(diffusion) = self.diffusion_compute.as_ref() {
                        field_view_to_render = Some(diffusion.current_layer_view(1));
                    }
                }
                ui::ActiveHeatmap::O2 => {
                    if let Some(diffusion) = self.diffusion_compute.as_ref() {
                        field_view_to_render = Some(diffusion.current_layer_view(2));
                    }
                }
                ui::ActiveHeatmap::CO2 => {
                    if let Some(diffusion) = self.diffusion_compute.as_ref() {
                        field_view_to_render = Some(diffusion.current_layer_view(3));
                    }
                }
                ui::ActiveHeatmap::Glucose | ui::ActiveHeatmap::ATP => {
                    if let Some(splat_compute) = self.splat_compute.as_mut() {
                        let mut splats = Vec::new();
                        let mut sample_max = 0.0f32;
                        let mut query = self
                            .world
                            .ecs
                            .query::<(&physics::ParticleNode, &metabolism::ChemicalEconomy)>();
                        for (node, chem) in query.iter(&self.world.ecs) {
                            let value = if heatmap_state.active == ui::ActiveHeatmap::Glucose {
                                chem.glucose
                            } else {
                                chem.atp
                            };
                            sample_max = sample_max.max(value);

                            // Map world space to grid space — must use the
                            // same WORLD_BOUNDS the fragment shader assumes,
                            // not the viewport's pixel size (see comment on
                            // WORLD_BOUNDS above).
                            let grid_x = (node.position.x / WORLD_BOUNDS) * 128.0 + 128.0;
                            let grid_y = (-node.position.y / WORLD_BOUNDS) * 128.0 + 128.0;

                            splats.push(rendering::GpuSplat {
                                grid_pos: [grid_x, grid_y],
                                value,
                                grid_radius: 8.0,
                            });
                        }
                        splat_compute.step(&gpu.device, &gpu.queue, &splats);
                        field_view_to_render = Some(&splat_compute.view);
                        dynamic_min = 0.0;
                        dynamic_max = sample_max.max(1.0);
                    }
                }
                _ => {}
            }
        }

        if let (Some(field_renderer), Some(view_to_render)) =
            (self.field_renderer.as_ref(), field_view_to_render)
        {
            field_renderer.update_config(
                &gpu.queue,
                rendering::FieldConfig {
                    min_val: dynamic_min,
                    max_val: dynamic_max,
                    camera_pos: [self.ui.camera_pos.x, self.ui.camera_pos.y],
                    camera_zoom: self.ui.camera_zoom,
                    _pad0: 0,
                    screen_size: [screen_w, screen_h],
                    colormap: heatmap_state.colormap,
                    _pad: 0,
                    world_bounds: [WORLD_BOUNDS, WORLD_BOUNDS],
                },
            );

            field_renderer.render(
                &gpu.device,
                &mut encoder,
                &view,
                view_to_render,
                central_rect_px,
                clear_color,
            );
        } else if let Some(field_renderer) = self.field_renderer.as_ref() {
            // Render nothing but clear the screen
            field_renderer.update_config(
                &gpu.queue,
                rendering::FieldConfig {
                    min_val: 0.0,
                    max_val: -1.0, // Ensures range < 0.0001, alpha = 0.0
                    camera_pos: [self.ui.camera_pos.x, self.ui.camera_pos.y],
                    camera_zoom: self.ui.camera_zoom,
                    _pad0: 0,
                    screen_size: [screen_w, screen_h],
                    colormap: heatmap_state.colormap,
                    _pad: 0,
                    world_bounds: [WORLD_BOUNDS, WORLD_BOUNDS],
                },
            );
            if let Some(diffusion) = self.diffusion_compute.as_ref() {
                field_renderer.render(
                    &gpu.device,
                    &mut encoder,
                    &view,
                    diffusion.current_layer_view(0),
                    central_rect_px,
                    clear_color,
                );
            }
        }

        // Submit the field renderer (which clears the screen and draws the background) BEFORE
        // the other renderers, which rely on LoadOp::Load and submit their own encoders.
        gpu.queue.submit(std::iter::once(encoder.finish()));

        let (view_w, view_h) = (screen_w, screen_h);

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

        // Phase 5, SX-1e: `debug_instances` (Health/Disease/Category badges —
        // Priority 2/3/5 biological signals) now draws *before* the
        // hover/selection highlight, not after. Re-auditing the previous
        // order found it drew debug instances last, meaning a low-health
        // ring or disease badge would paint *over* (and could visually
        // obscure) the Priority-1 selection/hover outline wherever they
        // overlapped — a direct violation of "higher-priority signals must
        // always remain readable." Selection/hover now always paints last.
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

        // Screenshot/recording readback — must happen here, after the egui
        // pass has been submitted (so captured frames include the UI) but
        // before `output.present()` below, since `output.texture` is only
        // valid until it's presented.
        let capture_size = gpu
            .config
            .as_ref()
            .map(|c| (c.width, c.height))
            .unwrap_or((0, 0));
        let capture_format = gpu
            .config
            .as_ref()
            .map(|c| c.format)
            .unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb);

        if self.pending_screenshot && capture_size.0 > 0 && capture_size.1 > 0 {
            self.pending_screenshot = false;
            match crate::capture::read_texture_to_image(
                &gpu.device,
                &gpu.queue,
                &output.texture,
                capture_format,
                capture_size.0,
                capture_size.1,
            )
            .map(|img| crate::capture::save_screenshot(&img))
            {
                Some(Ok(path)) => self.ui.push_toast(
                    format!("Saved screenshot to {}", path.display()),
                    ui::ToastSeverity::Success,
                    3.0,
                ),
                Some(Err(e)) => {
                    tracing::error!("Failed to save screenshot: {e}");
                    self.ui.push_toast(
                        format!("Failed to save screenshot: {e}"),
                        ui::ToastSeverity::Error,
                        5.0,
                    );
                }
                None => tracing::error!("Screenshot readback produced no image"),
            }
        }

        if let Some(recording) = self.recording.as_mut() {
            if capture_size.0 > 0
                && capture_size.1 > 0
                && recording.last_capture.elapsed() >= crate::capture::CAPTURE_INTERVAL
            {
                if let Some(img) = crate::capture::read_texture_to_image(
                    &gpu.device,
                    &gpu.queue,
                    &output.texture,
                    capture_format,
                    capture_size.0,
                    capture_size.1,
                ) {
                    recording.frames.push(img);
                    recording.last_capture = std::time::Instant::now();
                }
            }
        }

        // Hit the recording cap — stop and save. Checked as a separate step
        // (rather than inline above) so `self.recording.take()` and
        // `self.ui.push_toast(...)` are plain disjoint-field accesses, not a
        // `&mut self` method call, which would conflict with the `gpu`
        // borrow (from `self.gpu.as_mut()`) still live in this scope.
        if matches!(&self.recording, Some(r) if r.frames.len() >= crate::capture::MAX_RECORDING_FRAMES)
        {
            let recording = self.recording.take().unwrap();
            self.ui.recording_active = false;
            self.ui.recording_started_at = None;
            crate::capture::finish_recording(&recording.frames, &mut self.ui);
        }

        output.present();

        self.handle_menu_actions(ui_actions);

        Ok(())
    }
}
