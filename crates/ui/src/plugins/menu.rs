//! Menu bar plugin — main application menu with File, Edit, View, Selection, Simulation, Tools, Help.

use crate::types::*;
use egui::Button;

/// Render the full application menu bar row.
#[allow(clippy::too_many_arguments)]
pub fn menu_ui(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
    actions: &mut Vec<MenuAction>,
) {
    let shortcuts = state.shortcuts.clone();

    egui::menu::bar(ui, |ui| {
        // ── FILE ──────────────────────────────────────────────────────────
        ui.menu_button("File", |ui| {
            if ui
                .add(
                    Button::new("Save State").shortcut_text(
                        shortcuts
                            .save_state
                            .format(&egui::ModifierNames::NAMES, false),
                    ),
                )
                .clicked()
            {
                actions.push(MenuAction::SaveState);
                ui.close_menu();
            }
            if ui
                .add(
                    Button::new("Load State").shortcut_text(
                        shortcuts
                            .load_state
                            .format(&egui::ModifierNames::NAMES, false),
                    ),
                )
                .clicked()
            {
                actions.push(MenuAction::LoadState);
                ui.close_menu();
            }

            // Recent Files (Phase 7, W0d) — this block is presentation
            // only: all ordering/duplicate/cap policy lives in
            // `crate::RecentItemsService` (see its module doc comment).
            // Previously this pushed a generic `MenuAction::LoadState`
            // regardless of which entry was clicked (opening a fresh file
            // picker instead of that entry's own path) — a confirmed,
            // permanently-dead control, since `recent_files` was also
            // never populated by anything. Both are fixed here.
            let recent_files: Vec<String> = state
                .recent_items
                .items(crate::RecentCategory::Files)
                .map(str::to_string)
                .collect();
            if !recent_files.is_empty() {
                ui.separator();
                ui.menu_button("Open Recent", |ui| {
                    for path in &recent_files {
                        let name = std::path::Path::new(path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(path);
                        let exists = std::path::Path::new(path).exists();
                        ui.horizontal(|ui| {
                            if exists {
                                if ui.button(name).clicked() {
                                    actions.push(MenuAction::LoadStateFromPath(path.clone()));
                                    ui.close_menu();
                                }
                            } else {
                                // Missing-file policy (see
                                // `recent_items`'s module doc comment):
                                // shown, clearly disabled, never silently
                                // dropped — removal is always an explicit
                                // user action, never automatic.
                                ui.add_enabled(
                                    false,
                                    egui::Button::new(format!("{name} (missing)")),
                                );
                                if ui
                                    .small_button("×")
                                    .on_hover_text("Remove from Recent Files")
                                    .clicked()
                                {
                                    state
                                        .recent_items
                                        .remove(crate::RecentCategory::Files, path);
                                }
                            }
                        });
                    }
                });
            }

            ui.separator();
            if ui
                .add(
                    Button::new("Import Genome").shortcut_text(
                        shortcuts
                            .import_genome
                            .format(&egui::ModifierNames::NAMES, false),
                    ),
                )
                .clicked()
            {
                actions.push(MenuAction::ImportGenome);
                ui.close_menu();
            }
            if ui
                .add(
                    Button::new("Export Genome").shortcut_text(
                        shortcuts
                            .export_genome
                            .format(&egui::ModifierNames::NAMES, false),
                    ),
                )
                .clicked()
            {
                actions.push(MenuAction::ExportGenome);
                ui.close_menu();
            }
            ui.separator();
            if ui
                .add(
                    Button::new("Take Screenshot").shortcut_text(
                        shortcuts
                            .take_screenshot
                            .format(&egui::ModifierNames::NAMES, false),
                    ),
                )
                .clicked()
            {
                actions.push(MenuAction::TakeScreenshot);
                ui.close_menu();
            }
            if ui
                .add(
                    Button::new(if state.recording_active {
                        "Stop Recording"
                    } else {
                        "Start Recording"
                    })
                    .shortcut_text(
                        shortcuts
                            .toggle_recording
                            .format(&egui::ModifierNames::NAMES, false),
                    ),
                )
                .clicked()
            {
                actions.push(MenuAction::ToggleRecording);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Quit").clicked() {
                actions.push(MenuAction::Quit);
            }
        });

        // ── EDIT ──────────────────────────────────────────────────────────
        //
        // Phase 6, Epic J: Undo/Redo and "Duplicate Selected" were removed
        // from here (and every other place that advertised them) — their
        // `MenuAction` handlers only ever logged a warning and did nothing.
        // Undo/Redo would need a real command-history stack; duplicating an
        // organism would need to clone its genome/lineage/diet through the
        // same spawn path births use — both are genuine new subsystems, out
        // of proportion for a UI-debt cleanup. Removed rather than left as
        // a control that silently does nothing while claiming otherwise.
        ui.menu_button("Edit", |ui| {
            if ui.button("Delete Selected").clicked() {
                actions.push(MenuAction::DeleteSelection);
                ui.close_menu();
            }
            ui.separator();
            if ui
                .add(Button::new("Select All").shortcut_text("Ctrl+A"))
                .clicked()
            {
                actions.push(MenuAction::SelectAll);
                ui.close_menu();
            }
            if ui
                .add(Button::new("Deselect").shortcut_text("Esc"))
                .clicked()
            {
                actions.push(MenuAction::Deselect);
                ui.close_menu();
            }
        });

        // ── VIEW ──────────────────────────────────────────────────────────
        ui.menu_button("View", |ui| {
            ui.checkbox(&mut state.debug_structural, "Wireframe View");
            ui.checkbox(&mut state.show_vision_cones, "Show Vision Cones");
            ui.checkbox(
                &mut state.show_organism_labels,
                format!(
                    "{} Show Organism Labels",
                    egui_remixicon::icons::PRICE_TAG_3_LINE
                ),
            );
            ui.checkbox(
                &mut state.spotlight_mode,
                format!(
                    "{} Spotlight Selected Organism",
                    egui_remixicon::icons::FOCUS_LINE
                ),
            );
            ui.checkbox(
                &mut state.show_world_boundary,
                format!(
                    "{} Show World Boundary",
                    egui_remixicon::icons::RECTANGLE_LINE
                ),
            );
            ui.checkbox(
                &mut state.show_scale_grid,
                format!("{} Show Scale Grid", egui_remixicon::icons::GRID_LINE),
            );
            ui.checkbox(
                &mut state.show_minimap,
                format!("{} Show Minimap", egui_remixicon::icons::MAP_2_LINE),
            );
            ui.separator();

            // Colormap selector — moved out of the always-visible toolbar
            // (Milestone 13): it's a configuration choice, not a per-frame
            // control, so it doesn't need constant screen real estate.
            let current_colormap = world
                .ecs
                .get_resource::<HeatmapState>()
                .map(|h| h.colormap)
                .unwrap_or(0);
            ui.menu_button(
                format!(
                    "Colormap: {}",
                    crate::plugins::toolbar::colormap_label(current_colormap)
                ),
                |ui| {
                    for (index, label) in crate::plugins::toolbar::COLORMAP_VARIANTS {
                        if ui
                            .selectable_label(current_colormap == index, label)
                            .clicked()
                        {
                            actions.push(MenuAction::SetColormap(index));
                            ui.close_menu();
                        }
                    }
                },
            );
            ui.separator();
            if ui
                .add(
                    Button::new(if state.sidebar_visible {
                        "Hide Sidebar"
                    } else {
                        "Show Sidebar"
                    })
                    .shortcut_text(
                        shortcuts
                            .toggle_sidebar
                            .format(&egui::ModifierNames::NAMES, false),
                    ),
                )
                .clicked()
            {
                actions.push(MenuAction::ToggleSidebar);
                state.sidebar_visible = !state.sidebar_visible;
                ui.close_menu();
            }
            if ui
                .add(
                    Button::new(if state.metrics_visible {
                        "Hide Metrics"
                    } else {
                        "Show Metrics"
                    })
                    .shortcut_text(
                        shortcuts
                            .toggle_metrics
                            .format(&egui::ModifierNames::NAMES, false),
                    ),
                )
                .clicked()
            {
                actions.push(MenuAction::ToggleMetrics);
                state.metrics_visible = !state.metrics_visible;
                ui.close_menu();
            }
            if ui
                .add(
                    Button::new(if state.event_log_visible {
                        "Hide Log"
                    } else {
                        "Show Log"
                    })
                    .shortcut_text(
                        shortcuts
                            .toggle_log
                            .format(&egui::ModifierNames::NAMES, false),
                    ),
                )
                .clicked()
            {
                actions.push(MenuAction::ToggleLog);
                state.event_log_visible = !state.event_log_visible;
                ui.close_menu();
            }
            if ui
                .button(if state.toolbar_visible {
                    "Hide Toolbar"
                } else {
                    "Show Toolbar"
                })
                .clicked()
            {
                state.toolbar_visible = !state.toolbar_visible;
                ui.close_menu();
            }
            ui.separator();
            ui.menu_button("Layout Presets", |ui| {
                // Phase 7, W3b: one list (`LayoutPreset::ALL`) instead of a
                // hardcoded button per preset, duplicated here and in the
                // Windows menu below — adding a 7th preset in the future is
                // now a one-line change to `LayoutPreset::ALL`, not two
                // more duplicated button blocks.
                for preset in crate::layout::LayoutPreset::ALL {
                    if ui.button(preset.label()).clicked() {
                        crate::layout::apply_layout_preset(state, preset);
                        ui.close_menu();
                    }
                }
            });
            if ui.button("Restore Defaults").clicked() {
                crate::layout::apply_default_layout(state);
                ui.close_menu();
            }
        });

        // ── SELECTION ─────────────────────────────────────────────────────
        ui.menu_button("Selection", |ui| {
            if ui.button("Select First Head").clicked() {
                actions.push(MenuAction::SelectAll);
                ui.close_menu();
            }
            if ui.button("Next Head").clicked() {
                actions.push(MenuAction::InvertSelection);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Select Producer").clicked() {
                actions.push(MenuAction::SelectByDiet(ecology::Diet::Producer));
                ui.close_menu();
            }
            if ui.button("Select Herbivore").clicked() {
                actions.push(MenuAction::SelectByDiet(ecology::Diet::Herbivore));
                ui.close_menu();
            }
            if ui.button("Select Carnivore").clicked() {
                actions.push(MenuAction::SelectByDiet(ecology::Diet::Carnivore));
                ui.close_menu();
            }
            if ui.button("Select Omnivore").clicked() {
                actions.push(MenuAction::SelectByDiet(ecology::Diet::Omnivore));
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Clear Selection").clicked() {
                actions.push(MenuAction::Deselect);
                ui.close_menu();
            }
            if ui.button("Focus Selection").clicked() {
                actions.push(MenuAction::FocusSelection);
                ui.close_menu();
            }
        });

        // ── SIMULATION ────────────────────────────────────────────────────
        ui.menu_button("Simulation", |ui| {
            let play_text = if state.is_paused { "Play" } else { "Pause" };
            if ui
                .add(
                    Button::new(play_text).shortcut_text(
                        shortcuts
                            .play_pause
                            .format(&egui::ModifierNames::NAMES, false),
                    ),
                )
                .clicked()
            {
                actions.push(MenuAction::TogglePlayPause);
                state.is_paused = !state.is_paused;
                ui.close_menu();
            }
            if ui
                .add(
                    Button::new("Step Forward").shortcut_text(
                        shortcuts
                            .step_forward
                            .format(&egui::ModifierNames::NAMES, false),
                    ),
                )
                .clicked()
            {
                actions.push(MenuAction::StepForward);
                ui.close_menu();
            }
            if ui.button("Reset Simulation").clicked() {
                actions.push(MenuAction::ReseedEcosystem);
                ui.close_menu();
            }
            ui.separator();
            ui.menu_button("Speed Presets", |ui| {
                for (label, speed) in [
                    ("1.0× Normal", 1.0f32),
                    ("2.0× Fast", 2.0),
                    ("5.0×", 5.0),
                    ("10.0× Very Fast", 10.0),
                ] {
                    if ui
                        .selectable_label((state.simulation_speed - speed).abs() < 0.05, label)
                        .clicked()
                    {
                        state.simulation_speed = speed;
                        ui.close_menu();
                    }
                }
            });
            ui.separator();
            if ui.button("Spawn Proto-Fish").clicked() {
                actions.push(MenuAction::SpawnProtoFish);
                ui.close_menu();
            }
            if ui.button("Spawn Hazard").clicked() {
                actions.push(MenuAction::SpawnManualHazard);
                ui.close_menu();
            }
        });

        // ── TOOLS ─────────────────────────────────────────────────────────
        ui.menu_button("Tools", |ui| {
            if ui
                .add(
                    Button::new("Export Genome…").shortcut_text(
                        shortcuts
                            .export_genome
                            .format(&egui::ModifierNames::NAMES, false),
                    ),
                )
                .clicked()
            {
                actions.push(MenuAction::ExportGenome);
                ui.close_menu();
            }
            if ui
                .add(
                    Button::new("Import Genome…").shortcut_text(
                        shortcuts
                            .import_genome
                            .format(&egui::ModifierNames::NAMES, false),
                    ),
                )
                .clicked()
            {
                actions.push(MenuAction::ImportGenome);
                ui.close_menu();
            }
        });

        // ── WINDOWS ──────────────────────────────────────────────────────────
        ui.menu_button("Windows", |ui| {
            ui.label(
                egui::RichText::new("Panels")
                    .small()
                    .color(crate::theme::DISABLED_FG),
            );
            ui.separator();

            for &panel_name in crate::layout::ALL_PANEL_NAMES {
                let mode = state
                    .panel_modes
                    .get(panel_name)
                    .copied()
                    .unwrap_or(crate::state::PanelMode::Docked);

                let is_visible = mode != crate::state::PanelMode::Closed;

                let label = match mode {
                    crate::state::PanelMode::Docked => {
                        format!("{} {}", egui_remixicon::icons::LAYOUT_LINE, panel_name)
                    }
                    crate::state::PanelMode::Floating => {
                        format!("{} {}", egui_remixicon::icons::WINDOW_2_LINE, panel_name)
                    }
                    crate::state::PanelMode::Closed => {
                        format!("{} {}", egui_remixicon::icons::EYE_CLOSE_LINE, panel_name)
                    }
                };

                let response = ui.selectable_label(is_visible, label);
                if response.clicked() {
                    if is_visible {
                        actions.push(MenuAction::ClosePanel(panel_name.to_string()));
                    } else {
                        actions.push(MenuAction::DockPanel(panel_name.to_string()));
                    }
                    ui.close_menu();
                }
                if response.hovered() {
                    let tip = match mode {
                        crate::state::PanelMode::Docked => "Docked in layout — click to close",
                        crate::state::PanelMode::Floating => "Floating window — click to close",
                        crate::state::PanelMode::Closed => "Closed — click to restore",
                    };
                    response.on_hover_text(tip);
                }
            }

            ui.separator();
            ui.menu_button("Layout Presets", |ui| {
                // Phase 7, W3b: one list (`LayoutPreset::ALL`) instead of a
                // hardcoded button per preset, duplicated here and in the
                // Windows menu below — adding a 7th preset in the future is
                // now a one-line change to `LayoutPreset::ALL`, not two
                // more duplicated button blocks.
                for preset in crate::layout::LayoutPreset::ALL {
                    if ui.button(preset.label()).clicked() {
                        crate::layout::apply_layout_preset(state, preset);
                        ui.close_menu();
                    }
                }
            });
            if ui.button("Restore Defaults").clicked() {
                crate::layout::apply_default_layout(state);
                ui.close_menu();
            }
        });

        // ── HELP ──────────────────────────────────────────────────────────
        ui.menu_button("Help", |ui| {
            if ui.button("Documentation").clicked() {
                actions.push(MenuAction::ShowDocumentation);
                ui.close_menu();
            }
            if ui.button("Keybinds").clicked() {
                actions.push(MenuAction::ShowKeybinds);
                ui.close_menu();
            }
            if ui.button("Welcome Tips").clicked() {
                actions.push(MenuAction::ShowOnboardingHints);
                ui.close_menu();
            }
            if ui.button("About").clicked() {
                actions.push(MenuAction::ShowAbout);
                ui.close_menu();
            }
        });
    });
}
