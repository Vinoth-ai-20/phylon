use crate::theme::BORDER_DEFAULT;
use egui::{RichText, Ui};

pub fn section_header(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).size(11.0).strong());
    ui.add_space(2.0);
    let rect = ui.cursor();
    ui.painter().line_segment(
        [
            egui::pos2(rect.min.x, rect.min.y),
            egui::pos2(rect.min.x + ui.available_width(), rect.min.y),
        ],
        egui::Stroke::new(1.0, BORDER_DEFAULT),
    );
    ui.add_space(4.0);
}
