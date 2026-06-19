//! # Phylon UI
//!
//! `egui`-based research interface: entity inspector, analytics dashboard,
//! experiment controls, replay timeline, and debug overlay toggles.
//!
//! The UI crate renders on top of the simulation frame using egui's wgpu
//! backend. It reads from the simulation state (via shared snapshots) and
//! publishes intervention events to the event bus.

#![warn(missing_docs)]
#![warn(clippy::all)]

/// Errors from the UI subsystem.
#[derive(Debug, thiserror::Error)]
pub enum UiError {
    /// An egui widget encountered an invalid state.
    #[error("UI state error: {message}")]
    StateError {
        /// Description of the invalid state.
        message: String,
    },
}

impl common::PhylonError for UiError {}

/// The active tab in the primary sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidebarTab {
    /// Inspect single organisms and the physical environment
    #[default]
    Inspector,
    /// View neural networks and genotypes
    Genetics,
    /// Global metrics and population charts
    Analytics,
}

/// Contains the screen-space rect of the transparent canvas area and the
/// unified touch/mouse/trackpad gesture interactions performed on it.
#[derive(Debug, Clone, Copy)]
pub struct CanvasInteraction {
    /// The screen-space bounding rect of the central canvas panel.
    pub rect: egui::Rect,
    /// True if the user tapped/clicked on the canvas this frame.
    pub clicked: bool,
    /// The screen-space coordinates of the tap/click, if `clicked` is true.
    pub click_pos: Option<egui::Pos2>,
    /// The screen-space delta for a pan/drag gesture this frame.
    pub drag_delta: egui::Vec2,
    /// The scale factor for a pinch-to-zoom or scroll-zoom gesture this frame (1.0 = no change).
    pub zoom_delta: f32,
}

impl Default for CanvasInteraction {
    fn default() -> Self {
        Self {
            rect: egui::Rect::NOTHING,
            clicked: false,
            click_pos: None,
            drag_delta: egui::Vec2::ZERO,
            zoom_delta: 1.0,
        }
    }
}

/// Actions triggered from the UI Menu Bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    /// Save the simulation state to disk.
    SaveState,
    /// Load a simulation state from disk.
    LoadState,
    /// Undo the last action.
    Undo,
    /// Redo the last undone action.
    Redo,
    /// Advance the simulation by one tick while paused.
    StepForward,
    /// Reset the simulation to default organisms.
    Reset,
    /// Select all or cycle through organisms.
    SelectAll,
    /// Clear the current selection.
    Deselect,
    /// Spawn a new proto-fish under the camera.
    SpawnProtoFish,
    /// Show the Phylon documentation.
    ShowDocumentation,
    /// Show the About Phylon dialog.
    ShowAbout,
    /// Zoom camera in.
    CameraZoomIn,
    /// Zoom camera out.
    CameraZoomOut,
    /// Reset camera view.
    CameraHome,
}

