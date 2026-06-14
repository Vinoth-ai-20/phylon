use crate::modal::UiModal;
use crate::state::UiState;
use egui::{Context, TopBottomPanel};

pub fn render_menu_bar(ctx: &Context, state: &mut UiState) {
    TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            // FILE
            ui.menu_button("File", |ui| {
                if ui.button("New Simulation").clicked() {
                    state.active_modal = Some(UiModal::ConfirmNewSim);
                    ui.close_menu();
                }
                if ui.button("Open Snapshot...").clicked() {
                    // TODO: trigger rfd file picker for .bincode or .ron
                    // and spawn loading task
                    ui.close_menu();
                }
                if ui.button("Save Snapshot").clicked() {
                    // TODO: trigger direct save
                    ui.close_menu();
                }
                if ui.button("Save Snapshot As...").clicked() {
                    // TODO: trigger rfd file picker
                    ui.close_menu();
                }
                if ui.button("Open Experiment...").clicked() {
                    // TODO: trigger rfd for .toml
                    ui.close_menu();
                }
                if ui.button("Export CSV...").clicked() {
                    // TODO: trigger export
                    ui.close_menu();
                }
                if ui.button("Export Lineage Tree...").clicked() {
                    // TODO: trigger export
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    if state.unsaved_changes {
                        state.active_modal = Some(UiModal::ConfirmQuit);
                    } else {
                        // Directly quit or set a flag that we'll catch later
                        std::process::exit(0);
                    }
                    ui.close_menu();
                }
            });

            // EDIT
            ui.menu_button("Edit", |ui| {
                if ui
                    .add_enabled(
                        !state.god_mode_action_stack.is_empty(),
                        egui::Button::new("Undo God-Mode Action"),
                    )
                    .clicked()
                {
                    // TODO: Undo logic
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        !state.god_mode_redo_stack.is_empty(),
                        egui::Button::new("Redo"),
                    )
                    .clicked()
                {
                    // TODO: Redo logic
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Preferences...").clicked() {
                    state.active_modal = Some(UiModal::Preferences);
                    ui.close_menu();
                }
            });

            // SIMULATION
            ui.menu_button("Simulation", |ui| {
                let play_pause = if state.is_paused { "Play" } else { "Pause" };
                if ui.button(play_pause).clicked() {
                    state.is_paused = !state.is_paused;
                    ui.close_menu();
                }
                if ui
                    .add_enabled(state.is_paused, egui::Button::new("Step Forward"))
                    .clicked()
                {
                    // TODO: Step logic
                    ui.close_menu();
                }
                ui.separator();
                ui.menu_button("Speed", |ui| {
                    if ui
                        .radio_value(&mut state.simulation_speed, 0.25, "0.25×")
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(&mut state.simulation_speed, 0.5, "0.5×")
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(&mut state.simulation_speed, 1.0, "1× (Normal)")
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(&mut state.simulation_speed, 2.0, "2×")
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(&mut state.simulation_speed, 5.0, "5×")
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(&mut state.simulation_speed, 10.0, "10×")
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    // Uncapped might mean 1000.0 or a special flag. For now, max float
                    if ui
                        .radio_value(&mut state.simulation_speed, f32::MAX, "Uncapped")
                        .clicked()
                    {
                        ui.close_menu();
                    }
                });
                ui.separator();
                if ui.button("Reset Camera").clicked() {
                    state.camera = crate::state::CameraState::default();
                    ui.close_menu();
                }
            });

            // SELECTION
            ui.menu_button("Selection", |ui| {
                if ui.button("Select All Organisms").clicked() {
                    // TODO: Populate
                    ui.close_menu();
                }
                if ui.button("Deselect All").clicked() {
                    state.selected_entities.clear();
                    ui.close_menu();
                }
                if ui.button("Select by Diet...").clicked() {
                    state.active_modal = Some(UiModal::FilterByDiet {
                        herbivore: true,
                        carnivore: true,
                        scavenger: true,
                    });
                    ui.close_menu();
                }
                ui.menu_button("Select by Species...", |ui| {
                    // TODO: map active Species components
                    ui.label("(Coming soon)");
                });
                if ui.button("Invert Selection").clicked() {
                    // TODO: logic
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Inspect Selected").clicked() {
                    state.panels.entity_inspector = true;
                    ui.close_menu();
                }
            });

            // VIEW
            ui.menu_button("View", |ui| {
                ui.checkbox(&mut state.show_field_overlay, "Field Overlay");
                ui.checkbox(&mut state.show_trails, "Organism Trails");
                ui.checkbox(&mut state.show_species_colors, "Species Colors");
                ui.checkbox(&mut state.show_grid, "Grid Lines");
                ui.checkbox(&mut state.show_sensor_cones, "Sensor Cones");
                ui.checkbox(&mut state.show_disease_highlight, "Disease Highlight");
                ui.separator();
                ui.menu_button("Panels", |ui| {
                    ui.checkbox(&mut state.panels.analytics, "Analytics Dashboard");
                    ui.checkbox(&mut state.panels.entity_inspector, "Entity Inspector");
                    ui.checkbox(&mut state.panels.genome_inspector, "Genome Inspector");
                    ui.checkbox(&mut state.panels.brain_inspector, "Brain Inspector");
                    ui.checkbox(&mut state.panels.research, "Research & Plugins");
                    ui.checkbox(&mut state.panels.profiler, "Profiler (puffin)");
                });
                ui.separator();
                if ui.button("Fullscreen").clicked() {
                    // TODO: trigger fullscreen
                    ui.close_menu();
                }
            });

            // GO
            ui.menu_button("Go", |ui| {
                if ui.button("Focus Selected Organism").clicked() {
                    // TODO: logic
                    ui.close_menu();
                }
                if ui.button("Focus Origin").clicked() {
                    state.camera.offset = [0.0, 0.0];
                    ui.close_menu();
                }
                if ui.button("Jump to Tick...").clicked() {
                    state.active_modal = Some(UiModal::JumpToTick {
                        input: String::new(),
                    });
                    ui.close_menu();
                }
                if ui.button("Previous Species Event").clicked() {
                    // TODO: scrub logic
                    ui.close_menu();
                }
                if ui.button("Next Species Event").clicked() {
                    // TODO: scrub logic
                    ui.close_menu();
                }
            });

            // RUN
            ui.menu_button("Run", |ui| {
                if ui.button("Run Experiment...").clicked() {
                    // TODO: rfd
                    ui.close_menu();
                }
                if ui.button("Stop Experiment").clicked() {
                    // TODO: stop headless
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Run Script...").clicked() {
                    // TODO: rfd
                    ui.close_menu();
                }
                if ui.button("Script Console").clicked() {
                    state.panels.script_console = !state.panels.script_console;
                    ui.close_menu();
                }
            });

            // TERMINAL
            ui.menu_button("Terminal", |ui| {
                if ui.button("Open Script Console").clicked() {
                    state.panels.script_console = true;
                    ui.close_menu();
                }
                if ui.button("Clear Console Output").clicked() {
                    // TODO: clear
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("DB Query Console").clicked() {
                    state.panels.db_console = !state.panels.db_console;
                    ui.close_menu();
                }
            });

            // HELP
            ui.menu_button("Help", |ui| {
                if ui.button("Documentation").clicked() {
                    // TODO: Open "https://github.com/phylon-sim/docs"
                    ui.close_menu();
                }
                if ui.button("Keyboard Shortcuts").clicked() {
                    state.active_modal = Some(UiModal::KeyboardShortcuts);
                    ui.close_menu();
                }
                if ui.button("About Phylon").clicked() {
                    state.active_modal = Some(UiModal::AboutPhylon);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Report Issue").clicked() {
                    // TODO: Open "https://github.com/phylon-sim/issues"
                    ui.close_menu();
                }
            });
        });
    });
}
