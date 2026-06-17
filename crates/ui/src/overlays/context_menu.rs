use crate::commands::AppCommand;
use crate::theme::{BG_PANEL, BORDER_SUBTLE};
use egui::{Area, Color32, Frame, Id, Order, RichText};

pub fn render_context_menu(ctx: &egui::Context, ui_state: &mut crate::state::UiState) {
    let mut close_menu = false;

    if let Some((pos, entity)) = ui_state.active_context_menu {
        // If user clicks outside the menu, it should close.
        // We can check if there's a click not on the menu.
        if ctx.input(|i| i.pointer.primary_clicked() || i.pointer.secondary_clicked()) {
            // We'll defer closing to after drawing, to allow the menu itself to consume the click if it's inside.
        }

        Area::new(Id::new("context_menu"))
            .fixed_pos(pos)
            .order(Order::Tooltip) // On top of everything
            .show(ctx, |ui| {
                let frame = Frame::none()
                    .fill(BG_PANEL)
                    .stroke(egui::Stroke::new(1.0, BORDER_SUBTLE))
                    .rounding(4.0)
                    .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                    .shadow(egui::epaint::Shadow {
                        offset: egui::vec2(0.0, 4.0),
                        blur: 8.0,
                        spread: 0.0,
                        color: Color32::from_black_alpha(100),
                    });

                frame.show(ui, |ui| {
                    ui.set_min_width(160.0);

                    if let Some(id) = entity {
                        ui.label(RichText::new(format!("Organism #{}", id.0)).strong());
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // Styling for menu buttons
                        let mut style = ui.style().as_ref().clone();
                        style.visuals.widgets.inactive.bg_fill = Color32::TRANSPARENT;
                        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(40, 45, 60);
                        style.visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
                        ui.style_mut().visuals = style.visuals;

                        if ui.button("⌂ Focus Camera").clicked() {
                            // We don't have FocusEntity yet, but we'll wire the UI
                            if let Some(_tx) = &ui_state.app_tx {
                                // let _ = tx.send(AppCommand::FocusEntity(id));
                            }
                            close_menu = true;
                        }
                        if ui.button("🧬 Inspect Genome").clicked() {
                            ui_state.active_right_tab = 1; // Genome tab
                            ui_state.is_right_collapsed = false;
                            close_menu = true;
                        }
                        if ui.button("🕸 Inspect Brain").clicked() {
                            ui_state.active_right_tab = 2; // Brain tab
                            ui_state.is_right_collapsed = false;
                            close_menu = true;
                        }
                        if ui.button("📍 Mark for Tracking").clicked() {
                            // Tracking logic
                            close_menu = true;
                        }

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);

                        if ui
                            .button(
                                RichText::new("☠ Kill Entity")
                                    .color(Color32::from_rgb(200, 60, 60)),
                            )
                            .clicked()
                        {
                            // Kill entity logic
                            close_menu = true;
                        }
                    } else {
                        ui.label(RichText::new("Environment").strong());
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);

                        let mut style = ui.style().as_ref().clone();
                        style.visuals.widgets.inactive.bg_fill = Color32::TRANSPARENT;
                        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(40, 45, 60);
                        style.visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
                        ui.style_mut().visuals = style.visuals;

                        if ui.button("⌂ Reset Camera").clicked() {
                            if let Some(tx) = &ui_state.app_tx {
                                let _ = tx.send(AppCommand::ResetCamera);
                            }
                            close_menu = true;
                        }
                        if ui.button("Spawn Food").clicked() {
                            // Spawn food
                            close_menu = true;
                        }
                    }

                    // Check if mouse clicked outside this frame to close it
                    if ctx.input(|i| i.pointer.primary_clicked() || i.pointer.secondary_clicked()) {
                        let pointer_pos = ctx.input(|i| i.pointer.interact_pos());
                        if let Some(p) = pointer_pos {
                            if !ui.clip_rect().contains(p) {
                                close_menu = true;
                            }
                        }
                    }
                });
            });
    }

    if close_menu {
        ui_state.active_context_menu = None;
    }
}
