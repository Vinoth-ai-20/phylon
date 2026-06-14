use common::EntityId;
use egui::{Color32, RichText, Ui};

pub fn render_entity_inspector(ui: &mut Ui, selected: &[EntityId]) {
    ui.heading("Entity Inspector");

    if selected.is_empty() {
        ui.label("Click an organism to inspect");
        return;
    }

    let entity_id = selected[0];
    ui.label(format!("Inspecting Entity {}", entity_id.0));

    ui.separator();

    // Rows requested in prompt: Stats, Scales, Colonors, Transparency, Transpometters, Energy Levels, Energy Deats, Death cauries, Oreation
    egui::Grid::new("entity_stats_grid")
        .striped(true)
        .num_columns(2)
        .show(ui, |ui| {
            let labels = [
                ("Stats", "Active"),
                ("Scales", "0.8"),
                ("Colonors", "Blue-green"),
                ("Transparency", "0.9"),
                ("Transpometters", "High"),
                ("Energy Levels", "85%"),
                ("Energy Deats", "0"),
                ("Death cauries", "None"),
                ("Oreation", "Omnivore"),
            ];

            for (label, val) in labels {
                ui.label(RichText::new(label).color(Color32::from_rgb(150, 150, 170)));
                ui.label(RichText::new(val).color(Color32::WHITE));
                ui.end_row();
            }
        });
}
