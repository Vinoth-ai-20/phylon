use egui::{Color32, Pos2, RichText, Stroke, Ui, Vec2};

pub fn render_genome_inspector(
    ui: &mut Ui,
    selected: &[common::EntityId],
    world: &mut world::PhylonWorld,
) {
    ui.heading("Genome Inspector");
    ui.label("Genetic Traits & Parameters");

    ui.add_space(10.0);

    if selected.is_empty() {
        ui.label("Select an organism to view its genome.");
        return;
    }

    let entity_id = selected[0];
    let e = match hecs::Entity::from_bits(entity_id.0) {
        Some(entity) => entity,
        None => return,
    };

    if let Ok(genome) = world.ecs.query_one_mut::<&genetics::Genome>(e) {
        // Generate a pseudo sequence from the entity ID to look cool
        let seed = entity_id.0;
        let mut sequence = String::new();
        let chars = ['A', 'T', 'C', 'G'];
        for i in 0..16 {
            let idx = (seed.rotate_left(i as u32) % 4) as usize;
            sequence.push(chars[idx]);
            if i % 4 == 3 {
                sequence.push(' ');
            }
        }

        ui.label(
            RichText::new(sequence)
                .family(egui::FontFamily::Monospace)
                .color(Color32::from_rgb(255, 180, 0)) // Amber/orange
                .background_color(Color32::from_rgb(15, 15, 20)),
        );

        ui.add_space(20.0);

        let (rect, _response) =
            ui.allocate_exact_size(Vec2::new(280.0, 150.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);

        // Dark background for plot
        painter.rect_filled(rect, 4.0, Color32::from_rgb(10, 10, 15));

        let num_axes = 6;
        let axis_spacing = rect.width() / (num_axes as f32 + 1.0);

        let labels = ["Speed", "Metab", "VisAng", "VisDep", "Size", "Weight"];
        let values = [
            genome.max_speed / 200.0, // normalize based on lib.rs limits
            genome.metabolic_rate / 5.0,
            genome.vision_cone_angle / std::f32::consts::PI,
            genome.vision_depth / 500.0,
            genome.size / 20.0,
            genome.max_weight / 50.0,
        ];

        // Draw vertical axes and labels
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
        }

        // Draw genome parameter line
        let mut prev_pos = None;
        for (i, value) in values.iter().enumerate().take(num_axes) {
            let x = rect.min.x + axis_spacing * (i as f32 + 1.0);
            let height_ratio = 1.0 - value.clamp(0.0, 1.0); // 0 is bottom, 1 is top
            let y = rect.min.y + 20.0 + (rect.height() - 40.0) * height_ratio;
            let pos = Pos2::new(x, y);

            painter.circle_filled(pos, 3.0, Color32::from_rgb(0, 255, 200));

            if let Some(prev) = prev_pos {
                painter.line_segment(
                    [prev, pos],
                    Stroke::new(2.0, Color32::from_rgb(0, 255, 200)),
                );
            }
            prev_pos = Some(pos);
        }
    } else {
        ui.label("No genome available.");
    }
}
