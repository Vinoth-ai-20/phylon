use crate::theme::{BG_PANEL, BORDER_DEFAULT, TEXT_MUTED};
use egui::{Area, Color32, Frame, Id, Order, Pos2, Rect, RichText};

pub fn render_search_bar(
    ctx: &egui::Context,
    ui_state: &mut crate::state::UiState,
    viewport_rect: Rect,
) {
    if !ui_state.is_search_active {
        return;
    }

    Area::new(Id::new("search_bar_overlay"))
        .fixed_pos(Pos2::new(
            viewport_rect.min.x + 16.0,
            viewport_rect.min.y + 16.0,
        ))
        .order(Order::Foreground)
        .show(ctx, |ui| {
            let frame = Frame::none()
                .fill(BG_PANEL)
                .stroke(egui::Stroke::new(1.0, BORDER_DEFAULT))
                .rounding(20.0)
                .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                .shadow(egui::epaint::Shadow {
                    offset: egui::vec2(0.0, 6.0),
                    blur: 12.0,
                    spread: 0.0,
                    color: Color32::from_black_alpha(150),
                });

            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS)
                            .color(TEXT_MUTED)
                            .size(16.0),
                    );

                    let response = ui.add(
                        egui::TextEdit::singleline(&mut ui_state.search_query)
                            .desired_width(240.0)
                            .hint_text("ID, species, or gene (e.g. has:flagella)")
                            .frame(false)
                            .font(egui::FontId::proportional(14.0)),
                    );

                    response.request_focus(); // Auto-focus when activated

                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        // Perform search
                        // Placeholder: just select first entity if query is a number
                        if let Ok(id) = ui_state.search_query.parse::<u64>() {
                            ui_state.selected_entities = vec![common::EntityId(id)];
                        }
                        ui_state.is_search_active = false;
                        ui_state.search_query.clear();
                    }

                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        ui_state.is_search_active = false;
                        ui_state.search_query.clear();
                    }

                    if ui.button("✕").clicked() {
                        ui_state.is_search_active = false;
                        ui_state.search_query.clear();
                    }
                });
            });
        });
}
