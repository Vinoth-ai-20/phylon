use crate::commands::AppCommand;
use crate::modal::UiModal;
use crate::state::{LoadingTask, UiState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn send_cmd(state: &mut UiState, cmd: AppCommand) {
    if let Some(tx) = &state.app_tx {
        let _ = tx.send(cmd);
    }
}

pub fn render_compact_menu(
    ui: &mut egui::Ui,
    state: &mut UiState,
    stats: &analytics::SimulationStats,
) {
    let mut style = ui.style().as_ref().clone();
    style.visuals.widgets.active.bg_fill = egui::Color32::TRANSPARENT;
    style.visuals.widgets.hovered.bg_fill = egui::Color32::TRANSPARENT;
    style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    ui.style_mut().visuals = style.visuals;

    egui::menu::bar(ui, |ui| {
        // FILE
        ui.menu_button("File", |ui| {
            if ui.button("New Simulation").clicked() {
                if state.unsaved_changes {
                    state.active_modal = Some(UiModal::ConfirmNewSim);
                } else {
                    send_cmd(state, AppCommand::ResetWorld);
                }
                ui.close_menu();
            }
            if ui.button("Open Snapshot...").clicked() {
                let task_tx = state.task_tx.clone();
                let app_tx = state.app_tx.clone();
                std::thread::spawn(move || {
                    if let Some(file) = pollster::block_on(
                        rfd::AsyncFileDialog::new()
                            .add_filter("Phylon Snapshot", &["bincode", "ron"])
                            .pick_file(),
                    ) {
                        if let Some(tx) = &task_tx {
                            let _ = tx.send(LoadingTask {
                                label: "Loading Snapshot".to_string(),
                                detail: "Reading file...".to_string(),
                                progress: -1.0,
                                can_cancel: false,
                                cancel_flag: Arc::new(AtomicBool::new(false)),
                            });
                        }
                        if let Some(tx) = &app_tx {
                            let _ = tx.send(AppCommand::LoadSnapshot(file.path().to_path_buf()));
                        }
                    }
                });
                ui.close_menu();
            }
            if ui.button("Save Snapshot").clicked() {
                if let Some(last) = &state.last_snapshot_path {
                    send_cmd(state, AppCommand::SaveSnapshot(last.clone()));
                } else {
                    let app_tx = state.app_tx.clone();
                    std::thread::spawn(move || {
                        if let Some(file) = pollster::block_on(
                            rfd::AsyncFileDialog::new()
                                .add_filter("Bincode (fast)", &["bincode"])
                                .add_filter("RON (readable)", &["ron"])
                                .save_file(),
                        ) {
                            let path = file.path().to_path_buf();
                            if let Some(tx) = &app_tx {
                                let _ = tx.send(AppCommand::SaveSnapshot(path));
                            }
                        }
                    });
                }
                ui.close_menu();
            }
            if ui.button("Save Snapshot As...").clicked() {
                let app_tx = state.app_tx.clone();
                std::thread::spawn(move || {
                    if let Some(file) = pollster::block_on(
                        rfd::AsyncFileDialog::new()
                            .add_filter("Bincode (fast)", &["bincode"])
                            .add_filter("RON (readable)", &["ron"])
                            .save_file(),
                    ) {
                        let path = file.path().to_path_buf();
                        if let Some(tx) = &app_tx {
                            let _ = tx.send(AppCommand::SaveSnapshot(path));
                        }
                    }
                });
                ui.close_menu();
            }
            if ui.button("Open Experiment...").clicked() {
                let app_tx = state.app_tx.clone();
                std::thread::spawn(move || {
                    if let Some(file) = pollster::block_on(
                        rfd::AsyncFileDialog::new()
                            .add_filter("Experiment Config", &["toml"])
                            .pick_file(),
                    ) {
                        let exp = research::Experiment::from_toml(file.path());
                        if let Some(tx) = &app_tx {
                            let _ = tx.send(AppCommand::StageExperiment(exp));
                        }
                    }
                });
                ui.close_menu();
            }
            if ui.button("Export CSV...").clicked() {
                if let Some(file) = pollster::block_on(
                    rfd::AsyncFileDialog::new()
                        .add_filter("CSV", &["csv"])
                        .save_file(),
                ) {
                    let path = file.path().to_path_buf();
                    let task_tx = state.task_tx.clone();
                    let history = stats.history.clone();
                    std::thread::spawn(move || {
                        if let Some(tx) = &task_tx {
                            let _ = tx.send(LoadingTask {
                                label: "Exporting CSV".to_string(),
                                detail: "Writing rows...".to_string(),
                                progress: 0.5,
                                can_cancel: false,
                                cancel_flag: Arc::new(AtomicBool::new(false)),
                            });
                        }
                        if let Ok(mut w) = std::fs::File::create(path) {
                            use std::io::Write;
                            let _ = writeln!(w, "tick,population,avg_energy,total_food");
                            for (tick, pop, energy, food) in history {
                                let _ = writeln!(w, "{},{},{},{}", tick, pop, energy, food);
                            }
                        }
                        if let Some(tx) = &task_tx {
                            let _ = tx.send(LoadingTask {
                                label: "Done".to_string(),
                                detail: "".to_string(),
                                progress: 1.0,
                                can_cancel: false,
                                cancel_flag: Arc::new(AtomicBool::new(false)),
                            });
                        }
                    });
                }
                ui.close_menu();
            }
            if ui.button("Export Lineage Tree...").clicked() {
                let app_tx = state.app_tx.clone();
                std::thread::spawn(move || {
                    if let Some(file) = pollster::block_on(
                        rfd::AsyncFileDialog::new()
                            .add_filter("JSON", &["json"])
                            .save_file(),
                    ) {
                        if let Some(tx) = &app_tx {
                            let _ =
                                tx.send(AppCommand::ExportLineageTree(file.path().to_path_buf()));
                        }
                    }
                });
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Quit").clicked() {
                if state.unsaved_changes {
                    state.active_modal = Some(UiModal::ConfirmQuit);
                } else {
                    send_cmd(state, AppCommand::Quit);
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
                if let Some(action) = state.god_mode_action_stack.pop() {
                    send_cmd(state, AppCommand::UndoGodMode(action.clone()));
                    state.god_mode_redo_stack.push(action);
                }
                ui.close_menu();
            }
            if ui
                .add_enabled(
                    !state.god_mode_redo_stack.is_empty(),
                    egui::Button::new("Redo"),
                )
                .clicked()
            {
                if let Some(action) = state.god_mode_redo_stack.pop() {
                    send_cmd(state, AppCommand::RedoGodMode(action.clone()));
                    state.god_mode_action_stack.push(action);
                }
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
                send_cmd(state, AppCommand::StepOneTick);
                ui.close_menu();
            }
            ui.separator();
            ui.menu_button("Speed", |ui| {
                ui.radio_value(&mut state.simulation_speed, 0.25, "0.25×");
                ui.radio_value(&mut state.simulation_speed, 0.5, "0.5×");
                ui.radio_value(&mut state.simulation_speed, 1.0, "1× (Normal)");
                ui.radio_value(&mut state.simulation_speed, 2.0, "2×");
                ui.radio_value(&mut state.simulation_speed, 5.0, "5×");
                ui.radio_value(&mut state.simulation_speed, 10.0, "10×");
                ui.radio_value(&mut state.simulation_speed, f32::MAX, "Uncapped");
            });
            ui.separator();
            if ui.button("Reset Camera").clicked() {
                send_cmd(state, AppCommand::ResetCamera);
                ui.close_menu();
            }
        });

        // SELECTION
        ui.menu_button("Selection", |ui| {
            if ui.button("Select All Organisms").clicked() {
                send_cmd(state, AppCommand::QueryAllEntityIds);
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
            if ui.button("Select by Species...").clicked() {
                send_cmd(state, AppCommand::QuerySpeciesList);
                ui.close_menu();
            }
            if ui.button("Invert Selection").clicked() {
                send_cmd(state, AppCommand::InvertSelection);
                ui.close_menu();
            }
            ui.separator();
            if ui
                .add_enabled(
                    !state.selected_entities.is_empty(),
                    egui::Button::new("Inspect Selected"),
                )
                .clicked()
            {
                state.panels.entity_inspector = true;
                if let Some(&first) = state.selected_entities.first() {
                    send_cmd(state, AppCommand::TrackEntity(first));
                }
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
                send_cmd(state, AppCommand::ToggleFullscreen);
                ui.close_menu();
            }
        });

        // GO
        ui.menu_button("Go", |ui| {
            if ui
                .add_enabled(
                    !state.selected_entities.is_empty(),
                    egui::Button::new("Focus Selected Organism"),
                )
                .clicked()
            {
                if let Some(&first) = state.selected_entities.first() {
                    send_cmd(state, AppCommand::TrackEntity(first));
                }
                ui.close_menu();
            }
            if ui.button("Focus Origin").clicked() {
                send_cmd(state, AppCommand::ResetCamera);
                ui.close_menu();
            }
            if ui.button("Jump to Tick...").clicked() {
                state.active_modal = Some(UiModal::JumpToTick {
                    input: String::new(),
                });
                ui.close_menu();
            }
            if ui.button("Previous Species Event").clicked() {
                send_cmd(state, AppCommand::SeekToPreviousSpeciationEvent);
                ui.close_menu();
            }
            if ui.button("Next Species Event").clicked() {
                send_cmd(state, AppCommand::SeekToNextSpeciationEvent);
                ui.close_menu();
            }
        });

        // RUN
        ui.menu_button("Run", |ui| {
            if ui.button("Run Experiment...").clicked() {
                if let Some(file) = pollster::block_on(
                    rfd::AsyncFileDialog::new()
                        .add_filter("Experiment Config", &["toml"])
                        .pick_file(),
                ) {
                    let exp = research::Experiment::from_toml(file.path());
                    send_cmd(state, AppCommand::RunExperiment(exp));
                    if let Some(tx) = &state.task_tx {
                        let _ = tx.send(LoadingTask {
                            label: "Running Experiment".to_string(),
                            detail: "Tick 0 / Total".to_string(),
                            progress: 0.0,
                            can_cancel: true,
                            cancel_flag: Arc::new(AtomicBool::new(false)),
                        });
                    }
                }
                ui.close_menu();
            }
            if ui
                .add_enabled(
                    state.active_loading_task.is_some(),
                    egui::Button::new("Stop Experiment"),
                )
                .clicked()
            {
                if let Some(task) = &mut state.active_loading_task {
                    task.cancel_flag.store(true, Ordering::Relaxed);
                }
                send_cmd(state, AppCommand::StopExperiment);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Run Script...").clicked() {
                if let Some(file) = pollster::block_on(
                    rfd::AsyncFileDialog::new()
                        .add_filter("Rhai Script", &["rhai"])
                        .pick_file(),
                ) {
                    send_cmd(state, AppCommand::RunScript(file.path().to_path_buf()));
                    state.panels.script_console = true;
                }
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
                state.script_console_log.clear();
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
                let _ = open::that("https://github.com/yourrepo/phylon/wiki");
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
                let _ = open::that("https://github.com/yourrepo/phylon/issues");
                ui.close_menu();
            }
        });
    });
}
