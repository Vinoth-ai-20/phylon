//! Event log panel — live simulation events with search, filter, and severity coloring.

use crate::state::EventLogFilter;
use crate::types::*;

/// Render the event log panel.
pub fn event_log_ui(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
    actions: &mut Vec<MenuAction>,
) {
    // ── Toolbar row ────────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "{} Event Log",
                egui_remixicon::icons::NOTIFICATION_LINE
            ))
            .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button(egui_remixicon::icons::DOWNLOAD_LINE)
                .on_hover_text("Export log to clipboard")
                .clicked()
            {
                if let Some(log) = world.ecs.get_resource::<analytics::NarrationLog>() {
                    let text = log
                        .events
                        .iter()
                        .map(|e| format!("[{}] [{}] {}", e.tick, e.event_type, e.description))
                        .collect::<Vec<_>>()
                        .join("\n");
                    ui.output_mut(|o| o.copied_text = text);
                    state.push_toast(
                        "Log copied to clipboard",
                        crate::ToastSeverity::Success,
                        2.0,
                    );
                }
                let _ = actions; // keep borrow happy
            }
            let auto_label = if state.event_log_auto_scroll {
                egui::RichText::new(egui_remixicon::icons::ARROW_DOWN_LINE)
                    .color(crate::theme::GOOD)
            } else {
                egui::RichText::new(egui_remixicon::icons::ARROW_DOWN_LINE)
                    .color(crate::theme::DISABLED_FG)
            };
            if ui
                .small_button(auto_label)
                .on_hover_text("Toggle auto-scroll")
                .clicked()
            {
                state.event_log_auto_scroll = !state.event_log_auto_scroll;
            }
        });
    });

    // ── Search bar ────────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.label(egui_remixicon::icons::SEARCH_LINE);
        ui.text_edit_singleline(&mut state.event_log_search);
        if ui
            .small_button(egui_remixicon::icons::CLOSE_LINE)
            .on_hover_text("Clear search")
            .clicked()
        {
            state.event_log_search.clear();
        }
    });

    // ── Filter buttons ────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.label("Filter:");
        for (label, filter) in [
            ("All", EventLogFilter::All),
            ("Births", EventLogFilter::Births),
            ("Deaths", EventLogFilter::Deaths),
            ("Hazards", EventLogFilter::Hazards),
            ("User", EventLogFilter::UserActions),
        ] {
            if ui
                .selectable_label(state.event_log_filter == filter, label)
                .clicked()
            {
                state.event_log_filter = filter;
            }
        }
    });

    ui.separator();

    // ── Events list ──────────────────────────────────────────────────────
    let Some(log) = world.ecs.get_resource::<analytics::NarrationLog>() else {
        ui.label(
            egui::RichText::new("Event system not yet initialised.")
                .italics()
                .color(crate::theme::DISABLED_FG),
        );
        return;
    };

    let search_lower = state.event_log_search.to_lowercase();
    let filter = state.event_log_filter;

    let filtered: Vec<_> = log
        .events
        .iter()
        .filter(|ev| {
            // Apply severity filter
            let passes_filter = match filter {
                EventLogFilter::All => true,
                EventLogFilter::Births => {
                    ev.event_type.contains("Birth") || ev.event_type.contains("Spawn")
                }
                EventLogFilter::Deaths => {
                    ev.event_type.contains("Death") || ev.event_type.contains("Died")
                }
                EventLogFilter::Hazards => {
                    ev.event_type.contains("Hazard") || ev.event_type.contains("Catastrophe")
                }
                EventLogFilter::UserActions => {
                    ev.event_type.contains("User") || ev.event_type.contains("Manual")
                }
            };
            if !passes_filter {
                return false;
            }
            // Apply text search
            if !search_lower.is_empty() {
                let combined = format!("{} {}", ev.event_type, ev.description).to_lowercase();
                if !combined.contains(&search_lower) {
                    return false;
                }
            }
            true
        })
        .collect();

    let event_count = filtered.len();
    let auto_scroll = state.event_log_auto_scroll;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(auto_scroll)
        .show(ui, |ui| {
            if filtered.is_empty() {
                ui.label(
                    egui::RichText::new("No matching events.")
                        .italics()
                        .color(crate::theme::DISABLED_FG),
                );
            }
            for ev in &filtered {
                let color = severity_color_for_type(&ev.event_type);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("[{}]", ev.tick))
                            .color(crate::theme::DISABLED_FG)
                            .monospace()
                            .size(crate::theme::SIZE_SMALL),
                    );
                    ui.label(
                        egui::RichText::new(format!("[{}]", ev.event_type))
                            .color(color)
                            .size(crate::theme::SIZE_SMALL),
                    );
                    ui.label(
                        egui::RichText::new(&ev.description)
                            .color(egui::Color32::LIGHT_GRAY)
                            .size(crate::theme::SIZE_BODY),
                    );
                });
            }
        });

    // Footer
    ui.separator();
    ui.label(
        egui::RichText::new(format!("{} events", event_count))
            .color(crate::theme::DISABLED_FG)
            .size(crate::theme::SIZE_SMALL),
    );
}

/// Map an event type string to a display color — see `theme.rs`'s "Event Log
/// category palette" section (`LOG_BIRTH`/`LOG_HAZARD`/`LOG_MUTATION`/
/// `LOG_USER`; death reuses `DANGER`, which already carried the same value).
fn severity_color_for_type(event_type: &str) -> egui::Color32 {
    let et = event_type.to_lowercase();
    if et.contains("birth") || et.contains("spawn") || et.contains("born") {
        crate::theme::LOG_BIRTH
    } else if et.contains("death") || et.contains("died") || et.contains("extinct") {
        crate::theme::DANGER
    } else if et.contains("hazard") || et.contains("catastrophe") || et.contains("fire") {
        crate::theme::LOG_HAZARD
    } else if et.contains("mutation") || et.contains("speciation") {
        crate::theme::LOG_MUTATION
    } else if et.contains("user") || et.contains("manual") {
        crate::theme::LOG_USER
    } else {
        crate::theme::DISABLED_FG
    }
}
