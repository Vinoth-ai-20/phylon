use common::EntityId;
use egui::{Color32, Pos2, Stroke, Ui, Vec2};

#[derive(Default)]
pub struct AnalyticsPanel {
    pub lineage_query_sent: bool,
}

pub fn render_lineage_tree(
    ui: &mut Ui,
    ui_state: &mut crate::state::UiState,
    selected_entities: &[EntityId],
) {
    ui.heading("Lineage Tree");

    if selected_entities.is_empty() {
        ui.label("Select an organism to view its lineage.");
        return;
    }

    let entity_id = selected_entities[0];

    // Trigger DB query if not already sent for this entity (or if results are missing)
    if ui_state.db_query_results.is_none() {
        if let Some(tx) = &ui_state.app_tx {
            let query = format!(
                "WITH RECURSIVE lineage_tree AS (
                    SELECT entity_id, parent_id, generation, birth_tick, death_tick
                    FROM lineages
                    WHERE entity_id = {}
                    UNION ALL
                    SELECT l.entity_id, l.parent_id, l.generation, l.birth_tick, l.death_tick
                    FROM lineages l
                    INNER JOIN lineage_tree lt ON l.parent_id = lt.entity_id
                )
                SELECT entity_id, parent_id, generation, birth_tick, death_tick FROM lineage_tree;",
                entity_id.0
            );
            let _ = tx.send(crate::commands::AppCommand::RunDbQuery(query));
            ui.label("Fetching lineage data...");
        } else {
            ui.label("Database query channel unavailable.");
        }
        return;
    }

    if let Some(Ok(results)) = &ui_state.db_query_results {
        if results.is_empty() {
            ui.label("No lineage data found.");
            return;
        }

        // Extremely simple node graph rendering using egui Painter
        let (rect, _response) =
            ui.allocate_exact_size(Vec2::new(400.0, 300.0), egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);

        // Background
        painter.rect_filled(rect, 4.0, Color32::from_rgb(20, 20, 25));

        // Draw nodes (basic visualization)
        let center_x = rect.min.x + rect.width() / 2.0;
        let mut y_offset = 20.0;

        for row in results {
            if row.len() >= 3 {
                let id = &row[0];
                let gen = &row[2];
                let text = format!("ID: {} (Gen {})", id, gen);

                let node_pos = Pos2::new(center_x, rect.min.y + y_offset);

                // Draw connecting line to parent (stubbed)
                if y_offset > 20.0 {
                    let parent_pos = Pos2::new(center_x, rect.min.y + y_offset - 40.0);
                    painter.line_segment(
                        [parent_pos, node_pos],
                        Stroke::new(1.0, Color32::from_rgb(100, 100, 150)),
                    );
                }

                // Draw node
                painter.circle_filled(node_pos, 5.0, Color32::from_rgb(0, 200, 255));
                painter.text(
                    node_pos + Vec2::new(10.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    text,
                    egui::FontId::proportional(12.0),
                    Color32::WHITE,
                );

                y_offset += 40.0;
            }
        }
    } else if let Some(Err(e)) = &ui_state.db_query_results {
        ui.colored_label(Color32::RED, format!("DB Error: {}", e));
    }
}
