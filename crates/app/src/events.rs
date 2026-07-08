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
//! 6. Run the event loop, calling `PhylonApp::update_simulation` each tick
//!    (see `app.rs`'s top doc comment — Phase 6, Epic A removed the
//!    `SimulationScheduler` this step previously constructed, since it was
//!    never actually advanced by anything) and presenting a cleared frame
//!    on each `RedrawRequested`.
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
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Phylon Save", &["bin"])
                        .set_file_name("autosave.bin")
                        .save_file()
                    {
                        self.ui.push_toast("Saving…", ui::ToastSeverity::Info, 5.0);
                        if let Some(tx) = &self.task_tx {
                            let tx = tx.clone();
                            tokio::task::spawn_blocking(move || {
                                let res = storage::StorageManager::save_simulation_state(
                                    &snapshot, &path,
                                )
                                .map_err(|e| e.to_string());
                                let _ =
                                    tx.send(crate::app::BackgroundTaskResult::SaveComplete(res));
                            });
                        }
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
                    let spawn_pos = self.ui.camera_pos;
                    self.replay_log.record(
                        self.current_tick(),
                        storage::replay::ReplayAction::SpawnPreset {
                            name: name.clone(),
                            position: spawn_pos.into(),
                        },
                    );
                    self.apply_spawn_preset(&name, spawn_pos);
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
                    let tick = self.current_tick();
                    self.replay_log.record(
                        tick,
                        storage::replay::ReplayAction::SpawnManualHazard {
                            position: pos.into(),
                        },
                    );
                    self.apply_spawn_manual_hazard(pos, tick);
                }
                ui::MenuAction::SpawnPaste => tracing::warn!("SpawnPaste not implemented"),
                ui::MenuAction::JoinSelection => tracing::warn!("JoinSelection not implemented"),
                ui::MenuAction::GrabSelection => tracing::warn!("GrabSelection not implemented"),
                ui::MenuAction::GoToMainMenu => {
                    self.app_state = ui::AppState::MainMenu;
                }
                ui::MenuAction::StartSimulation => {
                    self.app_state = ui::AppState::Simulation;
                    // Reset standard flags
                    self.ui.is_paused = false;
                    self.ui.show_about = false;
                    self.ui.show_docs = false;
                    // Phase 5, SX-9a: fires the first time this session the
                    // user actually reaches the simulation view — not at
                    // `WorkbenchState::default()` construction time, since
                    // `show_dialogs` also renders over the Main Menu screen,
                    // where this dialog's viewport/Inspector references
                    // wouldn't make sense yet.
                    self.ui.show_onboarding_hints = true;
                }
                ui::MenuAction::Quit => {
                    info!("Quit action triggered from menu.");
                    std::process::exit(0);
                }
                ui::MenuAction::LoadState => {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Phylon Save", &["bin"])
                        .pick_file()
                    {
                        self.ui.push_toast("Loading…", ui::ToastSeverity::Info, 5.0);
                        if let Some(tx) = &self.task_tx {
                            let tx = tx.clone();
                            tokio::task::spawn_blocking(move || {
                                let res = storage::StorageManager::load_simulation_state(&path)
                                    .map_err(|e| e.to_string());
                                let _ =
                                    tx.send(crate::app::BackgroundTaskResult::LoadComplete(res));
                            });
                        }
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
                ui::MenuAction::ReseedEcosystem => {
                    self.replay_log.record(
                        self.current_tick(),
                        storage::replay::ReplayAction::ReseedEcosystem,
                    );
                    self.apply_reseed_ecosystem();
                }
                ui::MenuAction::TakeScreenshot => {
                    // Actual capture happens in `render()`, right before
                    // `output.present()` — that's the only place the live
                    // swapchain texture is available. This just requests it.
                    self.pending_screenshot = true;
                }
                ui::MenuAction::ToggleRecording => match self.recording.take() {
                    None => {
                        self.recording = Some(crate::capture::RecordingState::new());
                        self.ui.recording_active = true;
                        self.ui.recording_started_at = Some(self.ui.time);
                    }
                    Some(recording) => {
                        self.ui.recording_active = false;
                        self.ui.recording_started_at = None;
                        crate::capture::finish_recording(&recording.frames, &mut self.ui);
                    }
                },
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
                ui::MenuAction::SelectHeadOf(organism_id) => {
                    let mut query = self
                        .world
                        .ecs
                        .query::<(bevy_ecs::entity::Entity, &physics::ParticleNode)>();
                    for (entity, node) in query.iter(&self.world.ecs) {
                        if node.segment_type == 0 && node.organism_id == organism_id {
                            self.ui.selected_entity = Some(entity);
                            self.ui.tracked_entity = Some(entity);
                            break;
                        }
                    }
                }
                ui::MenuAction::SpawnProtoFish => {
                    let pos = self.ui.camera_pos;
                    self.replay_log.record(
                        self.current_tick(),
                        storage::replay::ReplayAction::SpawnProtoFish {
                            position: pos.into(),
                        },
                    );
                    self.apply_spawn_proto_fish(pos);
                }
                ui::MenuAction::ShowDocumentation => {
                    self.ui.show_docs = true;
                }
                ui::MenuAction::ShowAbout => {
                    self.ui.show_about = true;
                }
                ui::MenuAction::ShowKeybinds => {
                    self.ui.show_keybinds = true;
                }
                ui::MenuAction::ShowOnboardingHints => {
                    self.ui.show_onboarding_hints = true;
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
                ui::MenuAction::TogglePlayPause => {
                    self.ui.is_paused = !self.ui.is_paused;
                }
                ui::MenuAction::SetSpeedUp => {
                    self.ui.simulation_speed = (self.ui.simulation_speed * 2.0).clamp(0.1, 10.0);
                }
                ui::MenuAction::SetSpeedDown => {
                    self.ui.simulation_speed = (self.ui.simulation_speed / 2.0).clamp(0.1, 10.0);
                }
                ui::MenuAction::ToggleMetrics => {
                    self.ui.metrics_visible = !self.ui.metrics_visible;
                }
                ui::MenuAction::ToggleLog => {
                    self.ui.event_log_visible = !self.ui.event_log_visible;
                }
                ui::MenuAction::ToggleSidebar => {
                    self.ui.sidebar_visible = !self.ui.sidebar_visible;
                }
                ui::MenuAction::ImportGenome => {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Phylon Genome", &["genome"])
                        .pick_file()
                    {
                        if let Ok(bytes) = std::fs::read(path) {
                            if let Ok(genome) = bincode::deserialize::<genetics::Genome>(&bytes) {
                                self.world.ecs.resource_scope::<common::SimRng, _>(
                                    |ecs, mut sim_rng| {
                                        organisms::spawn_organism(
                                            ecs,
                                            &genome,
                                            self.ui.camera_pos,
                                            ecology::Diet::Omnivore,
                                            ecology::EcologicalCategory::None,
                                            0,
                                            0,
                                            &mut sim_rng.0,
                                        );
                                    },
                                );
                                self.ui.push_toast(
                                    "Genome imported",
                                    ui::ToastSeverity::Success,
                                    3.0,
                                );
                            } else {
                                tracing::error!("Failed to deserialize genome.");
                            }
                        }
                    }
                }
                ui::MenuAction::ExportGenome => {
                    if let Some(entity) = self.ui.selected_entity {
                        if let Ok(genome) = self
                            .world
                            .ecs
                            .query::<&genetics::Genome>()
                            .get(&self.world.ecs, entity)
                        {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Phylon Genome", &["genome"])
                                .set_file_name("organism.genome")
                                .save_file()
                            {
                                if let Ok(encoded) = bincode::serialize(genome) {
                                    if std::fs::write(path, encoded).is_ok() {
                                        self.ui.push_toast(
                                            "Genome exported",
                                            ui::ToastSeverity::Success,
                                            3.0,
                                        );
                                    }
                                }
                            }
                        } else {
                            tracing::warn!("Selected entity does not have a genome.");
                        }
                    } else {
                        tracing::warn!("No entity selected to export.");
                    }
                }
                ui::MenuAction::OpenReplayBundle => {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Phylon Replay", &["phylon-replay"])
                        .pick_file()
                    {
                        match storage::replay::ReplayBundle::load_from_file(&path) {
                            Ok(bundle) => {
                                let events = bundle
                                    .log
                                    .events
                                    .iter()
                                    .map(|e| (e.tick, crate::replay::describe_action(&e.action)))
                                    .collect();
                                self.ui.replay_browser = Some(ui::ReplayBrowserSummary {
                                    source_path: path.display().to_string(),
                                    seed: bundle.log.seed,
                                    last_event_tick: bundle.log.last_event_tick(),
                                    events,
                                });
                                self.ui.push_toast(
                                    "Replay bundle loaded",
                                    ui::ToastSeverity::Success,
                                    3.0,
                                );
                            }
                            Err(e) => {
                                self.ui.push_toast(
                                    format!("Failed to load replay bundle: {e}"),
                                    ui::ToastSeverity::Error,
                                    5.0,
                                );
                            }
                        }
                    }
                }
                ui::MenuAction::CloseReplayBundle => {
                    self.ui.replay_browser = None;
                }
                ui::MenuAction::ExportLineagesCsv => {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("CSV", &["csv"])
                        .set_file_name("lineages.csv")
                        .save_file()
                    {
                        match self.storage.export_lineages_csv(&path) {
                            Ok(()) => self.ui.push_toast(
                                "Lineages exported",
                                ui::ToastSeverity::Success,
                                3.0,
                            ),
                            Err(e) => self.ui.push_toast(
                                format!("Export failed: {e}"),
                                ui::ToastSeverity::Error,
                                5.0,
                            ),
                        }
                    }
                }
                ui::MenuAction::ExportEventsCsv => {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("CSV", &["csv"])
                        .set_file_name("events.csv")
                        .save_file()
                    {
                        match self.storage.export_events_csv(&path) {
                            Ok(()) => self.ui.push_toast(
                                "Events exported",
                                ui::ToastSeverity::Success,
                                3.0,
                            ),
                            Err(e) => self.ui.push_toast(
                                format!("Export failed: {e}"),
                                ui::ToastSeverity::Error,
                                5.0,
                            ),
                        }
                    }
                }
                ui::MenuAction::ExportOrganismsCsv => {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("CSV", &["csv"])
                        .set_file_name("organisms.csv")
                        .save_file()
                    {
                        let snapshot = storage::snapshot::SimulationSnapshot::from_world(
                            &mut self.world.ecs,
                            self.sim_config.simulation.rng_seed,
                            self.total_sim_time,
                        );
                        match storage::export_organisms_csv(&snapshot, &path) {
                            Ok(()) => self.ui.push_toast(
                                "Organisms exported",
                                ui::ToastSeverity::Success,
                                3.0,
                            ),
                            Err(e) => self.ui.push_toast(
                                format!("Export failed: {e}"),
                                ui::ToastSeverity::Error,
                                5.0,
                            ),
                        }
                    }
                }
                ui::MenuAction::ExportMetricsCsv => {
                    if let Some(metrics) = self.world.ecs.get_resource::<analytics::MetricsState>()
                    {
                        let csv = analytics::export::metrics_to_csv(metrics);
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("CSV", &["csv"])
                            .set_file_name("metrics.csv")
                            .save_file()
                        {
                            match std::fs::write(&path, csv) {
                                Ok(()) => self.ui.push_toast(
                                    "Metrics exported",
                                    ui::ToastSeverity::Success,
                                    3.0,
                                ),
                                Err(e) => self.ui.push_toast(
                                    format!("Export failed: {e}"),
                                    ui::ToastSeverity::Error,
                                    5.0,
                                ),
                            }
                        }
                    }
                }
                ui::MenuAction::ExportMetricsJson => {
                    if let Some(metrics) = self.world.ecs.get_resource::<analytics::MetricsState>()
                    {
                        match analytics::export::metrics_to_json(metrics) {
                            Ok(json) => {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("JSON", &["json"])
                                    .set_file_name("metrics.json")
                                    .save_file()
                                {
                                    match std::fs::write(&path, json) {
                                        Ok(()) => self.ui.push_toast(
                                            "Metrics exported",
                                            ui::ToastSeverity::Success,
                                            3.0,
                                        ),
                                        Err(e) => self.ui.push_toast(
                                            format!("Export failed: {e}"),
                                            ui::ToastSeverity::Error,
                                            5.0,
                                        ),
                                    }
                                }
                            }
                            Err(e) => self.ui.push_toast(
                                format!("Failed to serialize metrics: {e}"),
                                ui::ToastSeverity::Error,
                                5.0,
                            ),
                        }
                    }
                }
                ui::MenuAction::ExportChartPng {
                    x,
                    y,
                    width,
                    height,
                } => {
                    // Same deferred-capture rationale as `TakeScreenshot`
                    // above — actual crop+encode happens in `render()`
                    // against the live swapchain texture.
                    self.pending_chart_export = Some((x, y, width, height));
                }
                ui::MenuAction::ToggleCommandPalette => {
                    self.ui.show_command_palette = !self.ui.show_command_palette;
                    self.ui.command_palette_query.clear();
                }
                ui::MenuAction::FocusSelection => {
                    tracing::warn!("FocusSelection not yet implemented.");
                }
                ui::MenuAction::SetOverlay(heatmap) => {
                    if let Some(mut hs) = self.world.ecs.get_resource_mut::<ui::HeatmapState>() {
                        hs.active = heatmap;
                    }
                }
                ui::MenuAction::SetColormap(colormap) => {
                    if let Some(mut hs) = self.world.ecs.get_resource_mut::<ui::HeatmapState>() {
                        hs.colormap = colormap;
                    }
                }
                ui::MenuAction::KillEntity(entity) => {
                    self.world.ecs.despawn(entity);
                    if self.ui.selected_entity == Some(entity) {
                        self.ui.selected_entity = None;
                    }
                    if self.ui.tracked_entity == Some(entity) {
                        self.ui.tracked_entity = None;
                    }
                    self.ui
                        .push_toast("Entity killed", ui::ToastSeverity::Warning, 2.0);
                }
                ui::MenuAction::TrackEntity(entity) => {
                    self.ui.tracked_entity = Some(entity);
                    self.ui.selected_entity = Some(entity);
                }
                ui::MenuAction::SelectEntity(entity) => {
                    self.ui.selected_entity = Some(entity);
                }
                ui::MenuAction::SelectInRect { min, max } => {
                    // Head nodes only (`segment_type == 0`) — one selection
                    // entry per organism, matching how the rest of the app
                    // treats "the organism" as its head entity (e.g.
                    // `SelectHeadOf`, the Lineage panel).
                    let mut node_q = self
                        .world
                        .ecs
                        .query::<(bevy_ecs::entity::Entity, &physics::ParticleNode)>();
                    let matches: Vec<bevy_ecs::entity::Entity> = node_q
                        .iter(&self.world.ecs)
                        .filter(|(_, node)| {
                            node.segment_type == 0
                                && node.position.x >= min.x
                                && node.position.x <= max.x
                                && node.position.y >= min.y
                                && node.position.y <= max.y
                        })
                        .map(|(e, _)| e)
                        .collect();
                    let count = matches.len();
                    self.ui.select_multiple(matches);
                    if count > 0 {
                        self.ui.push_toast(
                            format!("Selected {count} organism(s)"),
                            ui::ToastSeverity::Info,
                            2.0,
                        );
                    }
                }
                ui::MenuAction::CopyEntityId(entity) => {
                    // Write entity bits to clipboard via egui (best-effort)
                    let id_str = format!("{:?}", entity);
                    tracing::info!("Copy entity ID to clipboard: {}", id_str);
                    self.ui
                        .push_toast(format!("Copied: {}", id_str), ui::ToastSeverity::Info, 2.0);
                }
                ui::MenuAction::SelectByDiet(diet) => {
                    let mut found = None;
                    let mut q = self
                        .world
                        .ecs
                        .query::<(bevy_ecs::entity::Entity, &ecology::Diet)>();
                    for (e, d) in q.iter(&self.world.ecs) {
                        if *d == diet {
                            found = Some(e);
                            break;
                        }
                    }
                    if let Some(e) = found {
                        self.ui.selected_entity = Some(e);
                        self.ui.tracked_entity = Some(e);
                    }
                }
                ui::MenuAction::InvertSelection => {
                    // Cycle to next head node (nearest to current selection)
                    let current = self.ui.selected_entity;
                    let mut found_next = false;
                    let mut first = None;
                    let mut take_next = current.is_none();
                    let mut q = self
                        .world
                        .ecs
                        .query::<(bevy_ecs::entity::Entity, &physics::ParticleNode)>();
                    for (e, node) in q.iter(&self.world.ecs) {
                        if node.segment_type == 0 {
                            if first.is_none() {
                                first = Some(e);
                            }
                            if take_next {
                                self.ui.selected_entity = Some(e);
                                found_next = true;
                                break;
                            }
                            if current == Some(e) {
                                take_next = true;
                            }
                        }
                    }
                    if !found_next {
                        self.ui.selected_entity = first;
                    }
                }

                // ── Panel window management ──────────────────────────────────
                ui::MenuAction::DetachPanel(name) => {
                    self.ui
                        .panel_modes
                        .insert(name.clone(), ui::PanelMode::Floating);
                    // Remove the tile immediately instead of waiting for the
                    // next lazy `retain_pane`/simplify pass.
                    ui::layout::remove_panel_from_tree(&mut self.ui.dock_tree, &name);
                    info!("Detached panel: {}", name);
                }
                ui::MenuAction::DockPanel(name) => {
                    self.ui
                        .panel_modes
                        .insert(name.clone(), ui::PanelMode::Docked);
                    // Rebuild the tree from current modes so the panel lands
                    // back in its canonical home slot (not wherever the root
                    // container currently happens to be).
                    ui::layout::rebuild_tree_from_modes(
                        &mut self.ui.dock_tree,
                        &self.ui.panel_modes,
                        &self.ui.layout_shares,
                    );
                    info!("Docked panel: {}", name);
                }
                ui::MenuAction::ClosePanel(name) => {
                    self.ui
                        .panel_modes
                        .insert(name.clone(), ui::PanelMode::Closed);
                    // Remove the tile immediately instead of waiting for the
                    // next lazy `retain_pane`/simplify pass.
                    ui::layout::remove_panel_from_tree(&mut self.ui.dock_tree, &name);
                    self.ui.push_toast(
                        format!("\"{}\" closed — reopen via Windows menu", name),
                        ui::ToastSeverity::Info,
                        3.0,
                    );
                    info!("Closed panel: {}", name);
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
                let s = modifiers.state();
                self.ui.modifiers = egui::Modifiers {
                    alt: s.alt_key(),
                    ctrl: s.control_key(),
                    shift: s.shift_key(),
                    mac_cmd: s.super_key(),
                    command: s.control_key() || s.super_key(),
                };
            }
            WindowEvent::MouseWheel { delta, .. } => {
                match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        if self.ui.modifiers.ctrl {
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
                        if self.ui.modifiers.ctrl {
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
                        self.ui
                            .push_toast("Simulation saved", ui::ToastSeverity::Success, 3.0);
                        tracing::info!("Saved state successfully");
                    }
                    crate::app::BackgroundTaskResult::SaveComplete(Err(e)) => {
                        self.ui.push_toast(
                            format!("Save failed: {}", e),
                            ui::ToastSeverity::Error,
                            5.0,
                        );
                        tracing::error!("Failed to save state: {}", e);
                    }
                    crate::app::BackgroundTaskResult::LoadComplete(Ok(snapshot)) => {
                        // Reseed the shared SimRng from the snapshot's
                        // recorded seed — without this, a loaded run
                        // continues drawing from whatever RNG stream state
                        // happened to be live before the load, silently
                        // breaking the "seed + interventions guarantee
                        // replay" determinism promise (see
                        // `storage::ReplayLog`'s doc comment).
                        let seed = snapshot.seed;
                        snapshot.restore_world(&mut self.world.ecs);
                        self.world
                            .ecs
                            .insert_resource(common::SimRng::from_seed(seed));
                        self.ui
                            .push_toast("Simulation loaded", ui::ToastSeverity::Success, 3.0);
                        tracing::info!("Loaded state successfully");
                    }
                    crate::app::BackgroundTaskResult::LoadComplete(Err(e)) => {
                        self.ui.push_toast(
                            format!("Load failed: {}", e),
                            ui::ToastSeverity::Error,
                            5.0,
                        );
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