/// Renders the main immediate-mode user interface.
///
/// Returns a `CanvasInteraction` containing the screen-space `Rect` of the
/// transparent `CentralPanel` (for viewport sizing) and the unified
/// touch/mouse interactions (clicks, drags, zooms) generated on it.
///
/// `debug_structural` is mutated by a checkbox in the Inspector sidebar.
/// When `true`, the caller should render raw physics quads instead of the SDF
/// organic skin.
#[allow(clippy::too_many_arguments)]
pub fn render_ui(
    ctx: &egui::Context,
    world: &mut world::World,
    camera_pos: common::Vec2,
    camera_zoom: f32,
    selected_entity: &mut Option<bevy_ecs::entity::Entity>,
    tracked_entity: &mut Option<bevy_ecs::entity::Entity>,
    debug_structural: &mut bool,
    bone_line_thickness: &mut f32,
    active_tab: &mut SidebarTab,
    simulation_speed: &mut f32,
    is_paused: &mut bool,
    show_about: &mut bool,
    show_docs: &mut bool,
    show_vision_cones: &mut bool,
) -> (CanvasInteraction, Vec<MenuAction>) {
    let mut actions = Vec::new();

    let shortcut_save = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::S);
    let shortcut_load = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::O);
    let shortcut_undo = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Z);
    let shortcut_redo =
        egui::KeyboardShortcut::new(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::Z);
    let shortcut_play_pause = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Space);
    let shortcut_step = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::ArrowRight);
    let shortcut_reset = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::R);
    let shortcut_select_all = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::A);
    let shortcut_deselect = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Escape);
    let shortcut_spawn = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::P);

    if ctx.input_mut(|i| i.consume_shortcut(&shortcut_save)) {
        actions.push(MenuAction::SaveState);
    }
    if ctx.input_mut(|i| i.consume_shortcut(&shortcut_load)) {
        actions.push(MenuAction::LoadState);
    }
    if ctx.input_mut(|i| i.consume_shortcut(&shortcut_undo)) {
        actions.push(MenuAction::Undo);
    }
    if ctx.input_mut(|i| i.consume_shortcut(&shortcut_redo)) {
        actions.push(MenuAction::Redo);
    }
    if ctx.input_mut(|i| i.consume_shortcut(&shortcut_play_pause)) {
        *is_paused = !*is_paused;
    }
    if ctx.input_mut(|i| i.consume_shortcut(&shortcut_step)) {
        actions.push(MenuAction::StepForward);
    }
    if ctx.input_mut(|i| i.consume_shortcut(&shortcut_reset)) {
        actions.push(MenuAction::Reset);
    }
    if ctx.input_mut(|i| i.consume_shortcut(&shortcut_select_all)) {
        actions.push(MenuAction::SelectAll);
    }
    if ctx.input_mut(|i| i.consume_shortcut(&shortcut_deselect)) {
        actions.push(MenuAction::Deselect);
    }
    if ctx.input_mut(|i| i.consume_shortcut(&shortcut_spawn)) {
        actions.push(MenuAction::SpawnProtoFish);
    }

    // Hardcode camera zoom keys
    if ctx.input(|i| i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals)) {
        actions.push(MenuAction::CameraZoomIn);
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Minus)) {
        actions.push(MenuAction::CameraZoomOut);
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Home) || i.key_pressed(egui::Key::Num0)) {
        actions.push(MenuAction::CameraHome);
    }

    egui::Window::new("About Phylon")
        .open(show_about)
        .show(ctx, |ui| {
            ui.heading("Phylon Artificial Life Simulator");
            ui.label("A GPU-accelerated ALife simulation.");
            ui.label("Version: 0.1.0");
        });

    egui::Window::new("Documentation")
        .open(show_docs)
        .show(ctx, |ui| {
            ui.heading("Documentation");
            ui.label("Welcome to Phylon. The core architecture uses continuous space and compute shaders.");
            ui.label("Features:");
            ui.label("- Hox-driven procedural generation");
            ui.label("- Neural network control via CTRNNs");
            ui.label("- Diffusion based metabolism");
        });

    // ── Top menu bar ───────────────────────────────────────────────────────
    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui
                    .add(
                        egui::Button::new("Save State")
                            .shortcut_text(ctx.format_shortcut(&shortcut_save)),
                    )
                    .clicked()
                {
                    actions.push(MenuAction::SaveState);
                }
                if ui
                    .add(
                        egui::Button::new("Load State")
                            .shortcut_text(ctx.format_shortcut(&shortcut_load)),
                    )
                    .clicked()
                {
                    actions.push(MenuAction::LoadState);
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    std::process::exit(0);
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui
                    .add(
                        egui::Button::new("Undo")
                            .shortcut_text(ctx.format_shortcut(&shortcut_undo)),
                    )
                    .clicked()
                {
                    actions.push(MenuAction::Undo);
                }
                if ui
                    .add(
                        egui::Button::new("Redo")
                            .shortcut_text(ctx.format_shortcut(&shortcut_redo)),
                    )
                    .clicked()
                {
                    actions.push(MenuAction::Redo);
                }
            });
            ui.menu_button("Simulation", |ui| {
                if ui
                    .add(
                        egui::Button::new(if *is_paused { "Play" } else { "Pause" })
                            .shortcut_text(ctx.format_shortcut(&shortcut_play_pause)),
                    )
                    .clicked()
                {
                    *is_paused = !*is_paused;
                }
                if ui
                    .add(
                        egui::Button::new("Step Forward")
                            .shortcut_text(ctx.format_shortcut(&shortcut_step)),
                    )
                    .clicked()
                {
                    actions.push(MenuAction::StepForward);
                }
                if ui
                    .add(
                        egui::Button::new("Reset")
                            .shortcut_text(ctx.format_shortcut(&shortcut_reset)),
                    )
                    .clicked()
                {
                    actions.push(MenuAction::Reset);
                }
            });
            ui.menu_button("View", |ui| {
                ui.checkbox(debug_structural, "Debug Structural View");
                ui.checkbox(show_vision_cones, "Show Vision Cones");
            });
            ui.menu_button("Selection", |ui| {
                if ui
                    .add(
                        egui::Button::new("Select All")
                            .shortcut_text(ctx.format_shortcut(&shortcut_select_all)),
                    )
                    .clicked()
                {
                    actions.push(MenuAction::SelectAll);
                }
                if ui
                    .add(
                        egui::Button::new("Deselect")
                            .shortcut_text(ctx.format_shortcut(&shortcut_deselect)),
                    )
                    .clicked()
                {
                    actions.push(MenuAction::Deselect);
                }
            });
            ui.menu_button("Tools", |ui| {
                if ui
                    .add(
                        egui::Button::new("Spawn Proto-Fish")
                            .shortcut_text(ctx.format_shortcut(&shortcut_spawn)),
                    )
                    .clicked()
                {
                    actions.push(MenuAction::SpawnProtoFish);
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("Documentation").clicked() {
                    actions.push(MenuAction::ShowDocumentation);
                }
                if ui.button("About").clicked() {
                    actions.push(MenuAction::ShowAbout);
                }
            });

            ui.separator();
            ui.label("Speed:");
            ui.add(
                egui::Slider::new(simulation_speed, 0.1..=10.0)
                    .text("x")
                    .logarithmic(true),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // right-to-left means items are placed from right edge toward left edge.
                // we want the order visual left-to-right to be: "Cam info", "-", "Home", "+"
                // so we add "+", "Home", "-", then the label.

                if ui.button("+").on_hover_text("Zoom In (+/=)").clicked() {
                    actions.push(MenuAction::CameraZoomIn);
                }
                if ui
                    .button("Home")
                    .on_hover_text("Reset Camera (Home/0)")
                    .clicked()
                {
                    actions.push(MenuAction::CameraHome);
                }
                if ui.button("-").on_hover_text("Zoom Out (-)").clicked() {
                    actions.push(MenuAction::CameraZoomOut);
                }

                let track_str = if let Some(e) = tracked_entity {
                    format!(" — Tracking {:?}", e)
                } else {
                    String::new()
                };
                ui.label(format!(
                    "Cam: ({:.0}, {:.0})  ×{:.1}{}",
                    camera_pos.x, camera_pos.y, camera_zoom, track_str
                ));
            });
        });
    });

    // ── Activity bar (narrow icon strip, far left) ─────────────────────────
    egui::SidePanel::left("activity_bar")
        .exact_width(40.0)
        .resizable(false)
        .show(ctx, |ui| {
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                if ui
                    .selectable_label(*active_tab == SidebarTab::Inspector, "🔍")
                    .on_hover_text("Inspector")
                    .clicked()
                {
                    *active_tab = SidebarTab::Inspector;
                }
                ui.add_space(4.0);
                if ui
                    .selectable_label(*active_tab == SidebarTab::Genetics, "🧬")
                    .on_hover_text("Genetics")
                    .clicked()
                {
                    *active_tab = SidebarTab::Genetics;
                }
                ui.add_space(4.0);
                if ui
                    .selectable_label(*active_tab == SidebarTab::Analytics, "📈")
                    .on_hover_text("Analytics")
                    .clicked()
                {
                    *active_tab = SidebarTab::Analytics;
                }
            });
        });

    // ── Primary sidebar ────────────────────────────────────────────────────
    egui::SidePanel::left("primary_sidebar")
        .resizable(true)
        .default_width(260.0)
        .show(ctx, |ui| {
            match active_tab {
                SidebarTab::Inspector => {
                    ui.heading("Inspector");
                    ui.separator();
                    ui.checkbox(debug_structural, "🔲 Debug Structural View");
                    if *debug_structural {
                        ui.add(
                            egui::Slider::new(bone_line_thickness, 0.5..=5.0)
                                .text("Bone Line Thickness"),
                        );
                    }
                    ui.checkbox(show_vision_cones, "👁 Show Vision Cones");
                    ui.separator();
                    if let Some(entity) = *selected_entity {
                        ui.label(
                            egui::RichText::new(format!("Selected: {:?}", entity))
                                .heading()
                                .color(egui::Color32::LIGHT_GREEN),
                        );
                        let mut is_tracked = *tracked_entity == Some(entity);
                        if ui.checkbox(&mut is_tracked, "Track Selected").changed() {
                            if is_tracked {
                                *tracked_entity = Some(entity);
                            } else {
                                if *tracked_entity == Some(entity) {
                                    *tracked_entity = None;
                                }
                            }
                        }

                        ui.separator(); // Physics node
                        let mut node_q = world.ecs.query::<&physics::ParticleNode>();
                        if let Ok(node) = node_q.get(&world.ecs, entity) {
                            egui::CollapsingHeader::new("⚙ Physics Node")
                                .default_open(true)
                                .show(ui, |ui| {
                                    let seg_name = match node.segment_type {
                                        0 => "Head",
                                        1 => "Torso",
                                        2 => "Muscle",
                                        3 => "Tail",
                                        4 => "Fin",
                                        _ => "Unknown",
                                    };
                                    ui.label(format!("Segment  : {seg_name}"));
                                    ui.label(format!(
                                        "Position : ({:.1}, {:.1})",
                                        node.position.x, node.position.y
                                    ));
                                    ui.label(format!(
                                        "Velocity : ({:.2}, {:.2})",
                                        node.velocity.x, node.velocity.y
                                    ));
                                    ui.label(format!("Mass     : {:.2}", node.mass));
                                });
                        }

                        // Metabolism — Energy
                        let mut energy_q = world.ecs.query::<&metabolism::Energy>();
                        let mut age_q = world.ecs.query::<&metabolism::Age>();
                        let mut meta_q = world.ecs.query::<&metabolism::Metabolism>();
                        let has_meta = energy_q.get(&world.ecs, entity).is_ok();

                        if has_meta {
                            egui::CollapsingHeader::new("🧬 Biology")
                                .default_open(true)
                                .show(ui, |ui| {
                                    if let Ok(en) = energy_q.get(&world.ecs, entity) {
                                        let pct = en.current / en.max;
                                        ui.label(format!(
                                            "Energy : {:.1} / {:.1}",
                                            en.current, en.max
                                        ));
                                        ui.add(
                                            egui::ProgressBar::new(pct)
                                                .text(format!("{:.0}%", pct * 100.0)),
                                        );
                                    }
                                    if let Ok(age) = age_q.get(&world.ecs, entity) {
                                        ui.label(format!(
                                            "Age    : {} / {} ticks",
                                            age.ticks, age.max_lifespan
                                        ));
                                    }
                                    if let Ok(meta) = meta_q.get(&world.ecs, entity) {
                                        ui.label(format!("Mass   : {:.2}", meta.mass));
                                        ui.label(format!("Rate   : {:.3} /tick", meta.base_rate));
                                    }
                                });
                        }

                        // Biological components (ecology)
                        let mut diet_q = world.ecs.query::<&ecology::Diet>();
                        if let Ok(diet) = diet_q.get(&world.ecs, entity) {
                            ui.label(format!("Diet   : {:?}", diet));
                        }
                        let mut category_q = world.ecs.query::<&ecology::EcologicalCategory>();
                        if let Ok(cat) = category_q.get(&world.ecs, entity) {
                            ui.label(format!("Category: {:?}", cat));
                        }

                        // Entity Graph / Segment Tree
                        egui::CollapsingHeader::new("🌳 Body Structure")
                            .default_open(true)
                            .show(ui, |ui| {
                                // Build adjacency list from springs
                                let mut adj: std::collections::HashMap<
                                    bevy_ecs::entity::Entity,
                                    Vec<(bevy_ecs::entity::Entity, physics::Spring)>,
                                > = std::collections::HashMap::new();
                                let mut spring_q = world.ecs.query::<&physics::Spring>();
                                for spring in spring_q.iter(&world.ecs) {
                                    adj.entry(spring.node_a)
                                        .or_default()
                                        .push((spring.node_b, spring.clone()));
                                    adj.entry(spring.node_b)
                                        .or_default()
                                        .push((spring.node_a, spring.clone()));
                                }

                                // Find the root of this connected component (the Head node)
                                let mut visited = std::collections::HashSet::new();
                                let mut component = Vec::new();
                                let mut queue = std::collections::VecDeque::new();
                                queue.push_back(entity);
                                visited.insert(entity);

                                while let Some(curr) = queue.pop_front() {
                                    component.push(curr);
                                    if let Some(neighbors) = adj.get(&curr) {
                                        for (neighbor, _) in neighbors {
                                            if visited.insert(*neighbor) {
                                                queue.push_back(*neighbor);
                                            }
                                        }
                                    }
                                }

                                // Try to find the head (segment_type == 0) in the component
                                let mut root = entity; // fallback
                                let mut node_q = world.ecs.query::<&physics::ParticleNode>();
                                for &node_entity in &component {
                                    if let Ok(n) = node_q.get(&world.ecs, node_entity) {
                                        if n.segment_type == 0 {
                                            // Head
                                            root = node_entity;
                                            break;
                                        }
                                    }
                                }

                                let mut tree_visited = std::collections::HashSet::new();
                                draw_segment_tree(
                                    ui,
                                    root,
                                    &adj,
                                    &world.ecs,
                                    &mut tree_visited,
                                    selected_entity,
                                );
                            });
                    } else {
                        ui.label(
                            egui::RichText::new("Click a node to inspect")
                                .italics()
                                .color(egui::Color32::GRAY),
                        );
                    }
                }
                SidebarTab::Genetics => {
                    ui.heading("Genetics");
                    ui.separator();
                    if let Some(entity) = *selected_entity {
                        // Find the head node for this organism to get the genome
                        let mut adj: std::collections::HashMap<
                            bevy_ecs::entity::Entity,
                            Vec<bevy_ecs::entity::Entity>,
                        > = std::collections::HashMap::new();
                        let mut spring_q = world.ecs.query::<&physics::Spring>();
                        for spring in spring_q.iter(&world.ecs) {
                            adj.entry(spring.node_a).or_default().push(spring.node_b);
                            adj.entry(spring.node_b).or_default().push(spring.node_a);
                        }

                        let mut head_node = None;
                        let mut queue = std::collections::VecDeque::new();
                        let mut visited = std::collections::HashSet::new();
                        queue.push_back(entity);
                        visited.insert(entity);

                        let mut repro_q = world.ecs.query::<&reproduction::ReproductionStrategy>();
                        let mut growth_q = world.ecs.query::<&organisms::GrowthState>();

                        while let Some(curr) = queue.pop_front() {
                            if repro_q.get(&world.ecs, curr).is_ok()
                                || growth_q.get(&world.ecs, curr).is_ok()
                            {
                                head_node = Some(curr);
                                break;
                            }
                            if let Some(neighbors) = adj.get(&curr) {
                                for neighbor in neighbors {
                                    if visited.insert(*neighbor) {
                                        queue.push_back(*neighbor);
                                    }
                                }
                            }
                        }

                        let mut found_genome = false;
                        if let Some(head) = head_node {
                            let mut genome_ref = None;
                            if let Ok(repro) = repro_q.get(&world.ecs, head) {
                                genome_ref = Some(repro.genome.clone());
                            } else if let Ok(growth) = growth_q.get(&world.ecs, head) {
                                genome_ref = Some(growth.genome.clone());
                            }

                            let mut pending_mutation = None;

                            if let Some(genome) = genome_ref {
                                found_genome = true;
                                ui.label(
                                    egui::RichText::new(format!("Genome ID: {}", genome.id.0))
                                        .strong(),
                                );
                                ui.label(format!("Ploidy: {:?}", genome.ploidy));
                                ui.label(format!("Origin: {:?}", genome.origin));

                                ui.add_space(8.0);
                                if genome.hox.is_some() {
                                    ui.label(
                                        egui::RichText::new("⚠️ This organism's morphology and wiring is hardcoded by its Hox Sequence. CPPN mutations are disabled.")
                                            .color(egui::Color32::YELLOW),
                                    );
                                } else {
                                    ui.horizontal(|ui| {
                                        if ui.button("🎲 Mutate Add Node").clicked() {
                                            pending_mutation = Some("add_node");
                                        }
                                        if ui.button("🎲 Mutate Add Connection").clicked() {
                                            pending_mutation = Some("add_conn");
                                        }
                                        if ui.button("🎲 Mutate Weights").clicked() {
                                            pending_mutation = Some("mutate_weight");
                                        }
                                    });
                                }
                                ui.separator();

                                if let Some(hox) = &genome.hox {
                                    ui.horizontal(|ui| {
                                        ui.heading("Hox Sequence");
                                        ui.add_space(8.0);
                                        let mut color = [hox.color[0], hox.color[1], hox.color[2]];
                                        ui.color_edit_button_rgb(&mut color);
                                    });
                                    egui::ScrollArea::vertical()
                                        .id_salt("hox_scroll")
                                        .max_height(200.0)
                                        .show(ui, |ui| {
                                            for (i, gene) in hox.genes.iter().enumerate() {
                                                ui.group(|ui| {
                                                    ui.label(
                                                        egui::RichText::new(format!(
                                                            "[{}] {:?}",
                                                            i, gene.segment
                                                        ))
                                                        .strong(),
                                                    );
                                                    if gene.branching_signal > 0.0 {
                                                        ui.label(format!(
                                                            "Branching Signal: {:.2}",
                                                            gene.branching_signal
                                                        ));
                                                    }
                                                    if gene.actuation_amplitude > 0.0 {
                                                        ui.label(format!(
                                                            "Actuation Amp: {:.2}",
                                                            gene.actuation_amplitude
                                                        ));
                                                        ui.label(format!(
                                                            "Actuation Phase: {:.2}",
                                                            gene.actuation_phase
                                                        ));
                                                    }
                                                });
                                            }
                                        });
                                } else {
                                    ui.label("No explicit Hox sequence (CPPN driven).");
                                }

                                ui.separator();
                                ui.heading("CPPN Topology");
                                ui.label(format!("Nodes: {}", genome.nodes.len()));
                                ui.label(format!("Connections: {}", genome.connections.len()));

                                // Draw CPPN Graph
                                let (response, painter) = ui.allocate_painter(
                                    egui::vec2(ui.available_width(), 300.0),
                                    egui::Sense::hover(),
                                );
                                let rect = response.rect;
                                painter.rect_filled(rect, 4.0, egui::Color32::from_black_alpha(50));

                                // Find max layer
                                let max_layer =
                                    genome.nodes.iter().map(|n| n.layer).max().unwrap_or(0);

                                // Group nodes by layer
                                let mut layer_counts = std::collections::HashMap::new();
                                let mut node_positions = std::collections::HashMap::new();

                                for n in &genome.nodes {
                                    *layer_counts.entry(n.layer).or_insert(0) += 1;
                                }

                                let mut current_layer_idx = std::collections::HashMap::new();

                                for (i, node) in genome.nodes.iter().enumerate() {
                                    let layer_idx =
                                        *current_layer_idx.entry(node.layer).or_insert(0);
                                    let count = *layer_counts.get(&node.layer).unwrap();

                                    let x = if max_layer == 0 {
                                        rect.center().x
                                    } else {
                                        rect.left()
                                            + 20.0
                                            + (rect.width() - 40.0)
                                                * (node.layer as f32 / max_layer as f32)
                                    };

                                    let y = if count == 1 {
                                        rect.center().y
                                    } else {
                                        rect.top()
                                            + 20.0
                                            + (rect.height() - 40.0)
                                                * (layer_idx as f32 / (count - 1) as f32)
                                    };

                                    node_positions.insert(i, egui::pos2(x, y));
                                    current_layer_idx.insert(node.layer, layer_idx + 1);
                                }

                                // Draw edges
                                for conn in &genome.connections {
                                    if !conn.enabled {
                                        continue;
                                    }
                                    if let (Some(&p1), Some(&p2)) = (
                                        node_positions.get(&conn.source),
                                        node_positions.get(&conn.target),
                                    ) {
                                        let color = if conn.weight > 0.0 {
                                            egui::Color32::from_rgba_premultiplied(0, 255, 0, 150)
                                        } else {
                                            egui::Color32::from_rgba_premultiplied(255, 0, 0, 150)
                                        };
                                        let thickness = (conn.weight.abs() * 2.0).clamp(1.0, 5.0);
                                        painter.line_segment([p1, p2], (thickness, color));
                                    }
                                }

                                // Draw nodes
                                for (i, node) in genome.nodes.iter().enumerate() {
                                    if let Some(&pos) = node_positions.get(&i) {
                                        let fill = if node.layer == 0 {
                                            egui::Color32::LIGHT_BLUE
                                        } else if node.layer == max_layer {
                                            egui::Color32::LIGHT_RED
                                        } else {
                                            egui::Color32::GRAY
                                        };
                                        painter.circle_filled(pos, 6.0, fill);
                                        painter.circle_stroke(
                                            pos,
                                            6.0,
                                            (1.0, egui::Color32::WHITE),
                                        );

                                        // Tooltip for activation/bias
                                        if response
                                            .hover_pos()
                                            .is_some_and(|p| p.distance(pos) < 6.0)
                                        {
                                            egui::show_tooltip(
                                                ctx,
                                                ui.layer_id(),
                                                ui.id().with("tooltip"),
                                                |ui| {
                                                    ui.label(format!("Node {}", i));
                                                    ui.label(format!("Layer: {}", node.layer));
                                                    ui.label(format!(
                                                        "Activation: {:?}",
                                                        node.activation
                                                    ));
                                                    ui.label(format!("Bias: {:.2}", node.bias));
                                                },
                                            );
                                        }
                                    }
                                }
                            }

                            // Apply pending mutation
                            if let Some(action) = pending_mutation {
                                drop(repro_q);
                                drop(growth_q);
                                drop(spring_q);

                                let mut repro_mut =
                                    world.ecs.query::<&mut reproduction::ReproductionStrategy>();
                                let mut growth_mut =
                                    world.ecs.query::<&mut organisms::GrowthState>();

                                if let Ok(mut r) = repro_mut.get_mut(&mut world.ecs, head) {
                                    let mut next_innov = r.genome.connections.len() * 100;
                                    match action {
                                        "add_node" => r.genome.mutate_add_node(&mut next_innov),
                                        "add_conn" => {
                                            r.genome.mutate_add_connection(&mut next_innov)
                                        }
                                        "mutate_weight" => r.genome.mutate_weight(),
                                        _ => {}
                                    }
                                } else if let Ok(mut g) = growth_mut.get_mut(&mut world.ecs, head) {
                                    let mut next_innov = g.genome.connections.len() * 100;
                                    match action {
                                        "add_node" => g.genome.mutate_add_node(&mut next_innov),
                                        "add_conn" => {
                                            g.genome.mutate_add_connection(&mut next_innov)
                                        }
                                        "mutate_weight" => g.genome.mutate_weight(),
                                        _ => {}
                                    }
                                }
                            }
                        }

                        if !found_genome {
                            ui.label("Selected entity has no Genome component.");
                        }
                    } else {
                        ui.label(
                            egui::RichText::new("Select an organism's head to view its genome.")
                                .italics(),
                        );
                    }
                }
                SidebarTab::Analytics => {
                    ui.heading("Analytics");
                    ui.separator();
                    if let Some(metrics) = world.ecs.get_resource::<analytics::MetricsState>() {
                        ui.label(egui::RichText::new("Compute Profiling").strong());
                        ui.label(egui::RichText::new("(CPU-side estimate)").italics().small());

                        egui::Frame::none()
                            .fill(egui::Color32::from_black_alpha(20))
                            .inner_margin(8.0)
                            .rounding(4.0)
                            .show(ui, |ui| {
                                for pass in &metrics.compute_profiles {
                                    ui.horizontal(|ui| {
                                        ui.label(&pass.name);
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.label(
                                                    egui::RichText::new(format!(
                                                        "{:.2} ms",
                                                        pass.duration_ms
                                                    ))
                                                    .monospace(),
                                                );
                                            },
                                        );
                                    });
                                }
                            });

                        ui.add_space(16.0);
                        ui.label(egui::RichText::new("Global Simulation Metrics").strong());
                        ui.label(format!("Total Entities: {}", world.ecs.entities().len()));
                        ui.label(format!("Smoothed FPS: {:.1}", metrics.smoothed_fps));
                    } else {
                        ui.label("Analytics data not available.");
                    }
                }
            }
        });

    // ── Status bar (bottom strip) ──────────────────────────────────────────
    egui::TopBottomPanel::bottom("status_bar")
        .exact_height(24.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let entity_count = world.ecs.entities().len();
                let fps = world
                    .ecs
                    .get_resource::<analytics::MetricsState>()
                    .map(|m| m.smoothed_fps)
                    .unwrap_or(0.0);

                let sim_time = world
                    .ecs
                    .get_resource::<analytics::MetricsState>()
                    .map(|m| m.sim_time)
                    .unwrap_or(0.0);
                let tick_count = (sim_time / 0.016).round() as u64;

                ui.label(format!("⏱ Tick: {}", tick_count));
                ui.separator();
                ui.label(format!("⚡ FPS: {:.0}", fps));
                ui.separator();
                ui.label(format!("🦠 Entities: {}", entity_count));
                ui.separator();
                ui.label(if *debug_structural {
                    "👁 Mode: Structural"
                } else {
                    "👁 Mode: SDF Skin"
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Mem: <100MB"); // Placeholder, proper system memory integration later
                    ui.separator();
                    ui.label("🟢 Engine Online");
                });
            });
        });

    // ── Bottom panel — Metrics plots ───────────────────────────────────────
    egui::TopBottomPanel::bottom("bottom_panel")
        .resizable(true)
        .default_height(180.0)
        .show(ctx, |ui| {
            ui.heading("Output / Metrics");
            ui.separator();

            if let Some(metrics) = world.ecs.get_resource::<analytics::MetricsState>() {
                let pop_pts: egui_plot::PlotPoints =
                    metrics.population_history.iter().copied().collect();
                let fps_pts: egui_plot::PlotPoints = metrics.fps_history.iter().copied().collect();

                ui.columns(2, |cols| {
                    cols[0].label("Population");
                    egui_plot::Plot::new("pop_plot")
                        .height(120.0)
                        .show(&mut cols[0], |plot_ui| {
                            plot_ui.line(egui_plot::Line::new(pop_pts).name("entities"));
                        });

                    cols[1].label("FPS");
                    egui_plot::Plot::new("fps_plot")
                        .height(120.0)
                        .show(&mut cols[1], |plot_ui| {
                            plot_ui.line(egui_plot::Line::new(fps_pts).name("fps"));
                        });
                });
            } else {
                ui.label("Metrics not yet available.");
            }
        });

    // ── Central panel (transparent — simulation renders underneath) ────────
    let central = egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(egui::Color32::TRANSPARENT)
                .inner_margin(8.0)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(120)))
                .rounding(4.0),
        )
        .show(ctx, |ui| {
            ui.allocate_response(ui.available_size(), egui::Sense::click_and_drag())
        });

    let interact_response = central.inner;
    let zoom_delta = ctx.input(|i| i.zoom_delta());

    // Render vision cones if enabled
    if *show_vision_cones {
        let mut query = world
            .ecs
            .query::<(&physics::ParticleNode, &sensing::HeadVision)>();
        let mut painter = ctx.layer_painter(egui::LayerId::background());
        painter.set_clip_rect(interact_response.rect);

        let screen_center = interact_response.rect.center();
        let to_screen = |pos: common::Vec2| {
            egui::pos2(
                screen_center.x + (pos.x - camera_pos.x) * camera_zoom,
                screen_center.y + (pos.y - camera_pos.y) * camera_zoom,
            )
        };

        for (node, vision) in query.iter(&world.ecs) {
            let origin = to_screen(node.position);

            let fwd = vision.last_forward;
            // Angle of the forward direction
            let base_angle = fwd.y.atan2(fwd.x);
            let half_fov = vision.fov / 2.0;

            // Generate an arc polygon
            let segments = 16;
            let mut points = Vec::with_capacity(segments + 2);
            points.push(origin);
            for i in 0..=segments {
                let t = i as f32 / segments as f32;
                let angle = base_angle - half_fov + (vision.fov * t);
                let x = node.position.x + angle.cos() * vision.range;
                let y = node.position.y + angle.sin() * vision.range;
                points.push(to_screen(common::Vec2::new(x, y)));
            }

            painter.add(egui::Shape::convex_polygon(
                points,
                egui::Color32::from_rgba_premultiplied(0, 255, 255, 30),
                egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(0, 255, 255, 80)),
            ));
        }
    }

    (
        CanvasInteraction {
            rect: central.response.rect,
            clicked: interact_response.clicked(),
            click_pos: interact_response.interact_pointer_pos(),
            drag_delta: interact_response.drag_delta(),
            zoom_delta,
        },
        actions,
    )
}

