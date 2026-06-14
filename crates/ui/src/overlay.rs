use crate::state::UiState;
use egui::{Align2, Area, Color32, Context, Order, RichText, Rounding};

pub fn render_loading_overlay(ctx: &Context, state: &mut UiState) {
    let task = if let Some(task) = &mut state.active_loading_task {
        task
    } else {
        return;
    };

    // Fullscreen dimming rect
    ctx.layer_painter(egui::LayerId::new(
        Order::Foreground,
        egui::Id::new("dim_layer"),
    ))
    .rect_filled(
        ctx.screen_rect(),
        Rounding::ZERO,
        Color32::from_black_alpha(160),
    );

    // Centered overlay
    Area::new(egui::Id::new("loading_overlay"))
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .order(Order::Tooltip) // Render on top of everything
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .inner_margin(24.0)
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading(RichText::new(&task.label).size(24.0).strong());
                        ui.add_space(8.0);
                        ui.label(RichText::new(&task.detail).color(Color32::GRAY));
                        ui.add_space(16.0);

                        if task.progress >= 0.0 {
                            ui.add(egui::ProgressBar::new(task.progress).animate(true));
                        } else {
                            ui.spinner();
                        }

                        if task.can_cancel {
                            ui.add_space(16.0);
                            if ui.button("Cancel").clicked() {
                                task.cancel_flag
                                    .store(true, std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                    });
                });
        });
}
