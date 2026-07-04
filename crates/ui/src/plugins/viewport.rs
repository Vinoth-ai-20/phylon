//! Viewport plugin — transparent canvas with input capture, selection, and context menu.

use crate::types::*;
use crate::WorkbenchState;

/// Render the transparent viewport panel.
///
/// Captures mouse/scroll input for the 3D canvas and shows a context menu
/// on right-click that is entity-aware (different options when an organism
/// is under the cursor vs. empty space).
pub fn viewport_ui(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut WorkbenchState,
    canvas_interaction: &mut Option<CanvasInteraction>,
    actions: &mut Vec<MenuAction>,
) {
    let _ = ctx;

    // Keep the area transparent so wgpu shows through
    egui::Frame::none()
        .fill(egui::Color32::TRANSPARENT)
        .show(ui, |ui| {
            let rect = ui.available_rect_before_wrap();
            state.canvas_rect = Some([
                rect.min.x as u32,
                rect.min.y as u32,
                rect.width() as u32,
                rect.height() as u32,
            ]);

            let interact_response = ui.interact(
                rect,
                ui.id().with("viewport"),
                egui::Sense::click_and_drag(),
            );

            let hover_pos = ui.input(|i| i.pointer.hover_pos());
            let zoom_delta = ui.input(|i| i.zoom_delta());

            *canvas_interaction = Some(CanvasInteraction {
                rect: interact_response.rect,
                clicked: interact_response.clicked(),
                click_pos: interact_response.interact_pointer_pos(),
                hover_pos,
                drag_delta: interact_response.drag_delta(),
                zoom_delta,
            });

            // ── Context Menu (Right Click) ────────────────────────────────
            interact_response.context_menu(|ui| {
                // Determine if the hover is over a selected organism
                let selected = state.selected_entity;
                let hovered = state.hovered_entity;

                // Use whichever entity is most relevant
                let target = hovered.or(selected);

                if let Some(entity) = target {
                    // Entity-specific actions
                    ui.label(
                        egui::RichText::new(format!("Entity {:?}", entity))
                            .small()
                            .color(crate::theme::DISABLED_FG),
                    );
                    ui.separator();

                    if ui
                        .button(format!("{} Inspect", egui_remixicon::icons::SEARCH_LINE))
                        .clicked()
                    {
                        actions.push(MenuAction::SelectEntity(entity));
                        state.active_tab = crate::SidebarTab::Inspector;
                        state.sidebar_visible = true;
                        ui.close_menu();
                    }
                    if ui
                        .button(format!(
                            "{} Track / Follow",
                            egui_remixicon::icons::FOCUS_LINE
                        ))
                        .clicked()
                    {
                        actions.push(MenuAction::TrackEntity(entity));
                        ui.close_menu();
                    }
                    if ui
                        .button(format!(
                            "{} Export Genome…",
                            egui_remixicon::icons::DOWNLOAD_LINE
                        ))
                        .clicked()
                    {
                        actions.push(MenuAction::SelectEntity(entity));
                        actions.push(MenuAction::ExportGenome);
                        ui.close_menu();
                    }
                    if ui
                        .button(format!("{} Copy ID", egui_remixicon::icons::CLIPBOARD_LINE))
                        .clicked()
                    {
                        actions.push(MenuAction::CopyEntityId(entity));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add(egui::Button::new(
                            egui::RichText::new(format!(
                                "{} Kill Entity",
                                egui_remixicon::icons::DELETE_BIN_LINE
                            ))
                            .color(egui::Color32::from_rgb(220, 80, 80)),
                        ))
                        .clicked()
                    {
                        actions.push(MenuAction::KillEntity(entity));
                        ui.close_menu();
                    }

                    ui.separator();
                }

                // Empty-space / general actions
                ui.menu_button(
                    format!("{} Spawn…", egui_remixicon::icons::ADD_CIRCLE_LINE),
                    |ui| {
                        if ui.button("Proto-Fish").clicked() {
                            actions.push(MenuAction::SpawnProtoFish);
                            ui.close_menu();
                        }
                        for preset in organisms::sandbox::PresetDefinition::standard_presets() {
                            if ui.button(&preset.name).clicked() {
                                actions.push(MenuAction::SpawnPreset(preset.name));
                                ui.close_menu();
                            }
                        }
                    },
                );

                if ui
                    .button(format!("{} Spawn Hazard", egui_remixicon::icons::FIRE_LINE))
                    .clicked()
                {
                    actions.push(MenuAction::SpawnManualHazard);
                    ui.close_menu();
                }

                ui.separator();
                if ui
                    .button(format!("{} Reset Camera", egui_remixicon::icons::HOME_LINE))
                    .clicked()
                {
                    actions.push(MenuAction::CameraHome);
                    ui.close_menu();
                }
                if ui
                    .button(format!(
                        "{} Clear Selection",
                        egui_remixicon::icons::CLOSE_CIRCLE_LINE
                    ))
                    .clicked()
                {
                    actions.push(MenuAction::Deselect);
                    ui.close_menu();
                }
            });

            // Double-click to focus the selected entity
            if interact_response.double_clicked() {
                if let Some(entity) = state.selected_entity {
                    state.tracked_entity = Some(entity);
                    state.spectator_mode = false;
                } else {
                    actions.push(MenuAction::FocusSelection);
                }
            }

            // Draw selection rectangle while dragging
            if interact_response.dragged_by(egui::PointerButton::Primary) {
                if let Some(start) = interact_response.interact_pointer_pos() {
                    if let Some(current) = hover_pos {
                        if start != current {
                            let sel_rect = egui::Rect::from_two_pos(start, current);
                            ui.painter().rect(
                                sel_rect,
                                0.0,
                                egui::Color32::from_white_alpha(20),
                                egui::Stroke::new(1.0, egui::Color32::from_white_alpha(180)),
                            );
                        }
                    }
                }
            }
        });
}
