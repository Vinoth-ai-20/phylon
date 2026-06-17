use crate::commands::AppCommand;
use crate::theme::TEXT_MUTED;
use egui::{Area, Color32, Frame, Id, Order, Pos2, Rect, RichText};

pub fn render_camera_controls(
    ctx: &egui::Context,
    ui_state: &mut crate::state::UiState,
    viewport_rect: Rect,
) {
    Area::new(Id::new("camera_controls_overlay"))
        .fixed_pos(viewport_rect.max - egui::vec2(120.0, 40.0)) // approx bottom right
        .order(Order::Foreground)
        .show(ctx, |ui| {
            Frame::none()
                .fill(Color32::from_rgba_unmultiplied(10, 12, 16, 210))
                .rounding(4.0)
                .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new(egui_phosphor::regular::MINUS)
                                    .min_size(egui::vec2(24.0, 22.0)),
                            )
                            .clicked()
                        {
                            if let Some(tx) = &ui_state.app_tx {
                                let _ = tx.send(AppCommand::ZoomOut);
                            }
                        }
                        ui.label(
                            RichText::new(format!("{:.0}%", ui_state.camera.zoom_level * 100.0))
                                .size(11.0)
                                .color(TEXT_MUTED),
                        );
                        if ui
                            .add(
                                egui::Button::new(egui_phosphor::regular::PLUS)
                                    .min_size(egui::vec2(24.0, 22.0)),
                            )
                            .clicked()
                        {
                            if let Some(tx) = &ui_state.app_tx {
                                let _ = tx.send(AppCommand::ZoomIn);
                            }
                        }
                        if ui
                            .add(
                                egui::Button::new(egui_phosphor::regular::CAMERA)
                                    .min_size(egui::vec2(24.0, 22.0)),
                            )
                            .clicked()
                        {
                            if let Some(tx) = &ui_state.app_tx {
                                let _ = tx.send(AppCommand::ResetCamera);
                            }
                        }
                    });
                });
        });
}

pub fn render_view_mode_selector(
    ctx: &egui::Context,
    ui_state: &mut crate::state::UiState,
    viewport_rect: Rect,
) {
    Area::new(Id::new("view_mode_overlay"))
        .fixed_pos(Pos2::new(
            viewport_rect.max.x - 36.0,
            viewport_rect.min.y + 8.0,
        ))
        .order(Order::Foreground)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;

                // Instead of borrowing the fields directly in the array, we define an enum-like action
                enum ModeAction {
                    Organism,
                    Field,
                    Sensor,
                    Disease,
                    Species,
                    Trails,
                }
                let modes = [
                    (
                        egui_phosphor::regular::BUG,
                        "Organism view",
                        ModeAction::Organism,
                        false,
                    ),
                    (
                        egui_phosphor::regular::STACK,
                        "Field overlay view",
                        ModeAction::Field,
                        true,
                    ),
                    (
                        egui_phosphor::regular::EYE,
                        "Sensor cone view",
                        ModeAction::Sensor,
                        true,
                    ),
                    (
                        egui_phosphor::regular::ATOM,
                        "Disease highlight view",
                        ModeAction::Disease,
                        true,
                    ),
                    (
                        egui_phosphor::regular::TREE,
                        "Species color view",
                        ModeAction::Species,
                        true,
                    ),
                    (
                        egui_phosphor::regular::GLOBE,
                        "Trail view",
                        ModeAction::Trails,
                        true,
                    ),
                ];

                for (icon, tooltip, action, active_val) in modes {
                    let state_val = match action {
                        ModeAction::Organism => ui_state.show_field_overlay,
                        ModeAction::Field => ui_state.show_field_overlay,
                        ModeAction::Sensor => ui_state.show_sensor_cones,
                        ModeAction::Disease => ui_state.show_disease_highlight,
                        ModeAction::Species => ui_state.show_species_colors,
                        ModeAction::Trails => ui_state.show_trails,
                    };
                    let is_active = state_val == active_val;

                    let bg_color = if is_active {
                        Color32::from_rgb(40, 45, 60)
                    } else {
                        Color32::TRANSPARENT
                    };
                    let btn = egui::Button::new(RichText::new(icon).size(16.0))
                        .min_size(egui::vec2(28.0, 28.0))
                        .fill(bg_color)
                        .frame(is_active);

                    let mut style = ui.style().as_ref().clone();
                    if !is_active {
                        style.visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
                    }
                    ui.style_mut().visuals = style.visuals;

                    if ui.add(btn).on_hover_text(tooltip).clicked() {
                        match action {
                            ModeAction::Organism => {
                                ui_state.show_field_overlay = !ui_state.show_field_overlay
                            }
                            ModeAction::Field => {
                                ui_state.show_field_overlay = !ui_state.show_field_overlay
                            }
                            ModeAction::Sensor => {
                                ui_state.show_sensor_cones = !ui_state.show_sensor_cones
                            }
                            ModeAction::Disease => {
                                ui_state.show_disease_highlight = !ui_state.show_disease_highlight
                            }
                            ModeAction::Species => {
                                ui_state.show_species_colors = !ui_state.show_species_colors
                            }
                            ModeAction::Trails => ui_state.show_trails = !ui_state.show_trails,
                        }
                    }
                }
            });
        });
}

