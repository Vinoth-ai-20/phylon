use crate::components::tab_strip::tab_strip_vertical;
use crate::theme::{BG_PANEL, BORDER_DEFAULT};
use analytics::SimulationStats;
use common::Tick;
use egui::{Frame, Layout, SidePanel};
use world::PhylonWorld;

pub fn render_left_panel(
    ctx: &egui::Context,
    ui_state: &mut crate::state::UiState,
    _world: &mut PhylonWorld,
    stats: &SimulationStats,
    tick: Tick,
) {
    let collapsed = ui_state.is_left_collapsed;
    let width = if collapsed { 32.0 } else { 260.0 };

    let panel = SidePanel::left("left_panel")
        .resizable(!collapsed)
        .min_width(if collapsed { 32.0 } else { 300.0 })
        .max_width(if collapsed { 32.0 } else { 400.0 })
        .exact_width(if collapsed { 32.0 } else { width })
        .frame(
            Frame::none()
                .fill(BG_PANEL)
                .stroke(egui::Stroke::new(1.0, BORDER_DEFAULT)), // Right border
        );

    panel.show(ctx, |ui| {
        ui.horizontal(|ui| {
            // Tab strip is always 32px wide on the left edge
            let tabs = [
                (egui_phosphor::regular::CHART_BAR, "Analytics"),
                (egui_phosphor::regular::TREE, "Species"),
                (egui_phosphor::regular::HEARTBEAT, "Events"),
                (egui_phosphor::regular::FLASK, "Experiments"),
            ];

            let mut active_tab = ui_state.active_left_tab;
            tab_strip_vertical(ui, &tabs, active_tab, &mut |idx| {
                active_tab = idx;
                if ui_state.is_left_collapsed {
                    ui_state.is_left_collapsed = false; // Expanding if clicking a tab while collapsed
                }
            });
            ui_state.active_left_tab = active_tab;

            if !collapsed {
                // Separator line between tabs and content
                let rect = ui.max_rect();
                ui.painter().line_segment(
                    [
                        egui::pos2(rect.min.x + 32.0, rect.min.y),
                        egui::pos2(rect.min.x + 32.0, rect.max.y),
                    ],
                    egui::Stroke::new(1.0, BORDER_DEFAULT),
                );

                ui.add_space(8.0); // Padding after separator

                // Main content area
                ui.vertical(|ui| {
                    ui.add_space(8.0);

                    // Collapse button on the right edge of the header area
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(egui_phosphor::regular::CARET_LEFT).clicked() {
                                ui_state.is_left_collapsed = true;
                            }
                            ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.heading(tabs[active_tab].1);
                            });
                        });
                    });
                    ui.add_space(8.0);

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        match active_tab {
                            0 => {
                                crate::panels::analytics::render_analytics_dashboard(
                                    ui, stats, tick,
                                );
                            }
                            1 => {
                                // TODO: Species tab
                                ui.label("Species tab placeholder");
                            }
                            2 => {
                                // TODO: Events tab
                                ui.label("Events tab placeholder");
                            }
                            3 => {
                                // TODO: Experiments tab
                                ui.label("Experiments tab placeholder");
                            }
                            _ => {}
                        }
                    });
                });
            }
        });
    });
}
