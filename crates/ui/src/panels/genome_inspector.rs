use egui::{Color32, Pos2, RichText, Stroke, Ui, Vec2};

pub fn render_genome_inspector(ui: &mut Ui) {
    ui.heading("Genome Inspector");
    ui.label("Base-4 genome bit (secently bitstring)");

    ui.add_space(10.0);

    let bitstring = "ATCGGCTA ATCGGCTA ATCGGCTA ATCGGCTA ATCGGCTA ATCGGCTA ATCGGCTA ATCGGCTA ATCGGCTA ATCGGCTA ATCGGCTA ATCGGCTA ATCGGCTA ATCGGCTA ATCGGCTA ATCGGCTA";
    ui.label(
        RichText::new(bitstring)
            .family(egui::FontFamily::Monospace)
            .color(Color32::from_rgb(255, 180, 0)) // Amber/orange
            .background_color(Color32::from_rgb(15, 15, 20)),
    );

    ui.add_space(20.0);

    let (rect, _response) = ui.allocate_exact_size(Vec2::new(280.0, 150.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);

    // Dark background for plot
    painter.rect_filled(rect, 4.0, Color32::from_rgb(10, 10, 15));

    let num_axes = 6;
    let axis_spacing = rect.width() / (num_axes as f32 + 1.0);

    // Draw vertical axes and labels
    let labels = ["Size", "Diet", "Speed", "Sense", "Metab", "Repro"];
    for (i, label) in labels.iter().enumerate() {
        let x = rect.min.x + axis_spacing * (i as f32 + 1.0);
        painter.line_segment(
            [
                Pos2::new(x, rect.min.y + 20.0),
                Pos2::new(x, rect.max.y - 20.0),
            ],
            Stroke::new(1.0, Color32::from_rgb(50, 50, 70)),
        );

        // Top label
        painter.text(
            Pos2::new(x, rect.min.y + 10.0),
            egui::Align2::CENTER_CENTER,
            *label,
            egui::FontId::proportional(10.0),
            Color32::from_rgb(150, 150, 170),
        );
        // Bottom label
        painter.text(
            Pos2::new(x, rect.max.y - 10.0),
            egui::Align2::CENTER_CENTER,
            format!("Seg {}", i),
            egui::FontId::proportional(10.0),
            Color32::from_rgb(150, 150, 170),
        );
    }

    // Draw colored curves
    let colors = [
        Color32::from_rgb(50, 150, 255), // Blue
        Color32::from_rgb(0, 255, 200),  // Teal
        Color32::from_rgb(255, 150, 50), // Orange
        Color32::from_rgb(255, 220, 50), // Yellow
    ];

    for c in colors {
        let mut prev_pos = None;
        for i in 0..num_axes {
            let x = rect.min.x + axis_spacing * (i as f32 + 1.0);
            // Randomish height
            let height_ratio = ((i * 13 + c.r() as usize) % 100) as f32 / 100.0;
            let y = rect.min.y + 20.0 + (rect.height() - 40.0) * height_ratio;
            let pos = Pos2::new(x, y);

            if let Some(prev) = prev_pos {
                painter.line_segment([prev, pos], Stroke::new(1.5, c));
            }
            prev_pos = Some(pos);
        }
    }
}
