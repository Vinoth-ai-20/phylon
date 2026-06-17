use crate::components::tab_strip::tab_strip_vertical;
use crate::theme::{BG_PANEL, BORDER_DEFAULT};
use common::Tick;
use egui::{Frame, Layout, SidePanel};
use world::PhylonWorld;

pub fn render_right_panel(
    ctx: &egui::Context,
    ui_state: &mut crate::state::UiState,
    world: &mut PhylonWorld,
    tick: Tick,
    script_path: &mut String,
    load_script: &mut bool,
) {
    let collapsed = ui_state.is_right_collapsed;
    let width = if collapsed { 32.0 } else { 300.0 };

    let panel = SidePanel::right("right_panel")
        .resizable(!collapsed)
        .min_width(if collapsed { 32.0 } else { 300.0 })
        .max_width(if collapsed { 32.0 } else { 500.0 })
        .exact_width(if collapsed { 32.0 } else { width })
        .frame(
            Frame::none()
                .fill(BG_PANEL)
                .stroke(egui::Stroke::new(1.0, BORDER_DEFAULT)), // Left border
        );

    panel.show(ctx, |ui| {
        ui.horizontal(|ui| {
            if !collapsed {
                // Main content area first for right panel so tab strip is on the right
                ui.vertical(|ui| {
                    ui.add_space(8.0);

                    let tabs = [
                        (egui_phosphor::regular::CROSSHAIR, "Entity Inspector"),
                        (egui_phosphor::regular::DNA, "Genome Inspector"),
                        (egui_phosphor::regular::BRAIN, "Brain Inspector"),
                        (egui_phosphor::regular::FLASK, "Research & Plugins"),
                    ];

                    let active_tab = ui_state.active_right_tab;

                    // Collapse button on the left edge of the header area
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                            if ui.button(egui_phosphor::regular::CARET_RIGHT).clicked() {
                                ui_state.is_right_collapsed = true;
                            }
                            ui.heading(tabs[active_tab].1);
                        });
                    });
                    ui.add_space(8.0);

                    // Actually, we need to allocate the space properly so the tab strip goes to the right side
                    // So we wrap the main content in a UI that takes available width minus tab strip width
                    let content_width = ui.available_width() - 32.0 - 8.0; // 32px strip + 8px padding
                    ui.allocate_ui(egui::vec2(content_width, ui.available_height()), |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| match active_tab {
                            0 => {
                                crate::panels::entity_inspector::render_entity_inspector(
                                    ui,
                                    &ui_state.selected_entities,
                                    world,
                                );
                            }
                            1 => {
                                crate::panels::genome_inspector::render_genome_inspector(
                                    ui,
                                    &ui_state.selected_entities,
                                    world,
                                );
                            }
                            2 => {
                                crate::panels::brain_inspector::render_brain_inspector(
                                    ui,
                                    tick,
                                    &ui_state.selected_entities,
                                    world,
                                );
                            }
                            3 => {
                                crate::panels::research::render_research(
                                    ui,
                                    script_path,
                                    load_script,
                                );
                            }
                            _ => {}
                        });
                    });
                });

                // Separator line between content and tabs
                let rect = ui.max_rect();
                let x_pos = rect.max.x - 32.0;
                ui.painter().line_segment(
                    [egui::pos2(x_pos, rect.min.y), egui::pos2(x_pos, rect.max.y)],
                    egui::Stroke::new(1.0, BORDER_DEFAULT),
                );
            }

            // Tab strip on the right edge
            let tabs = [
                (egui_phosphor::regular::CROSSHAIR, "Entity Inspector"),
                (egui_phosphor::regular::DNA, "Genome Inspector"),
                (egui_phosphor::regular::BRAIN, "Brain Inspector"),
                (egui_phosphor::regular::FLASK, "Research & Plugins"),
            ];

            let mut active_tab = ui_state.active_right_tab;
            tab_strip_vertical(ui, &tabs, active_tab, &mut |idx| {
                active_tab = idx;
                if ui_state.is_right_collapsed {
                    ui_state.is_right_collapsed = false; // Expanding if clicking a tab while collapsed
                }
            });
            ui_state.active_right_tab = active_tab;
        });
    });
}
