use egui::{Color32, RichText, Ui};

pub fn tab_strip_vertical(
    ui: &mut Ui,
    tabs: &[(&str, &str)], // (icon, tooltip)
    active: usize,
    on_change: &mut dyn FnMut(usize),
) {
    ui.allocate_ui(egui::vec2(32.0, ui.available_height()), |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            for (i, &(icon, tooltip)) in tabs.iter().enumerate() {
                let is_active = i == active;
                let bg_color = if is_active {
                    Color32::from_rgb(40, 45, 60)
                } else {
                    Color32::TRANSPARENT
                };

                let btn = egui::Button::new(RichText::new(icon).size(20.0))
                    .min_size(egui::vec2(28.0, 28.0))
                    .fill(bg_color)
                    .frame(is_active); // Only show frame if active

                // Hover overrides for inactive
                let mut style = ui.style().as_ref().clone();
                if !is_active {
                    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(30, 33, 44);
                    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
                } else {
                    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(50, 55, 70);
                }
                ui.style_mut().visuals = style.visuals;

                let response = ui.add(btn).on_hover_text(tooltip);
                if response.clicked() {
                    on_change(i);
                }
                ui.add_space(4.0);
            }
        });
    });
}
