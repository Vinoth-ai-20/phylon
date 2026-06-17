use crate::theme::{TEXT_MUTED, TEXT_PRIMARY};
use egui::{Color32, RichText, Ui, Vec2};

pub fn stat_bar(ui: &mut Ui, label: &str, value: f32, color: Color32) {
    let height = 14.0;

    ui.horizontal(|ui| {
        ui.allocate_ui(Vec2::new(72.0, height), |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.label(RichText::new(label).size(10.0).color(TEXT_MUTED));
            });
        });

        let available_width = ui.available_width();
        let value_str = format!("{:.0}", value * 100.0); // e.g. 0.82 -> 82

        // Calculate text width approximately for the value (10px monospace)
        let text_width = 24.0;
        let bar_width = available_width - text_width - 8.0; // 8px gap

        let (rect, _response) =
            ui.allocate_exact_size(Vec2::new(bar_width, height), egui::Sense::hover());

        let painter = ui.painter();

        // Track
        painter.rect_filled(rect, 2.0, Color32::from_rgb(25, 28, 38));

        // Fill
        let fill_width = bar_width * value.clamp(0.0, 1.0);
        if fill_width > 0.0 {
            painter.rect_filled(
                egui::Rect::from_min_size(rect.min, Vec2::new(fill_width, height)),
                2.0,
                color,
            );
        }

        ui.allocate_ui(Vec2::new(text_width, height), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(value_str)
                        .family(egui::FontFamily::Monospace)
                        .size(10.0)
                        .color(TEXT_PRIMARY),
                );
            });
        });
    });
}
