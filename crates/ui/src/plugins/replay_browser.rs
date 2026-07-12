use crate::types::*;

/// Renders the Replay Browser — static inspection of a loaded
/// `.phylon-replay` bundle's recorded interventions. Deliberately not a
/// live-playback "Replay Timeline": replay execution is a separate
/// headless mode that never coexists with the interactive UI. Answers
/// "what's in this recording?" (seed, event count/tick range, every
/// recorded intervention) without live scrub/seek control.
#[allow(clippy::too_many_arguments)]
pub fn replay_browser_ui(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    _world: &mut world::World,
    actions: &mut Vec<MenuAction>,
) {
    ui.horizontal(|ui| {
        if ui
            .button(format!(
                "{} Open Replay Bundle…",
                egui_remixicon::icons::FOLDER_OPEN_LINE
            ))
            .clicked()
        {
            actions.push(MenuAction::OpenReplayBundle);
        }
        if state.replay_browser.is_some()
            && ui
                .button(format!("{} Close", egui_remixicon::icons::CLOSE_LINE))
                .clicked()
        {
            actions.push(MenuAction::CloseReplayBundle);
        }
    });
    ui.add_space(crate::theme::SPACE_SM);

    let Some(summary) = &state.replay_browser else {
        crate::widgets::empty_state(
            ui,
            "Open a .phylon-replay bundle to inspect its recorded interventions.",
        );
        return;
    };

    egui::Grid::new("replay_browser_summary")
        .striped(true)
        .show(ui, |ui| {
            crate::widgets::kv_row(ui, "Source", &summary.source_path);
            crate::widgets::kv_row_mono(ui, "Seed", &summary.seed.to_string());
            crate::widgets::kv_row_mono(ui, "Recorded events", &summary.events.len().to_string());
            crate::widgets::kv_row_mono(
                ui,
                "Last event tick",
                &summary.last_event_tick.to_string(),
            );
        });

    ui.add_space(crate::theme::SPACE_SM);
    ui.separator();

    if summary.events.is_empty() {
        crate::widgets::empty_state(ui, "This recording has no interventions — a purely emergent run, already fully reproducible from its seed alone.");
        return;
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("replay_browser_events")
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Tick").strong());
                    ui.label(egui::RichText::new("Event").strong());
                    ui.end_row();

                    for (tick, description) in &summary.events {
                        ui.label(tick.to_string());
                        ui.label(description);
                        ui.end_row();
                    }
                });
        });
}
