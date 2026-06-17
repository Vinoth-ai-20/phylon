use crate::theme::TEXT_MUTED;
use egui::{RichText, Ui};

pub fn empty_state(ui: &mut Ui, icon: &str, title: &str, subtitle: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() / 2.0 - 40.0);
        ui.label(RichText::new(icon).size(32.0));
        ui.add_space(8.0);
        ui.heading(title);
        ui.label(RichText::new(subtitle).size(11.0).color(TEXT_MUTED));
    });
}
