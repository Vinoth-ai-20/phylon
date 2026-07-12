use crate::types::*;

/// The Command Palette's registry: `(label, action)` pairs a user can invoke
/// by fuzzy-typing the label. Deliberately scoped to `MenuAction` variants
/// that need no extra context (no `Entity`, no `Diet`, ...) — this makes a
/// subset of what already exists in `MenuAction` searchable, rather than
/// expanding `MenuAction` itself.
const COMMANDS: &[(&str, MenuAction)] = &[
    ("Play / Pause", MenuAction::TogglePlayPause),
    ("Step Forward", MenuAction::StepForward),
    ("Speed Up", MenuAction::SetSpeedUp),
    ("Speed Down", MenuAction::SetSpeedDown),
    ("Reseed Ecosystem", MenuAction::ReseedEcosystem),
    ("Save State…", MenuAction::SaveState),
    ("Load State…", MenuAction::LoadState),
    ("Import Genome…", MenuAction::ImportGenome),
    ("Take Screenshot", MenuAction::TakeScreenshot),
    ("Toggle Recording", MenuAction::ToggleRecording),
    ("Toggle Metrics Panel", MenuAction::ToggleMetrics),
    ("Toggle Event Log Panel", MenuAction::ToggleLog),
    ("Toggle Sidebar", MenuAction::ToggleSidebar),
    ("Reset Camera", MenuAction::CameraHome),
    ("Clear Selection", MenuAction::Deselect),
    ("Select All", MenuAction::SelectAll),
    ("Spawn Proto-Fish", MenuAction::SpawnProtoFish),
    ("Spawn Manual Hazard", MenuAction::SpawnManualHazard),
    ("Open Replay Bundle…", MenuAction::OpenReplayBundle),
    ("Export Lineages CSV…", MenuAction::ExportLineagesCsv),
    ("Export Events CSV…", MenuAction::ExportEventsCsv),
    ("Export Organisms CSV…", MenuAction::ExportOrganismsCsv),
    ("Export Metrics CSV…", MenuAction::ExportMetricsCsv),
    ("Export Metrics JSON…", MenuAction::ExportMetricsJson),
];

/// Renders the Command Palette overlay when `state.show_command_palette` is
/// set (toggled by Ctrl+Shift+P — see `shortcuts.rs`). A floating,
/// non-collapsible window with a search box filtering `COMMANDS` by a
/// case-insensitive substring match against the label; clicking (or
/// pressing Enter on) a result pushes its action and closes the palette.
pub fn command_palette_ui(
    ctx: &egui::Context,
    state: &mut crate::WorkbenchState,
    actions: &mut Vec<MenuAction>,
) {
    if !state.show_command_palette {
        return;
    }

    let mut open = true;
    egui::Window::new("Command Palette")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 80.0))
        .fixed_size(egui::vec2(360.0, 320.0))
        .show(ctx, |ui| {
            let search = ui.text_edit_singleline(&mut state.command_palette_query);
            search.request_focus();

            let needle = state.command_palette_query.to_lowercase();
            let matches: Vec<&(&str, MenuAction)> = COMMANDS
                .iter()
                .filter(|(label, _)| needle.is_empty() || label.to_lowercase().contains(&needle))
                .collect();

            ui.separator();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if matches.is_empty() {
                        crate::widgets::empty_state(ui, "No matching commands.");
                    }
                    for (label, action) in matches {
                        if ui.selectable_label(false, *label).clicked() {
                            actions.push(action.clone());
                            state.show_command_palette = false;
                        }
                    }
                });
        });

    if !open {
        state.show_command_palette = false;
    }
}
