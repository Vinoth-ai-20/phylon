//! Workspace Manager overlay — the UI surface for every
//! workspace lifecycle operation (save/rename/duplicate/delete/export/
//! import/reset/apply). Deliberately thin: every operation that mutates
//! `WorkbenchState` is a plain function call into `crate::workspace`
//! (mirroring `layout::apply_layout_preset`'s existing "no `MenuAction`
//! round-trip needed for panel-arrangement changes" precedent); only
//! Export/Import — which need `app`-crate file I/O — go through
//! `MenuAction`. This file owns no lifecycle logic itself, only
//! presentation, the same "menu is a thin layer over the service" split
//! this crate also applies to Recent Files.

use crate::types::*;
use crate::workspace::ActiveWorkspace;

/// Renders the Workspace Manager window when `state.show_workspace_manager`
/// is set, plus its name-input sub-dialog (Save/Rename/Duplicate all share
/// one text field, per `WorkspaceNameDialog`'s own doc comment).
pub fn workspace_manager_ui(
    ctx: &egui::Context,
    state: &mut crate::WorkbenchState,
    actions: &mut Vec<MenuAction>,
) {
    if state.show_workspace_manager {
        let mut open = true;
        egui::Window::new("Workspace Manager")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(egui::vec2(380.0, 420.0))
            .show(ctx, |ui| {
                let active_label = match state.workspaces.active() {
                    Some(ActiveWorkspace::BuiltIn(preset)) => preset.label().to_string(),
                    Some(ActiveWorkspace::Saved(name)) => format!("{name} (saved)"),
                    None => "(none)".to_string(),
                };
                ui.label(format!("Active workspace: {active_label}"));
                ui.separator();

                ui.label(egui::RichText::new("Built-in Presets").strong());
                for preset in crate::layout::LayoutPreset::ALL {
                    ui.horizontal(|ui| {
                        let is_active =
                            state.workspaces.active() == Some(&ActiveWorkspace::BuiltIn(preset));
                        if ui.selectable_label(is_active, preset.label()).clicked() {
                            crate::layout::apply_layout_preset(state, preset);
                        }
                        if is_active
                            && ui
                                .small_button("Reset")
                                .on_hover_text(
                                    "Discard any live changes, restore this preset's canonical layout",
                                )
                                .clicked()
                        {
                            crate::workspace::reset_active_built_in(state);
                        }
                        if ui.small_button("Duplicate…").clicked() {
                            state.workspace_name_dialog = crate::WorkspaceNameDialog::Duplicating(
                                ActiveWorkspace::BuiltIn(preset),
                            );
                            state.workspace_name_input = format!("{} Copy", preset.label());
                        }
                    });
                }

                ui.separator();
                ui.label(egui::RichText::new("Saved Workspaces").strong());
                let mut names: Vec<String> =
                    state.workspaces.names().map(str::to_string).collect();
                names.sort();
                if names.is_empty() {
                    crate::widgets::empty_state(ui, "No saved workspaces yet.");
                }
                for name in &names {
                    ui.horizontal(|ui| {
                        let is_active = state.workspaces.active()
                            == Some(&ActiveWorkspace::Saved(name.clone()));
                        if ui.selectable_label(is_active, name).clicked() {
                            crate::workspace::apply_saved(state, name);
                        }
                        if ui.small_button("Rename").clicked() {
                            state.workspace_name_dialog =
                                crate::WorkspaceNameDialog::Renaming(name.clone());
                            state.workspace_name_input = name.clone();
                        }
                        if ui.small_button("Duplicate…").clicked() {
                            state.workspace_name_dialog = crate::WorkspaceNameDialog::Duplicating(
                                ActiveWorkspace::Saved(name.clone()),
                            );
                            state.workspace_name_input = format!("{name} Copy");
                        }
                        if ui.small_button("Export…").clicked() {
                            actions.push(MenuAction::ExportWorkspace(name.clone()));
                        }
                        if ui
                            .small_button("×")
                            .on_hover_text("Delete this workspace")
                            .clicked()
                        {
                            state.workspaces.delete(name);
                        }
                    });
                }

                ui.separator();
                if ui.button("Save Current Layout as New Workspace…").clicked() {
                    state.workspace_name_dialog = crate::WorkspaceNameDialog::SavingNew;
                    state.workspace_name_input.clear();
                }
                if ui.button("Import Workspace…").clicked() {
                    actions.push(MenuAction::ImportWorkspace);
                }
            });

        if !open {
            state.show_workspace_manager = false;
        }
    }

    render_name_dialog(ctx, state);
}

/// The shared Save/Rename/Duplicate name-input sub-dialog — one text field
/// driving all three flows, per `WorkspaceNameDialog`'s own doc comment.
fn render_name_dialog(ctx: &egui::Context, state: &mut crate::WorkbenchState) {
    let dialog = state.workspace_name_dialog.clone();
    let (title, confirm_label) = match &dialog {
        crate::WorkspaceNameDialog::Closed => return,
        crate::WorkspaceNameDialog::SavingNew => ("Save Workspace", "Save"),
        crate::WorkspaceNameDialog::Renaming(_) => ("Rename Workspace", "Rename"),
        crate::WorkspaceNameDialog::Duplicating(_) => ("Duplicate Workspace", "Duplicate"),
    };

    let mut open = true;
    let mut confirmed = false;
    let mut cancelled = false;
    egui::Window::new(title)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("Workspace name:");
            let response = ui.text_edit_singleline(&mut state.workspace_name_input);
            response.request_focus();
            ui.horizontal(|ui| {
                if ui.button(confirm_label).clicked() {
                    confirmed = true;
                }
                if ui.button("Cancel").clicked() {
                    cancelled = true;
                }
            });
        });

    if confirmed {
        let name = state.workspace_name_input.trim().to_string();
        if !name.is_empty() {
            match dialog {
                crate::WorkspaceNameDialog::Closed => {}
                crate::WorkspaceNameDialog::SavingNew => {
                    crate::workspace::save_current_as(state, name);
                }
                crate::WorkspaceNameDialog::Renaming(old_name) => {
                    state.workspaces.rename(&old_name, name);
                }
                crate::WorkspaceNameDialog::Duplicating(ActiveWorkspace::Saved(source)) => {
                    crate::workspace::duplicate_saved(state, &source, name);
                }
                crate::WorkspaceNameDialog::Duplicating(ActiveWorkspace::BuiltIn(preset)) => {
                    crate::workspace::duplicate_built_in(state, preset, name);
                }
            }
            state.workspace_name_dialog = crate::WorkspaceNameDialog::Closed;
        }
    } else if cancelled || !open {
        state.workspace_name_dialog = crate::WorkspaceNameDialog::Closed;
    }
}
