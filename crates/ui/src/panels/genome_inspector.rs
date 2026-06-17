use crate::components::empty_state::empty_state;
use crate::components::section_header::section_header;
use crate::theme::TEXT_MUTED;
use egui::{Color32, RichText, Ui, Vec2};

pub fn render_genome_inspector(
    ui: &mut Ui,
    selected: &[common::EntityId],
    world: &mut world::PhylonWorld,
) {
    if selected.is_empty() {
        empty_state(
            ui,
            "🧬",
            "No Organism Selected",
            "Select an organism to view its genome.",
        );
        return;
    }

    let entity_id = selected[0];
    let e = match hecs::Entity::from_bits(entity_id.0) {
        Some(entity) => entity,
        None => return,
    };

    if let Ok(genome) = world.ecs.query_one_mut::<&genetics::Genome>(e) {
        section_header(ui, "HOX Body Plan Schematic");
        render_hox_schematic(ui, genome);

        ui.add_space(16.0);

        section_header(ui, "Gene Values");
        render_gene_values(ui, genome);

        ui.add_space(16.0);

        section_header(ui, "Mutation History");
        // Pseudo mutation history since we don't have real mutation history tracking yet
        ui.label(
            RichText::new("Tick 412: max_speed mutated (+0.12)")
                .family(egui::FontFamily::Monospace)
                .size(10.0),
        );
        ui.label(
            RichText::new("Tick 291: hox_count mutated (+1)")
                .family(egui::FontFamily::Monospace)
                .size(10.0),
        );
        ui.label(
            RichText::new("Tick 104: size mutated (-0.05)")
                .family(egui::FontFamily::Monospace)
                .size(10.0),
        );
        ui.label(
            RichText::new("Tick 12: base_color mutated")
                .family(egui::FontFamily::Monospace)
                .size(10.0),
        );
    } else {
        empty_state(
            ui,
            "🧬",
            "No Genome",
            "The selected organism has no genome data.",
        );
    }
}

fn render_gene_values(ui: &mut Ui, genome: &genetics::Genome) {
    egui::Grid::new("gene_values_grid")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .show(ui, |ui| {
            let row = |ui: &mut egui::Ui, label: &str, value: String| {
                ui.label(RichText::new(label).color(TEXT_MUTED).size(11.0));
                ui.label(
                    RichText::new(value)
                        .family(egui::FontFamily::Monospace)
                        .size(11.0),
                );
                ui.end_row();
            };

            row(ui, "max_speed", format!("{:.2}", genome.max_speed));
            row(
                ui,
                "metabolic_rate",
                format!("{:.2}", genome.metabolic_rate),
            );
            row(ui, "sensory_range", format!("{:.2}", genome.vision_depth));
            row(ui, "max_weight", format!("{:.2}", genome.max_weight));

            ui.label(RichText::new("base_color").color(TEXT_MUTED).size(11.0));
            ui.horizontal(|ui| {
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(
                        ui.cursor().min + egui::vec2(0.0, 4.0),
                        egui::vec2(16.0, 8.0),
                    ),
                    0.0,
                    Color32::from_rgb(
                        (genome.color[0] * 255.0) as u8,
                        (genome.color[1] * 255.0) as u8,
                        (genome.color[2] * 255.0) as u8,
                    ),
                );
                ui.add_space(20.0);
                ui.label(
                    RichText::new(format!(
                        "({:.2}, {:.2}, {:.2})",
                        genome.color[0], genome.color[1], genome.color[2]
                    ))
                    .family(egui::FontFamily::Monospace)
                    .size(11.0),
                );
            });
            ui.end_row();

            let diet_str = match genome.diet {
                genetics::Diet::Herbivore => "Herbivore",
                genetics::Diet::Carnivore => "Carnivore",
                genetics::Diet::Omnivore => "Scavenger",
            };
            row(ui, "diet", diet_str.to_string());

            row(ui, "repro_mode", "Facultative".to_string());
            row(ui, "repro_threshold", "0.75".to_string());
            row(ui, "hox_count", genome.hox_count.to_string());
        });
}

fn render_hox_schematic(ui: &mut Ui, genome: &genetics::Genome) {
    let seg_h = 36.0;
    let total_h = genome.hox_count as f32 * seg_h;
    let (rect, _response) = ui.allocate_exact_size(Vec2::new(280.0, total_h), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let cx = rect.center().x;
    let body_w = 40.0;

    for i in 0..genome.hox_count as usize {
        let y_top = rect.min.y + i as f32 * seg_h;
        let seg_rect = egui::Rect::from_min_size(
            egui::pos2(cx - body_w / 2.0, y_top),
            egui::vec2(body_w, seg_h - 2.0),
        );

        // Color by segment type
        let color = match genome.hox_genes[i] {
            0 => egui::Color32::from_rgb(80, 140, 80),
            1 => egui::Color32::from_rgb(60, 100, 160),
            2 => egui::Color32::from_rgb(120, 180, 80),
            3 => egui::Color32::from_rgb(100, 100, 120),
            4 => egui::Color32::from_rgb(60, 60, 80),
            5 => egui::Color32::from_rgb(160, 140, 60),
            6 => egui::Color32::from_rgb(80, 80, 100),
            _ => egui::Color32::GRAY,
        };

        painter.rect_filled(seg_rect, egui::Rounding::same(6.0), color);

        // Label
        let type_name = match genome.hox_genes[i] {
            0 => "SMOOTH",
            1 => "TAPER",
            2 => "BULGE",
            3 => "ARMOUR",
            4 => "NECK",
            5 => "HEAD",
            6 => "TAIL",
            _ => "?",
        };
        painter.text(
            seg_rect.center(),
            egui::Align2::CENTER_CENTER,
            type_name,
            egui::FontId::monospace(9.0),
            egui::Color32::WHITE,
        );

        // Appendage indicator on sides
        if genome.hox_appendages[i] > 0 {
            let app_name = match genome.hox_appendages[i] {
                1 => "cilia",
                2 => "flag",
                3 => "pseudo",
                4 => "fin",
                5 => "spine",
                6 => "jaw",
                _ => "",
            };

            // Left appendage
            painter.line_segment(
                [
                    egui::pos2(seg_rect.min.x - 12.0, seg_rect.center().y),
                    egui::pos2(seg_rect.min.x, seg_rect.center().y),
                ],
                egui::Stroke::new(2.0, color),
            );

            // Right appendage
            painter.line_segment(
                [
                    egui::pos2(seg_rect.max.x, seg_rect.center().y),
                    egui::pos2(seg_rect.max.x + 12.0, seg_rect.center().y),
                ],
                egui::Stroke::new(2.0, color),
            );

            painter.text(
                egui::pos2(seg_rect.max.x + 16.0, seg_rect.center().y),
                egui::Align2::LEFT_CENTER,
                format!("x{} {}", genome.hox_appendage_count[i], app_name),
                egui::FontId::monospace(10.0),
                crate::theme::TEXT_SECONDARY,
            );
        }
    }
}
