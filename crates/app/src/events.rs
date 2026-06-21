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

use std::sync::Arc;

use tracing::{error, info};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

use crate::app::PhylonApp;

impl PhylonApp {
    pub(crate) fn handle_menu_actions(&mut self, actions: Vec<ui::MenuAction>) {
        for action in actions {
            match action {
                ui::MenuAction::SaveState => {
                    let snapshot = storage::snapshot::SimulationSnapshot::from_world(
                        &mut self.world.ecs,
                        self.sim_config.simulation.rng_seed,
                        self.total_sim_time,
                    );
                    self.ui.active_toast = Some(("Saving...".to_string(), 0.0));
                    if let Some(tx) = &self.task_tx {
                        let tx = tx.clone();
                        tokio::task::spawn_blocking(move || {
                            let path = std::path::Path::new("data/autosave.bin");
                            let res =
                                storage::StorageManager::save_simulation_state(&snapshot, path)
                                    .map_err(|e| e.to_string());
                            let _ = tx.send(crate::app::BackgroundTaskResult::SaveComplete(res));
                        });
                    }
                }
                ui::MenuAction::DeleteSelection => {
                    if let Some(entity) = self.ui.selected_entity {
                        self.world.ecs.despawn(entity);
                        self.ui.selected_entity = None;
                        if self.ui.tracked_entity == Some(entity) {
                            self.ui.tracked_entity = None;
                        }
                    }
                }
                ui::MenuAction::ToggleStationary => {
                    if let Some(entity) = self.ui.selected_entity {
                        if let Ok(mut node) = self
                            .world
                            .ecs
                            .query::<&mut physics::ParticleNode>()
                            .get_mut(&mut self.world.ecs, entity)
                        {
                            node.is_fixed = !node.is_fixed;
                        }
                    }
                }
                ui::MenuAction::DuplicateSelection => {
                    tracing::warn!("DuplicateSelection not implemented")
                }
                ui::MenuAction::SpawnPreset(name) => {
                    let preset_opt = organisms::sandbox::PresetDefinition::standard_presets()
                        .into_iter()
                        .find(|p| p.name == name);

                    if let Some(preset) = preset_opt {
                        let spawn_pos = self.ui.camera_pos;
                        if preset.evolvable {
                            // Evolvable presets get a HoxSequence
                            let hox = match name.as_str() {
                                "Herbivore (Evolvable)" => {
                                    genetics::HoxSequence::worm(6, [0.3, 0.8, 0.3])
                                }
                                "Hunter (Evolvable)" => {
                                    genetics::HoxSequence::fish(5, 2, [0.8, 0.2, 0.2])
                                }
                                "Edible Plant (Evolvable)" => {
                                    genetics::HoxSequence::worm(2, [0.2, 0.9, 0.2])
                                }
                                _ => genetics::HoxSequence::worm(4, [0.5, 0.5, 0.5]),
                            };
                            let genome = genetics::Genome::new_hox_driven(
                                genetics::GenomeId(0), // Would normally be a unique ID
                                common::EntityId(0),
                                hox,
                            );

                            let diet = preset.diet.unwrap_or(ecology::Diet::Herbivore);
                            let category =
                                preset.category.unwrap_or(ecology::EcologicalCategory::None);

                            // Spawn the organism
                            organisms::spawn_organism(
                                &mut self.world.ecs,
                                &genome,
                                spawn_pos,
                                diet,
                                category,
                                0,
                                0,
                            );

                            // We would attach the sandbox traits to the root node if possible,
                            // but spawn_organism doesn't return the head node easily right now.
                            // We'll leave the marker traits for later or add them to all nodes.
                        } else {
                            // Non-evolvable structures get a fixed static node topology.
                            // For Membrane Seed or Structure Node, just spawn a single node.
                            let seg_type = if preset.traits.is_membrane_seed { 1 } else { 0 };
                            let color = if preset.traits.is_membrane_seed {
                                [0.5, 0.5, 0.9]
                            } else {
                                [0.7, 0.7, 0.7]
                            };

                            let mut node = physics::ParticleNode::new(spawn_pos, 5.0, seg_type);
                            node.is_fixed = preset.traits.fixable;
                            let entity = self
                                .world
                                .ecs
                                .spawn((
                                    node,
                                    organisms::OrganismColor(color),
                                    preset.traits, // Attach SandboxTraits
                                ))
                                .id();

                            // Attach biological components so Inspector can view it
                            self.world.ecs.entity_mut(entity).insert((
                                metabolism::ChemicalEconomy {
                                    glucose: 10000.0,
                                    o2: 10000.0,
                                    co2: 0.0,
                                    atp: 10000.0,
                                    max_glucose: 100000.0,
                                    max_o2: 10000.0,
                                    max_co2: 10000.0,
                                    max_atp: 100000.0,
                                },
                                metabolism::Age {
                                    ticks: 0,
                                    max_lifespan: 10000,
                                },
                            ));
                        }
                    }
                }
                ui::MenuAction::GenerateHexMesh {
                    cols,
                    rows,
                    spacing,
                    stiffness,
                    is_fixed,
                } => {
                    organisms::sandbox::generate_hex_mesh(
                        &mut self.world.ecs,
                        self.ui.camera_pos,
                        cols,
                        rows,
                        spacing,
                        stiffness,
                        is_fixed,
                    );
                }
                ui::MenuAction::SpawnManualHazard => {
                    let pos = self.ui.camera_pos;
                    let tick = (self.total_sim_time / 0.016).round() as u64;
                    let mut manager = self
                        .world
                        .ecs
                        .resource_mut::<ecology::catastrophe::CatastropheManager>();
                    manager.spawn_hazard(common::Tick(tick), pos);
                }
                ui::MenuAction::SpawnPaste => tracing::warn!("SpawnPaste not implemented"),
                ui::MenuAction::JoinSelection => tracing::warn!("JoinSelection not implemented"),
                ui::MenuAction::GrabSelection => tracing::warn!("GrabSelection not implemented"),
                ui::MenuAction::GoToMainMenu => {
                    self.ui.app_state = ui::AppState::MainMenu;
                }
                ui::MenuAction::StartSimulation => {
                    self.ui.app_state = ui::AppState::Simulation;
                    // Reset standard flags
                    self.ui.is_paused = false;
                    self.ui.show_about = false;
                    self.ui.show_docs = false;
                }
                ui::MenuAction::Quit => {
                    info!("Quit action triggered from menu.");
                    std::process::exit(0);
                }
                ui::MenuAction::LoadState => {
                    self.ui.active_toast = Some(("Loading...".to_string(), 0.0));
                    if let Some(tx) = &self.task_tx {
                        let tx = tx.clone();
                        tokio::task::spawn_blocking(move || {
                            let path = std::path::Path::new("data/autosave.bin");
                            let res = storage::StorageManager::load_simulation_state(path)
                                .map_err(|e| e.to_string());
                            let _ = tx.send(crate::app::BackgroundTaskResult::LoadComplete(res));
                        });
                    }
                }
                ui::MenuAction::Undo => {
                    tracing::warn!("Undo not yet implemented fully.");
                }
                ui::MenuAction::Redo => {
                    tracing::warn!("Redo not yet implemented fully.");
                }
                ui::MenuAction::StepForward => {
                    self.accumulated_time += 1.0;
                }
                ui::MenuAction::Reset | ui::MenuAction::ReseedEcosystem => {
                    // Despawn all entities
                    let entities: Vec<_> = self.world.ecs.iter_entities().map(|e| e.id()).collect();
                    for entity in entities {
                        self.world.ecs.despawn(entity);
                    }

                    // Reset tracking
                    self.ui.selected_entity = None;
                    self.ui.tracked_entity = None;

                    // Clear lineage tracker
                    if let Some(mut tracker) = self
                        .world
                        .ecs
                        .get_resource_mut::<evolution::LineageTracker>()
                    {
                        *tracker = evolution::LineageTracker::new();
                    }

                    // Respawn defaults
                    let mut tracker = evolution::LineageTracker::new();
                    let mut global_tracker = genetics::GlobalInnovationTracker::default();
                    crate::app::seed_ecosystem(
                        &mut self.world.ecs,
                        &mut tracker,
                        &mut global_tracker,
                    );
                    self.world.ecs.insert_resource(tracker);
                    self.world.ecs.insert_resource(global_tracker);
                }
                ui::MenuAction::SelectAll => {
                    // Just select the first head we find
                    let mut query = self
                        .world
                        .ecs
                        .query::<(bevy_ecs::entity::Entity, &physics::ParticleNode)>();
                    for (entity, node) in query.iter(&self.world.ecs) {
                        if node.segment_type == 0 {
                            // Head
                            self.ui.selected_entity = Some(entity);
                            self.ui.tracked_entity = Some(entity);
                            break;
                        }
                    }
                }
                ui::MenuAction::Deselect => {
                    self.ui.selected_entity = None;
                    self.ui.tracked_entity = None;
                }
                ui::MenuAction::SpawnProtoFish => {
                    let fish_hox = genetics::HoxSequence::fish(5, 2, [0.25, 0.60, 0.90]);
                    let fish_genome = genetics::Genome::new_hox_driven(
                        genetics::GenomeId(100),
                        common::EntityId(0),
                        fish_hox,
                    );
                    organisms::spawn_organism(
                        &mut self.world.ecs,
                        &fish_genome,
                        self.ui.camera_pos,
                        ecology::Diet::Carnivore,
                        ecology::EcologicalCategory::None,
                        0,
                        0,
                    );
                }
                ui::MenuAction::ShowDocumentation => {
                    self.ui.show_docs = true;
                }
                ui::MenuAction::ShowAbout => {
                    self.ui.show_about = true;
                }
                ui::MenuAction::CameraZoomIn => {
                    self.ui.camera_zoom *= 1.1;
                    self.ui.camera_zoom = self.ui.camera_zoom.clamp(0.1, 10.0);
                }
                ui::MenuAction::CameraZoomOut => {
                    self.ui.camera_zoom /= 1.1;
                    self.ui.camera_zoom = self.ui.camera_zoom.clamp(0.1, 10.0);
                }
                ui::MenuAction::CameraHome => {
                    self.ui.camera_pos = common::Vec2::new(0.0, 0.0);
                    self.ui.camera_zoom = 1.0;
                    self.ui.tracked_entity = None;
                }
            }
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// winit ApplicationHandler impl
// ────────────────────────────────────────────────────────────────────────────

impl ApplicationHandler for PhylonApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attrs = Window::default_attributes()
            .with_title(&self.sim_config.render.window_title)
            .with_inner_size(LogicalSize::new(
                self.sim_config.render.window_width,
                self.sim_config.render.window_height,
            ));

        let window = match event_loop.create_window(window_attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                error!("Failed to create window: {e}");
                event_loop.exit();
                return;
            }
        };

        if let Err(e) = self.init_gpu(window) {
            error!("Failed to initialise GPU: {e:#}");
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(egui_state) = &mut self.egui_state {
            if let Some(window) = &self.window {
                let _response = egui_state.on_window_event(window, &event);
                if _response.consumed {
                    // Only return early if egui consumed the event specifically (e.g. text input),
                    // since we now handle primary interactions inside the render loop via egui's output.
                    return;
                }
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                info!("Window close requested — exiting");
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                self.resize(new_size);
            }
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        physical_key,
                        state: winit::event::ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                use winit::keyboard::{KeyCode, PhysicalKey};
                let pan_speed = 10.0 / self.ui.camera_zoom;
                match physical_key {
                    PhysicalKey::Code(KeyCode::KeyW) | PhysicalKey::Code(KeyCode::ArrowUp) => {
                        self.ui.camera_pos.y += pan_speed;
                        self.ui.tracked_entity = None;
                    }
                    PhysicalKey::Code(KeyCode::KeyS) | PhysicalKey::Code(KeyCode::ArrowDown) => {
                        self.ui.camera_pos.y -= pan_speed;
                        self.ui.tracked_entity = None;
                    }
                    PhysicalKey::Code(KeyCode::KeyA) | PhysicalKey::Code(KeyCode::ArrowLeft) => {
                        self.ui.camera_pos.x -= pan_speed;
                        self.ui.tracked_entity = None;
                    }
                    PhysicalKey::Code(KeyCode::KeyD) | PhysicalKey::Code(KeyCode::ArrowRight) => {
                        self.ui.camera_pos.x += pan_speed;
                        self.ui.tracked_entity = None;
                    }
                    // Zoom with + and -
                    PhysicalKey::Code(KeyCode::Equal) | PhysicalKey::Code(KeyCode::NumpadAdd) => {
                        self.ui.camera_zoom *= 1.1;
                        self.ui.camera_zoom = self.ui.camera_zoom.clamp(0.1, 10.0);
                    }
                    PhysicalKey::Code(KeyCode::Minus)
                    | PhysicalKey::Code(KeyCode::NumpadSubtract) => {
                        self.ui.camera_zoom /= 1.1;
                        self.ui.camera_zoom = self.ui.camera_zoom.clamp(0.1, 10.0);
                    }
                    _ => {}
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.ui.modifiers = modifiers.state();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        if self.ui.modifiers.control_key() {
                            // Zoom with Ctrl + Scroll
                            if y > 0.0 {
                                self.ui.camera_zoom *= 1.1;
                            } else if y < 0.0 {
                                self.ui.camera_zoom /= 1.1;
                            }
                        } else {
                            // Pan
                            self.ui.camera_pos.x -= x * 20.0 / self.ui.camera_zoom;
                            self.ui.camera_pos.y += y * 20.0 / self.ui.camera_zoom;
                        }
                    }
                    winit::event::MouseScrollDelta::PixelDelta(p) => {
                        if self.ui.modifiers.control_key() {
                            // Zoom
                            let zoom_factor = 1.0 + (p.y as f32 * 0.01);
                            if zoom_factor > 0.0 {
                                self.ui.camera_zoom *= zoom_factor;
                            }
                        } else {
                            // Touchpad two-finger swipe: pan
                            self.ui.camera_pos.x -= p.x as f32 / self.ui.camera_zoom;
                            self.ui.camera_pos.y += p.y as f32 / self.ui.camera_zoom;
                        }
                    }
                }
                self.ui.camera_zoom = self.ui.camera_zoom.clamp(0.1, 10.0);
            }

            WindowEvent::RedrawRequested => {
                if let Err(e) = self.render() {
                    error!("Render error: {e:#}");
                    event_loop.exit();
                }

                // Process pending clicks that require mutably borrowing self
                if let Some(click_pos) = self.ui.pending_click.take() {
                    let dims = self
                        .gpu
                        .as_ref()
                        .and_then(|g| g.config.as_ref())
                        .map(|c| (c.width as f32, c.height as f32));
                    if let Some((gpu_w, gpu_h)) = dims {
                        let selected = self.pick_entity(click_pos, gpu_w, gpu_h);
                        self.ui.selected_entity = selected;
                        self.ui.tracked_entity = selected;
                    }
                }

                let dims = self
                    .gpu
                    .as_ref()
                    .and_then(|g| g.config.as_ref())
                    .map(|c| (c.width as f32, c.height as f32));
                if let Some((gpu_w, gpu_h)) = dims {
                    if let Some(pos) = self.ui.current_hover_pos {
                        self.ui.hovered_entity = self.pick_entity(pos, gpu_w, gpu_h);
                    } else {
                        self.ui.hovered_entity = None;
                    }
                }

                // Request the next frame immediately.
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(rx) = &self.task_rx {
            while let Ok(res) = rx.try_recv() {
                match res {
                    crate::app::BackgroundTaskResult::SaveComplete(Ok(())) => {
                        self.ui.active_toast = None;
                        tracing::info!("Saved state to data/autosave.bin");
                    }
                    crate::app::BackgroundTaskResult::SaveComplete(Err(e)) => {
                        self.ui.active_toast = None;
                        tracing::error!("Failed to save state: {}", e);
                    }
                    crate::app::BackgroundTaskResult::LoadComplete(Ok(snapshot)) => {
                        snapshot.restore_world(&mut self.world.ecs);
                        self.ui.active_toast = None;
                        tracing::info!("Loaded state from data/autosave.bin");
                    }
                    crate::app::BackgroundTaskResult::LoadComplete(Err(e)) => {
                        self.ui.active_toast = None;
                        tracing::error!("Failed to load state: {}", e);
                    }
                }
            }
        }

        // Request a redraw every time the event loop is about to go idle
        // so the simulation keeps ticking even without user input.
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
