use crate::state::UiState;
use egui::{Align2, Context, Window};

pub enum UiModal {
    ConfirmNewSim,
    ConfirmQuit,
    JumpToTick {
        input: String,
    },
    FilterByDiet {
        herbivore: bool,
        carnivore: bool,
        scavenger: bool,
    },
    AboutPhylon,
    KeyboardShortcuts,
    Preferences,
    ExperimentReady,
}

pub fn render_modals(ctx: &Context, state: &mut UiState) {
    if state.active_modal.is_none() {
        return;
    }

    // Dim the background to block interaction behind the modal for Confirmations
    let is_blocking = matches!(
        state.active_modal,
        Some(UiModal::ConfirmNewSim) | Some(UiModal::ConfirmQuit)
    );

    if is_blocking {
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("modal_dim"),
        ))
        .rect_filled(ctx.screen_rect(), 0.0, egui::Color32::from_black_alpha(160));
    }

    let mut close = false;
    let mut execute_action = None;

    if let Some(modal) = &mut state.active_modal {
        match modal {
            UiModal::ConfirmNewSim => {
                Window::new("Confirm New Simulation")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label("Are you sure you want to start a new simulation?");
                        ui.label("All current unsaved progress will be lost.");
                        ui.horizontal(|ui| {
                            if ui.button("Start New").clicked() {
                                execute_action = Some("new_sim");
                                close = true;
                            }
                            if ui.button("Cancel").clicked() {
                                close = true;
                            }
                        });
                    });
            }
            UiModal::ConfirmQuit => {
                Window::new("Confirm Quit")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label("You have unsaved changes.");
                        ui.label("Are you sure you want to quit?");
                        ui.horizontal(|ui| {
                            if ui.button("Quit Anyway").clicked() {
                                execute_action = Some("quit");
                                close = true;
                            }
                            if ui.button("Cancel").clicked() {
                                close = true;
                            }
                        });
                    });
            }
            UiModal::JumpToTick { input } => {
                Window::new("Jump to Tick")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Tick:");
                            let response = ui.text_edit_singleline(input);
                            if response.lost_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            {
                                execute_action = Some("jump");
                                close = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.button("Jump").clicked() {
                                execute_action = Some("jump");
                                close = true;
                            }
                            if ui.button("Cancel").clicked() {
                                close = true;
                            }
                        });
                    });
            }
            UiModal::FilterByDiet {
                herbivore,
                carnivore,
                scavenger,
            } => {
                Window::new("Select by Diet")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.checkbox(herbivore, "Herbivore");
                        ui.checkbox(carnivore, "Carnivore");
                        ui.checkbox(scavenger, "Scavenger");
                        ui.horizontal(|ui| {
                            if ui.button("Select").clicked() {
                                execute_action = Some("select_diet");
                                close = true;
                            }
                            if ui.button("Cancel").clicked() {
                                close = true;
                            }
                        });
                    });
            }
            UiModal::AboutPhylon => {
                let mut is_open = true;
                Window::new("About Phylon")
                    .open(&mut is_open)
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.heading("Phylon");
                        ui.label("Research-Grade Artificial Life Laboratory");
                        ui.label("Version 0.1.0");
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
                if !is_open {
                    close = true;
                }
            }
            UiModal::KeyboardShortcuts => {
                let mut is_open = true;
                Window::new("Keyboard Shortcuts")
                    .open(&mut is_open)
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        egui::Grid::new("shortcuts_grid")
                            .num_columns(2)
                            .show(ui, |ui| {
                                ui.label("Space");
                                ui.label("Toggle pause");
                                ui.end_row();
                                ui.label("F11");
                                ui.label("Toggle fullscreen");
                                ui.end_row();
                                ui.label("Ctrl+S");
                                ui.label("Save Snapshot");
                                ui.end_row();
                                ui.label("Ctrl+Shift+S");
                                ui.label("Save Snapshot As");
                                ui.end_row();
                                ui.label("Ctrl+O");
                                ui.label("Open Snapshot");
                                ui.end_row();
                                ui.label("Ctrl+Z");
                                ui.label("Undo God-Mode Action");
                                ui.end_row();
                                ui.label("Ctrl+Y");
                                ui.label("Redo");
                                ui.end_row();
                                ui.label("Ctrl+Q");
                                ui.label("Quit");
                                ui.end_row();
                                ui.label("Ctrl+R");
                                ui.label("Run Script");
                                ui.end_row();
                                ui.label("Ctrl+`");
                                ui.label("Toggle Script Console");
                                ui.end_row();
                            });
                    });
                if !is_open {
                    close = true;
                }
            }
            UiModal::Preferences => {
                let mut is_open = true;
                Window::new("Preferences")
                    .open(&mut is_open)
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label("Preferences coming soon.");
                    });
                if !is_open {
                    close = true;
                }
            }
            UiModal::ExperimentReady => {
                Window::new("Experiment Ready")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        if let Some(_exp) = &state.active_experiment {
                            ui.label("Experiment Configuration Loaded");
                            // Ideally show name and tick count, but Experiment is a dummy struct here
                            ui.label("Ready to run experiment?");
                        }
                        ui.horizontal(|ui| {
                            if ui.button("Run").clicked() {
                                execute_action = Some("run_experiment");
                                close = true;
                            }
                            if ui.button("Cancel").clicked() {
                                close = true;
                            }
                        });
                    });
            }
        }
    }

    if close {
        state.active_modal = None;
    }

    if let Some(action) = execute_action {
        match action {
            "new_sim" => {
                if let Some(tx) = &state.app_tx {
                    let _ = tx.send(crate::commands::AppCommand::ResetWorld);
                }
                state.unsaved_changes = false;
            }
            "quit" => {
                if let Some(tx) = &state.app_tx {
                    let _ = tx.send(crate::commands::AppCommand::Quit);
                }
            }
            "jump" => {
                if let UiModal::JumpToTick { input } = &state.active_modal.as_ref().unwrap() {
                    if let Ok(tick) = input.parse::<u64>() {
                        if let Some(tx) = &state.app_tx {
                            let _ = tx.send(crate::commands::AppCommand::SeekReplayToTick(tick));
                        }
                    }
                }
            }
            "select_diet" => {
                if let UiModal::FilterByDiet {
                    herbivore,
                    carnivore,
                    scavenger,
                } = &state.active_modal.as_ref().unwrap()
                {
                    let filter = crate::commands::DietFilter {
                        herbivore: *herbivore,
                        carnivore: *carnivore,
                        scavenger: *scavenger,
                    };
                    if let Some(tx) = &state.app_tx {
                        let _ = tx.send(crate::commands::AppCommand::SelectByDiet(filter));
                    }
                }
            }
            "run_experiment" => {
                if let Some(exp) = &state.active_experiment {
                    if let Some(tx) = &state.app_tx {
                        let _ = tx.send(crate::commands::AppCommand::RunExperiment(exp.clone()));
                    }
                    if let Some(task_tx) = &state.task_tx {
                        let _ = task_tx.send(crate::state::LoadingTask {
                            label: "Running Experiment".to_string(),
                            detail: "Starting...".to_string(),
                            progress: 0.0,
                            can_cancel: true,
                            cancel_flag: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
                                false,
                            )),
                        });
                    }
                }
            }
            _ => {}
        }
    }
}
