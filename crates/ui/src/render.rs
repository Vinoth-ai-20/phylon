use crate::types::*;
use crate::utils::*;

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
    app_state: &mut AppState,
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
    _hovered_entity: Option<bevy_ecs::entity::Entity>,
    quit_confirm_time: &mut Option<f64>,
    main_menu_confirm_time: &mut Option<f64>,
    spectator_mode: &mut bool,
    last_spectator_switch_time: &mut f64,
) -> (CanvasInteraction, Vec<MenuAction>) {
    let mut actions = Vec::new();

    let shortcut_save = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::S);
    let shortcut_load = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::O);
    // Undo/Redo are manually handled via key_pressed when focus is none
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

    if *app_state == AppState::MainMenu {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() / 4.0);
                ui.heading(egui::RichText::new("PHYLON").size(64.0).strong());
                ui.add_space(40.0);

                let btn_size = egui::vec2(200.0, 40.0);

                if ui
                    .add_sized(
                        btn_size,
                        egui::Button::new(egui::RichText::new("New").size(24.0)),
                    )
                    .clicked()
                {
                    actions.push(MenuAction::StartSimulation);
                }
                ui.add_space(10.0);

                if ui
                    .add_sized(
                        btn_size,
                        egui::Button::new(egui::RichText::new("Continue").size(24.0)),
                    )
                    .clicked()
                {
                    // Placeholder for when Save/Load is fully implemented
                    actions.push(MenuAction::StartSimulation);
                }
                ui.add_space(10.0);

                ui.allocate_ui_with_layout(
                    btn_size,
                    egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                    |ui| {
                        ui.menu_button(egui::RichText::new("Open Recent").size(24.0), |ui| {
                            if ui.button("save_01.ron").clicked() {
                                actions.push(MenuAction::LoadState);
                                actions.push(MenuAction::StartSimulation);
                            }
                            if ui.button("save_02.ron").clicked() {
                                actions.push(MenuAction::LoadState);
                                actions.push(MenuAction::StartSimulation);
                            }
                        });
                    },
                );
                ui.add_space(10.0);

                if ui
                    .add_sized(
                        btn_size,
                        egui::Button::new(egui::RichText::new("Load").size(24.0)),
                    )
                    .clicked()
                {
                    actions.push(MenuAction::LoadState);
                }
                ui.add_space(10.0);

                if ui
                    .add_sized(
                        btn_size,
                        egui::Button::new(egui::RichText::new("Settings").size(24.0)),
                    )
                    .clicked()
                {
                    *active_tab = SidebarTab::Settings;
                    actions.push(MenuAction::StartSimulation);
                }
                ui.add_space(10.0);

                if ui
                    .add_sized(
                        btn_size,
                        egui::Button::new(egui::RichText::new("Quit").size(24.0)),
                    )
                    .clicked()
                {
                    actions.push(MenuAction::Quit);
                }
            });
        });

        // Return early with empty interaction
        return (CanvasInteraction::default(), actions);
    }

    if !ctx.wants_keyboard_input() {
        if ctx.input(|i| i.key_pressed(egui::Key::Z)) {
            actions.push(MenuAction::Undo);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Y)) {
            actions.push(MenuAction::Redo);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::G)) {
            actions.push(MenuAction::GrabSelection);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::X)) {
            actions.push(MenuAction::DeleteSelection);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::C)) {
            actions.push(MenuAction::DuplicateSelection);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::V)) {
            actions.push(MenuAction::SpawnPaste);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F)) {
            actions.push(MenuAction::ToggleStationary);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::J)) {
            actions.push(MenuAction::JoinSelection);
        }
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
                if ui.button("Settings").clicked() {
                    *active_tab = SidebarTab::Settings;
                }

                let current_time = ui.input(|i| i.time);

                if let Some(t) = *main_menu_confirm_time {
                    if current_time - t < 3.0 {
                        if ui.button("Click again to confirm Main Menu").clicked() {
                            actions.push(MenuAction::GoToMainMenu);
                            *main_menu_confirm_time = None;
                        }
                    } else {
                        *main_menu_confirm_time = None;
                        if ui.button("Main Menu").clicked() {
                            *main_menu_confirm_time = Some(current_time);
                        }
                    }
                } else {
                    if ui.button("Main Menu").clicked() {
                        *main_menu_confirm_time = Some(current_time);
                    }
                }

                if let Some(t) = *quit_confirm_time {
                    if current_time - t < 3.0 {
                        if ui.button("Click again to confirm Quit").clicked() {
                            actions.push(MenuAction::Quit);
                            *quit_confirm_time = None;
                        }
                    } else {
                        *quit_confirm_time = None;
                        if ui.button("Quit").clicked() {
                            *quit_confirm_time = Some(current_time);
                        }
                    }
                } else {
                    if ui.button("Quit").clicked() {
                        *quit_confirm_time = Some(current_time);
                    }
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui
                    .add(egui::Button::new("Undo").shortcut_text("Z"))
                    .clicked()
                {
                    actions.push(MenuAction::Undo);
                }
                if ui
                    .add(egui::Button::new("Redo").shortcut_text("Y"))
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
                    .button(egui_remixicon::icons::HOME_LINE)
                    .on_hover_text("Reset Camera (Home/0)")
                    .clicked()
                {
                    actions.push(MenuAction::CameraHome);
                }
                if ui.button("-").on_hover_text("Zoom Out (-)").clicked() {
                    actions.push(MenuAction::CameraZoomOut);
                }

                let track_str = if let Some(e) = tracked_entity {
                    format!(" - Tracking {:?}", e)
                } else {
                    String::new()
                };

                ui.add_space(8.0);
                ui.checkbox(
                    spectator_mode,
                    format!("{} Spectator", egui_remixicon::icons::FILM_LINE),
                )
                .on_hover_text("Automatically follow interesting organisms");

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
                    .selectable_label(
                        *active_tab == SidebarTab::Inspector,
                        egui_remixicon::icons::SEARCH_LINE,
                    )
                    .on_hover_text("Inspector")
                    .clicked()
                {
                    *active_tab = SidebarTab::Inspector;
                }
                ui.add_space(4.0);
                if ui
                    .selectable_label(
                        *active_tab == SidebarTab::Genetics,
                        egui_remixicon::icons::TEST_TUBE_LINE,
                    )
                    .on_hover_text("Genetics")
                    .clicked()
                {
                    *active_tab = SidebarTab::Genetics;
                }
                ui.add_space(4.0);
                if ui
                    .selectable_label(
                        *active_tab == SidebarTab::Analytics,
                        egui_remixicon::icons::LINE_CHART_LINE,
                    )
                    .on_hover_text("Analytics")
                    .clicked()
                {
                    *active_tab = SidebarTab::Analytics;
                }
                ui.add_space(4.0);
                if ui
                    .selectable_label(
                        *active_tab == SidebarTab::Sandbox,
                        egui_remixicon::icons::TOOLS_LINE,
                    )
                    .on_hover_text("Sandbox")
                    .clicked()
                {
                    *active_tab = SidebarTab::Sandbox;
                }
                ui.add_space(4.0);
                if ui
                    .selectable_label(
                        *active_tab == SidebarTab::Tuning,
                        egui_remixicon::icons::SETTINGS_3_LINE,
                    )
                    .on_hover_text("Tuning")
                    .clicked()
                {
                    *active_tab = SidebarTab::Tuning;
                }
                ui.add_space(4.0);
                if ui
                    .selectable_label(
                        *active_tab == SidebarTab::Ecology,
                        egui_remixicon::icons::EARTH_LINE,
                    )
                    .on_hover_text("Ecology")
                    .clicked()
                {
                    *active_tab = SidebarTab::Ecology;
                }
                ui.add_space(4.0);
                if ui
                    .selectable_label(
                        *active_tab == SidebarTab::Settings,
                        egui_remixicon::icons::SETTINGS_3_LINE,
                    )
                    .on_hover_text("Settings")
                    .clicked()
                {
                    *active_tab = SidebarTab::Settings;
                }
            });
        });

    // ── Primary sidebar ────────────────────────────────────────────────────
    egui::SidePanel::left("primary_sidebar")
        .resizable(true)
        .default_width(260.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                match active_tab {
                    SidebarTab::Inspector => {
                    ui.heading(format!("{} Inspector", egui_remixicon::icons::SEARCH_LINE));
                    ui.separator();
                    ui.checkbox(debug_structural, format!("{} Debug Structural View", egui_remixicon::icons::SHAPE_LINE));
                    if *debug_structural {
                        ui.add(
                            egui::Slider::new(bone_line_thickness, 0.5..=5.0)
                                .text("Bone Line Thickness"),
                        );
                    }
                    ui.checkbox(show_vision_cones, format!("{} Show Vision Cones", egui_remixicon::icons::EYE_LINE));
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
                        let mut food_q = world.ecs.query::<&ecology::FoodPellet>();
                        let mut mineral_q = world.ecs.query::<&ecology::MineralPellet>();
                        let mut corpse_q = world.ecs.query::<&ecology::Corpse>();

                        if let Ok(node) = node_q.get(&world.ecs, entity) {
                            egui::CollapsingHeader::new(format!("{} Physics Node", egui_remixicon::icons::SETTINGS_4_LINE))
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
                        } else if let Ok(food) = food_q.get(&world.ecs, entity) {
                            egui::CollapsingHeader::new(format!("{} Food Pellet", egui_remixicon::icons::LEAF_LINE))
                                .default_open(true)
                                .show(ui, |ui| {
                                    ui.label(format!("Position: ({:.1}, {:.1})", food.position.x, food.position.y));
                                    ui.label(format!("Energy: {:.1}", food.energy_value));
                                });
                        } else if let Ok(mineral) = mineral_q.get(&world.ecs, entity) {
                            egui::CollapsingHeader::new(format!("{} Mineral Pellet", egui_remixicon::icons::VIP_DIAMOND_LINE))
                                .default_open(true)
                                .show(ui, |ui| {
                                    ui.label(format!("Position: ({:.1}, {:.1})", mineral.position.x, mineral.position.y));
                                    ui.label(format!("Energy: {:.1}", mineral.energy_value));
                                });
                        } else if let Ok(corpse) = corpse_q.get(&world.ecs, entity) {
                            egui::CollapsingHeader::new(format!("{} Corpse", egui_remixicon::icons::SKULL_LINE))
                                .default_open(true)
                                .show(ui, |ui| {
                                    ui.label(format!("Position: ({:.1}, {:.1})", corpse.position.x, corpse.position.y));
                                    ui.label(format!("Energy: {:.1}", corpse.energy_value));
                                    ui.label(format!("Decay: {} / {}", corpse.decay_timer, corpse.max_decay));
                                });
                        }

                        // Metabolism — Energy
                        let mut energy_q = world.ecs.query::<&metabolism::Energy>();
                        let mut age_q = world.ecs.query::<&metabolism::Age>();
                        let mut meta_q = world.ecs.query::<&metabolism::Metabolism>();
                        let has_meta = energy_q.get(&world.ecs, entity).is_ok();

                        if has_meta {
                            egui::CollapsingHeader::new(format!("{} Biology", egui_remixicon::icons::MICROSCOPE_LINE))
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

                        let mut gen_q = world.ecs.query::<&organisms::Generation>();
                        if let Ok(gen) = gen_q.get(&world.ecs, entity) {
                            ui.label(format!("Generation: {}", gen.0));
                        }

                        let mut spawn_q = world.ecs.query::<&organisms::SpawnTick>();
                        if let Ok(spawn) = spawn_q.get(&world.ecs, entity) {
                            ui.label(format!("Spawn tick: {}", spawn.0));
                        }

                        let mut traits_q = world.ecs.query::<&organisms::SandboxTraits>();
                        if let Ok(traits) = traits_q.get(&world.ecs, entity) {
                            egui::CollapsingHeader::new(format!("{} Active Components", egui_remixicon::icons::PRICE_TAG_3_LINE))
                                .default_open(true)
                                .show(ui, |ui| {
                                    if traits.is_membrane_seed { ui.label("Membrane Seed"); }
                                    if traits.link_duplicate { ui.label("Link Duplicate"); }
                                    if traits.sends_energy { ui.label("Sends Energy"); }
                                    if traits.respires { ui.label("Respires"); }
                                    if traits.photosynthesis { ui.label("Photosynthesis"); }
                                    if traits.has_tail { ui.label("Has Tail"); }
                                    if traits.kills_animals { ui.label("Kills Animals"); }
                                    if traits.edible_plant { ui.label("Edible Plant"); }
                                    if traits.edible_animal { ui.label("Edible Animal"); }
                                    if traits.repels { ui.label("Repels"); }
                                    if traits.grabbable { ui.label("Grabbable"); }
                                    if traits.fixable { ui.label("Fixable"); }
                                    if traits.velocity_tear { ui.label("Velocity Tear"); }
                                    if traits.mesh { ui.label("Mesh"); }
                                });
                        }

                        // Entity Graph / Segment Tree
                        egui::CollapsingHeader::new(format!("{} Body Structure", egui_remixicon::icons::TREE_LINE))
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
                        ui.separator();
                    }
                }
                SidebarTab::Genetics => {
                    ui.heading(format!("{} Genetics", egui_remixicon::icons::TEST_TUBE_LINE));
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
                        let mut brain_q = world.ecs.query::<&brain::Brain>();

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
                                        egui::RichText::new(format!("{} This organism's morphology and wiring is hardcoded by its Hox Sequence. CPPN mutations are disabled.", egui_remixicon::icons::ERROR_WARNING_LINE))
                                            .color(egui::Color32::YELLOW),
                                    );
                                } else {
                                    ui.horizontal(|ui| {
                                        if ui.button(format!("{} Mutate Add Node", egui_remixicon::icons::DICE_LINE)).clicked() {
                                            pending_mutation = Some("add_node");
                                        }
                                        if ui.button(format!("{} Mutate Add Connection", egui_remixicon::icons::DICE_LINE)).clicked() {
                                            pending_mutation = Some("add_conn");
                                        }
                                        if ui.button(format!("{} Mutate Weights", egui_remixicon::icons::DICE_LINE)).clicked() {
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

                            if let Ok(brain) = brain_q.get(&world.ecs, head) {
                                ui.add_space(10.0);
                                ui.separator();
                                ui.heading("CTRNN Topology (Live Brain)");
                                ui.label(format!("Nodes: {}", brain.nodes.len()));
                                ui.label(format!("Synapses: {}", brain.synapses.len()));

                                let (resp, p) = ui.allocate_painter(egui::vec2(ui.available_width(), 300.0), egui::Sense::hover());
                                p.rect_filled(resp.rect, 4.0, egui::Color32::from_black_alpha(50));

                                let mut b_pos = std::collections::HashMap::new();
                                for (i, _node) in brain.nodes.iter().enumerate() {
                                    let is_in = i < brain.input_count;
                                    let is_out = i >= brain.nodes.len() - brain.output_count;
                                    let x = if is_in { resp.rect.left() + 20.0 } else if is_out { resp.rect.right() - 20.0 } else { resp.rect.center().x };

                                    let (idx, total) = if is_in {
                                        (i, brain.input_count)
                                    } else if is_out {
                                        (i - (brain.nodes.len() - brain.output_count), brain.output_count)
                                    } else {
                                        let hidden_count = brain.nodes.len() - brain.input_count - brain.output_count;
                                        (i - brain.input_count, hidden_count)
                                    };

                                    let y = if total <= 1 {
                                        resp.rect.center().y
                                    } else {
                                        resp.rect.top() + 20.0 + (resp.rect.height() - 40.0) * (idx as f32 / (total - 1) as f32)
                                    };
                                    b_pos.insert(i, egui::pos2(x, y));
                                }

                                for syn in &brain.synapses {
                                    if let (Some(&p1), Some(&p2)) = (b_pos.get(&(syn.source as usize)), b_pos.get(&(syn.target as usize))) {
                                        let c = if syn.weight > 0.0 { egui::Color32::from_rgba_premultiplied(0,255,0,100) } else { egui::Color32::from_rgba_premultiplied(255,0,0,100) };
                                        p.line_segment([p1, p2], ((syn.weight.abs() * 2.0).clamp(1.0, 5.0), c));
                                    }
                                }

                                for (i, node) in brain.nodes.iter().enumerate() {
                                    if let Some(&pos) = b_pos.get(&i) {
                                        let act = brain::Brain::apply_activation(node.state + node.bias, node.activation);
                                        let intensity = ((act + 1.0) / 2.0).clamp(0.0, 1.0) * 255.0;
                                        p.circle_filled(pos, 8.0, egui::Color32::from_rgb(intensity as u8, intensity as u8, 255));

                                        if resp.hover_pos().is_some_and(|h| h.distance(pos) < 8.0) {
                                            egui::show_tooltip(ctx, ui.layer_id(), ui.id().with(format!("brain_tt_{}", i)), |ui| {
                                                ui.label(format!("CTRNN Node {}", i));
                                                ui.label(format!("State: {:.2}", node.state));
                                                ui.label(format!("Activation: {:.2}", act));
                                                ui.label(format!("Bias: {:.2}", node.bias));
                                            });
                                        }
                                    }
                                }
                            }


                            // Apply pending mutation
                            if let Some(action) = pending_mutation {
                                drop(repro_q);
                                drop(growth_q);
                                drop(spring_q);
                                drop(brain_q);

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
                    ui.heading(format!("{} Analytics", egui_remixicon::icons::LINE_CHART_LINE));
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

                        let avg_fps = metrics.smoothed_fps;
                        let avg_frame_time = if avg_fps > 0.0 { 1000.0 / avg_fps } else { 0.0 };
                        // To accurately get max frame time we'd need to look at fps_history
                        // For now we just use the smoothed frame time
                        ui.label(format!("Avg Frame Time: {:.1} ms", avg_frame_time));
                        ui.label("Target TPS: 60");

                        let ticks = (metrics.sim_time / 0.016).round() as u64;
                        ui.label(format!("Ticks Elapsed: {}", ticks));

                        ui.label(format!("Smoothed FPS: {:.1}", avg_fps));
                    } else {
                        ui.label("Analytics data not available.");
                    }
                }
                SidebarTab::Sandbox => {
                    ui.heading(format!("{} Sandbox & Presets", egui_remixicon::icons::TOOLS_LINE));
                    ui.separator();

                    egui::CollapsingHeader::new("Entity Presets")
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Click to spawn under camera.").small());
                            ui.add_space(8.0);
                            for preset in organisms::sandbox::PresetDefinition::standard_presets() {
                                if ui.button(&preset.name).clicked() {
                                    actions.push(MenuAction::SpawnPreset(preset.name.clone()));
                                }
                            }

                            ui.separator();
                            if ui.button(format!("{} Re-seed Ecosystem", egui_remixicon::icons::SEEDLING_LINE)).clicked() {
                                actions.push(MenuAction::ReseedEcosystem);
                            }
                        });

                    ui.add_space(10.0);

                    egui::CollapsingHeader::new("Structure Generator")
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Procedural hex-mesh builder.").small());
                            ui.add_space(8.0);

                            // Temporary UI state for the builder parameters
                            let id = egui::Id::new("hex_mesh_builder_state");
                            let mut cols = ui.data_mut(|d| d.get_temp::<usize>(id).unwrap_or(5));
                            let mut rows = ui.data_mut(|d| d.get_temp::<usize>(id.with("rows")).unwrap_or(5));
                            let mut spacing = ui.data_mut(|d| d.get_temp::<f32>(id.with("spacing")).unwrap_or(20.0));
                            let mut stiffness = ui.data_mut(|d| d.get_temp::<f32>(id.with("stiffness")).unwrap_or(20.0));
                            let mut is_fixed = ui.data_mut(|d| d.get_temp::<bool>(id.with("fixed")).unwrap_or(true));

                            ui.add(egui::Slider::new(&mut cols, 1..=20).text("Columns"));
                            ui.add(egui::Slider::new(&mut rows, 1..=20).text("Rows"));
                            ui.add(egui::Slider::new(&mut spacing, 10.0..=50.0).text("Spacing"));
                            ui.add(egui::Slider::new(&mut stiffness, 1.0..=100.0).text("Stiffness"));
                            ui.checkbox(&mut is_fixed, "Fixed (Anchored)");

                            ui.data_mut(|d| d.insert_temp(id, cols));
                            ui.data_mut(|d| d.insert_temp(id.with("rows"), rows));
                            ui.data_mut(|d| d.insert_temp(id.with("spacing"), spacing));
                            ui.data_mut(|d| d.insert_temp(id.with("stiffness"), stiffness));
                            ui.data_mut(|d| d.insert_temp(id.with("fixed"), is_fixed));

                            ui.add_space(8.0);
                            if ui.button("Generate Test Mesh").clicked() {
                                actions.push(MenuAction::GenerateHexMesh {
                                    cols,
                                    rows,
                                    spacing,
                                    stiffness,
                                    is_fixed,
                                });
                            }
                        });
                }
                SidebarTab::Tuning => {
                    ui.heading(format!("{} Physics Tuning", egui_remixicon::icons::SETTINGS_4_LINE));
                    ui.separator();
                    if let Some(mut phys) = world.ecs.get_resource_mut::<physics::PhysicsConfig>() {
                        egui::Grid::new("tuning_grid").num_columns(2).show(ui, |ui| {
                            ui.label("Substeps");
                            ui.add(egui::Slider::new(&mut phys.substep_count, 1..=10));
                            ui.end_row();

                            ui.label("Dampening");
                            ui.add(egui::Slider::new(&mut phys.dampening, 0.8..=1.0));
                            ui.end_row();

                            ui.label("Centering Force");
                            ui.add(egui::Slider::new(&mut phys.centering_force, 0.0..=10.0));
                            ui.end_row();

                            ui.label("Gravity");
                            ui.add(egui::Slider::new(&mut phys.gravity, -20.0..=20.0));
                            ui.end_row();

                            ui.label("Collision Force");
                            ui.add(egui::Slider::new(&mut phys.collision_force, 0.0..=5.0));
                            ui.end_row();

                            ui.label("Repel Force");
                            ui.add(egui::Slider::new(&mut phys.repel_force, 0.0..=5.0));
                            ui.end_row();

                            ui.label("Links Force");
                            ui.add(egui::Slider::new(&mut phys.links_force, 0.0..=5.0));
                            ui.end_row();

                            ui.label("Wall Force");
                            ui.add(egui::Slider::new(&mut phys.wall_force, 0.0..=5.0));
                            ui.end_row();

                            ui.label("Bone Thickness");
                            ui.add(egui::Slider::new(bone_line_thickness, 1.0..=10.0));
                            ui.end_row();
                        });
                    } else {
                        ui.label("Physics resource not found.");
                    }
                }
                SidebarTab::Ecology => {
                    ui.heading(format!("{} Ecology & Environment", egui_remixicon::icons::EARTH_LINE));
                    ui.separator();
                    ui.label("Global Sunlight: 100%");
                    ui.label("Ambient CO2: 400 ppm");
                    ui.label("Soil Fertility: High");
                    ui.label("Temperature: 22°C");
                    ui.add_space(16.0);
                    ui.heading("Catastrophes");
                    ui.label("Trigger a localized spatial hazard to test organism resilience.");
                    if ui.button(format!("{} Spawn Local Hazard", egui_remixicon::icons::ERROR_WARNING_LINE)).clicked() {
                        actions.push(MenuAction::SpawnManualHazard);
                    }
                }
                SidebarTab::Settings => {
                    ui.heading(format!("{} Settings", egui_remixicon::icons::SETTINGS_3_LINE));
                    ui.separator();
                    ui.label("Application Settings");
                    ui.add_space(10.0);
                    // Add some dummy settings for now
                    let mut dummy_vsync = true;
                    ui.checkbox(&mut dummy_vsync, "VSync Enabled");
                    let mut dummy_volume = 0.5;
                    ui.add(egui::Slider::new(&mut dummy_volume, 0.0..=1.0).text("Master Volume"));
                    ui.add_space(10.0);
                    if ui.button("Reset Settings").clicked() {
                        // Reset
                    }
                }
            }
            });
        });

    // ── Status bar (bottom strip) ──────────────────────────────────────────
    egui::TopBottomPanel::bottom("status_bar")
        .exact_height(24.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if *is_paused {
                    ui.label(
                        egui::RichText::new(format!("{} PAUSED", egui_remixicon::icons::PAUSE_LINE))
                            .color(egui::Color32::from_rgb(255, 150, 50))
                            .strong(),
                    );
                    ui.separator();
                }

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

                ui.label(format!("{} Tick: {}", egui_remixicon::icons::TIMER_LINE, tick_count));
                ui.separator();
                ui.label(format!("{} FPS: {:.0}", egui_remixicon::icons::SPEED_LINE, fps));
                ui.separator();
                ui.label(format!("{} Entities: {}", egui_remixicon::icons::BUG_LINE, entity_count));
                ui.separator();
                ui.label(if *debug_structural {
                    format!("{} Mode: Structural", egui_remixicon::icons::EYE_LINE)
                } else {
                    format!("{} Mode: SDF Skin", egui_remixicon::icons::EYE_LINE)
                });
                ui.separator();

                let food_count = world.ecs.query::<&ecology::FoodPellet>().iter(&world.ecs).count();
                let mineral_count = world.ecs.query::<&ecology::MineralPellet>().iter(&world.ecs).count();
                let corpse_count = world.ecs.query::<&ecology::Corpse>().iter(&world.ecs).count();

                let mut prod_count = 0;
                let mut herb_count = 0;
                let mut carn_count = 0;
                let mut omni_count = 0;
                let mut deco_count = 0;

                for diet in world.ecs.query::<&ecology::Diet>().iter(&world.ecs) {
                    match diet {
                        ecology::Diet::Producer => prod_count += 1,
                        ecology::Diet::Herbivore => herb_count += 1,
                        ecology::Diet::Carnivore => carn_count += 1,
                        ecology::Diet::Omnivore => omni_count += 1,
                        ecology::Diet::Decomposer => deco_count += 1,
                    }
                }

                ui.label(format!("{} Food: {}", egui_remixicon::icons::LEAF_LINE, food_count));
                ui.label(format!("{} Min: {}", egui_remixicon::icons::VIP_DIAMOND_LINE, mineral_count));
                ui.label(format!("{} Corpse: {}", egui_remixicon::icons::SKULL_LINE, corpse_count));
                ui.separator();
                ui.label(format!("{} Prod: {}", egui_remixicon::icons::SEEDLING_LINE, prod_count));
                ui.label(format!("{} Herb: {}", egui_remixicon::icons::BUG_LINE, herb_count));
                ui.label(format!("{} Carn: {}", egui_remixicon::icons::ALIENS_LINE, carn_count));
                ui.label(format!("{} Omni: {}", egui_remixicon::icons::BEAR_SMILE_LINE, omni_count));
                ui.label(format!("{} Deco: {}", egui_remixicon::icons::RECYCLE_LINE, deco_count));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    thread_local! {
                        static SYS: std::cell::RefCell<sysinfo::System> = std::cell::RefCell::new(sysinfo::System::new());
                    }
                    let mem_mb = SYS.with(|sys_cell| {
                        let mut sys = sys_cell.borrow_mut();
                        if let Ok(pid) = sysinfo::get_current_pid() {
                            sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
                            if let Some(process) = sys.process(pid) {
                                return process.memory() / 1024 / 1024;
                            }
                        }
                        0
                    });
                    ui.label(format!("Mem: {}MB", mem_mb));
                    ui.separator();
                    ui.label(egui::RichText::new(format!("{} Engine Online", egui_remixicon::icons::SERVER_LINE)).color(egui::Color32::GREEN));
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

    if *spectator_mode {
        let current_time = ctx.input(|i| i.time);

        let is_tracked_dead = tracked_entity.is_none_or(|e| world.ecs.get_entity(e).is_none());

        if is_tracked_dead || current_time - *last_spectator_switch_time > 15.0 {
            // Find most "interesting" organism (e.g. oldest alive or highest generation)
            let mut best_entity = None;
            let mut highest_generation = 0;
            let mut query = world
                .ecs
                .query::<(bevy_ecs::entity::Entity, &organisms::OrganismColor)>();
            if let Some(tracker) = world.ecs.get_resource::<evolution::LineageTracker>() {
                for (entity, _) in query.iter(&world.ecs) {
                    if let Some(record) = tracker.get_record(common::EntityId(entity.to_bits())) {
                        if record.generation >= highest_generation {
                            highest_generation = record.generation;
                            best_entity = Some(entity);
                        }
                    } else if best_entity.is_none() {
                        best_entity = Some(entity); // fallback
                    }
                }
            }

            if let Some(new_target) = best_entity {
                if tracked_entity.is_none() || new_target != tracked_entity.unwrap() {
                    *tracked_entity = Some(new_target);
                    *last_spectator_switch_time = current_time;
                }
            }
        }
    }

    if let Some(log) = world.ecs.get_resource::<analytics::NarrationLog>() {
        egui::Window::new("Narration Log")
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-10.0, 60.0))
            .collapsible(false)
            .title_bar(false)
            .resizable(false)
            .frame(egui::Frame::window(&ctx.style()).fill(egui::Color32::from_black_alpha(200)))
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new("Event Log")
                        .strong()
                        .color(egui::Color32::WHITE),
                );
                ui.separator();
                let mut count = 0;
                for event in log.events.iter().rev() {
                    ui.label(
                        egui::RichText::new(&event.description)
                            .color(egui::Color32::LIGHT_GRAY)
                            .size(12.0),
                    );
                    count += 1;
                    if count >= 8 {
                        break;
                    }
                }
            });
    }

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
    let hover_pos = interact_response.hover_pos();

    let screen_center = interact_response.rect.center();
    let ppp = ctx.pixels_per_point();

    let to_screen = |pos: common::Vec2| {
        egui::pos2(
            screen_center.x + (pos.x - camera_pos.x) * camera_zoom / ppp,
            screen_center.y - (pos.y - camera_pos.y) * camera_zoom / ppp,
        )
    };

    // We need get_connected_component earlier for vision cones
    let mut get_connected_component = |entity: bevy_ecs::entity::Entity| {
        let mut adj: std::collections::HashMap<
            bevy_ecs::entity::Entity,
            Vec<bevy_ecs::entity::Entity>,
        > = std::collections::HashMap::new();
        let mut query_springs = world.ecs.query::<&physics::Spring>();
        for spring in query_springs.iter(&world.ecs) {
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

    let selected_component = (*selected_entity).map(&mut get_connected_component);

    // Render vision cones if enabled
    if *show_vision_cones {
        let mut query = world.ecs.query::<(
            bevy_ecs::entity::Entity,
            &physics::ParticleNode,
            &sensing::HeadVision,
        )>();
        let mut painter = ctx.layer_painter(egui::LayerId::background());
        painter.set_clip_rect(interact_response.rect);

        for (ent, node, vision) in query.iter(&world.ecs) {
            // Contextual filtering: if an organism is selected, only show its vision cone.
            if let Some(ref sel_comp) = selected_component {
                if !sel_comp.contains(&ent) {
                    continue;
                }
            }

            let fwd = vision.last_forward;

            // Offset the cone's origin to the edge of the head
            let head_radius = 12.0;
            let origin_pos = common::Vec2::new(
                node.position.x + fwd.x * head_radius,
                node.position.y + fwd.y * head_radius,
            );

            let origin = to_screen(origin_pos);

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
                let x = origin_pos.x + angle.cos() * vision.range;
                let y = origin_pos.y + angle.sin() * vision.range;
                points.push(to_screen(common::Vec2::new(x, y)));
            }

            painter.add(egui::Shape::closed_line(
                points,
                egui::Stroke::new(
                    2.0,
                    egui::Color32::from_rgba_premultiplied(0, 255, 255, 255),
                ),
            ));
        }
    }

    (
        CanvasInteraction {
            rect: interact_response.rect,
            clicked: interact_response.clicked(),
            click_pos: interact_response.interact_pointer_pos(),
            hover_pos,
            drag_delta: interact_response.drag_delta(),
            zoom_delta,
        },
        actions,
    )
}