pub fn render_selection_chip(
    ctx: &egui::Context,
    ui_state: &mut crate::state::UiState,
    viewport_rect: Rect,
) {
    if ui_state.selected_entities.is_empty() {
        return;
    }

    Area::new(Id::new("selection_chip_overlay"))
        .fixed_pos(Pos2::new(
            viewport_rect.min.x + viewport_rect.width() / 2.0 - 150.0,
            viewport_rect.min.y + 12.0,
        ))
        .order(Order::Foreground)
        .show(ctx, |ui| {
            Frame::none()
                .fill(Color32::from_rgba_unmultiplied(14, 16, 22, 230))
                .rounding(14.0)
                .inner_margin(egui::Margin::symmetric(12.0, 6.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let id = ui_state.selected_entities[0];
                        ui.label(
                            RichText::new(format!(
                                "{} Organism #{}  |  Stats...  ",
                                egui_phosphor::regular::BUG,
                                id.0
                            ))
                            .size(11.0),
                        );
                        if ui.button(egui_phosphor::regular::X).clicked() {
                            ui_state.selected_entities.clear();
                        }
                    });
                });
        });
}

pub fn render_async_progress(
    ctx: &egui::Context,
    ui_state: &mut crate::state::UiState,
    viewport_rect: Rect,
) {
    if let Some(task) = &ui_state.active_loading_task {
        // Dim viewport
        Area::new(Id::new("async_progress_dim"))
            .fixed_pos(viewport_rect.min)
            .order(Order::Foreground)
            .show(ctx, |ui| {
                ui.painter().rect_filled(
                    viewport_rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(0, 0, 0, 160),
                );
            });

        // Center card
        Area::new(Id::new("async_progress_card"))
            .fixed_pos(Pos2::new(
                viewport_rect.min.x + viewport_rect.width() / 2.0 - 170.0,
                viewport_rect.min.y + viewport_rect.height() / 2.0 - 50.0,
            ))
            .order(Order::Foreground)
            .show(ctx, |ui| {
                Frame::none()
                    .fill(Color32::from_rgb(18, 20, 28))
                    .rounding(6.0)
                    .inner_margin(16.0)
                    .show(ui, |ui| {
                        ui.set_width(340.0);
                        ui.vertical(|ui| {
                            ui.heading(&task.label);
                            ui.label(RichText::new(&task.detail).color(TEXT_MUTED));
                            ui.add_space(8.0);
                            ui.add(egui::ProgressBar::new(task.progress).show_percentage());
                            if task.can_cancel {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("Cancel").clicked() {
                                            task.cancel_flag
                                                .store(true, std::sync::atomic::Ordering::Relaxed);
                                        }
                                    },
                                );
                            }
                        });
                    });
            });
    }
}