fn draw_segment_tree(
    ui: &mut egui::Ui,
    current_node: bevy_ecs::entity::Entity,
    adj: &std::collections::HashMap<
        bevy_ecs::entity::Entity,
        Vec<(bevy_ecs::entity::Entity, physics::Spring)>,
    >,
    world: &bevy_ecs::world::World,
    visited: &mut std::collections::HashSet<bevy_ecs::entity::Entity>,
    selected_entity: &mut Option<bevy_ecs::entity::Entity>,
) {
    if visited.contains(&current_node) {
        return;
    }
    visited.insert(current_node);

    let Some(node) = world.get::<physics::ParticleNode>(current_node) else {
        return;
    };

    let seg_name = match node.segment_type {
        0 => "Head",
        1 => "Torso",
        2 => "Muscle",
        3 => "Tail",
        4 => "Fin",
        _ => "Unknown",
    };

    // Find children
    let empty = Vec::new();
    let neighbors = adj.get(&current_node).unwrap_or(&empty);
    let mut children = Vec::new();
    for (neighbor, spring) in neighbors {
        if !visited.contains(neighbor) {
            children.push((*neighbor, spring.clone()));
        }
    }

    let label = format!("{:?} ({})", current_node, seg_name);
    let is_selected = *selected_entity == Some(current_node);

    if children.is_empty() {
        if ui.selectable_label(is_selected, label).clicked() {
            *selected_entity = Some(current_node);
        }
    } else {
        let header = egui::CollapsingHeader::new(label).default_open(true);

        let response = header.show(ui, |ui| {
            for (child, spring) in children {
                let constraint_name = match spring.constraint_type {
                    physics::ConstraintType::Elastic => "Elastic",
                    physics::ConstraintType::Rigid => "Rigid",
                    physics::ConstraintType::Passive => "Passive",
                    physics::ConstraintType::Rotational => "Rotational",
                };

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("↳ {}", constraint_name))
                            .small()
                            .color(egui::Color32::GRAY),
                    );
                    if spring.actuation_amplitude > 0.0 {
                        ui.label(
                            egui::RichText::new(format!(
                                "(amp: {:.1}, ph: {:.1})",
                                spring.actuation_amplitude, spring.actuation_phase
                            ))
                            .small()
                            .color(egui::Color32::from_rgb(200, 150, 100)),
                        );
                    }
                });

                draw_segment_tree(ui, child, adj, world, visited, selected_entity);
            }
        });

        if response.header_response.clicked() {
            *selected_entity = Some(current_node);
        }
    }
}
