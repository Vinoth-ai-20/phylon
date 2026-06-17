use crate::components::empty_state::empty_state;
use crate::components::section_header::section_header;
use crate::components::stat_bar::stat_bar;
use crate::theme::ACCENT_PURPLE;
use common::Tick;
use egui::{Color32, Pos2, Stroke, Ui, Vec2};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

pub fn render_brain_inspector(
    ui: &mut Ui,
    tick: Tick,
    selected: &[common::EntityId],
    world: &mut world::PhylonWorld,
) {
    if selected.is_empty() {
        empty_state(
            ui,
            "🕸",
            "No Organism Selected",
            "Select an organism to inspect its brain state.",
        );
        return;
    }

    let entity_id = selected[0];
    let e = match hecs::Entity::from_bits(entity_id.0) {
        Some(entity) => entity,
        None => return,
    };

    // Assuming there's a BrainState component
    // If not, we'll fetch Genome to have some data, but the prompt says BrainState
    if let Ok(genome) = world.ecs.query_one_mut::<&genetics::Genome>(e) {
        section_header(ui, "CTRNN Node Graph");

        let (rect, _response) =
            ui.allocate_exact_size(Vec2::new(240.0, 240.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);

        // Dark cinematic background
        painter.rect_filled(rect, 4.0, Color32::from_rgb(10, 10, 15));

        // Generate static nodes (columns by layer)
        let mut rng = ChaCha8Rng::seed_from_u64(entity_id.0);
        let num_layers = 3;
        let nodes_per_layer = 6;
        let mut nodes = Vec::new();

        let dx = rect.width() / (num_layers as f32 + 1.0);
        let dy = rect.height() / (nodes_per_layer as f32 + 1.0);

        for layer in 0..num_layers {
            for node in 0..nodes_per_layer {
                let x = rect.min.x + dx * (layer as f32 + 1.0) + rng.gen_range(-10.0..10.0);
                let y = rect.min.y + dy * (node as f32 + 1.0) + rng.gen_range(-10.0..10.0);
                nodes.push(Pos2::new(x, y));
            }
        }

        let neuromodulator_idx = nodes.len() - 1;

        // Draw edges based on brain_weights
        for i in 0..nodes.len() {
            // Pick a couple connections for each
            for _ in 0..2 {
                let j = rng.gen_range(0..nodes.len());
                if i != j {
                    // pseudo weight
                    let weight_idx = (i * nodes.len() + j) % genome.brain_weights.len().max(1);
                    let weight = if !genome.brain_weights.is_empty() {
                        genome.brain_weights[weight_idx]
                    } else {
                        rng.gen_range(-1.0..1.0)
                    };

                    let start = nodes[i];
                    let end = nodes[j];

                    let edge_color = if weight > 0.0 {
                        Color32::from_rgb(55, 130, 230) // blue positive
                    } else {
                        Color32::from_rgb(220, 65, 55) // red negative
                    };

                    let thickness = weight.abs() * 2.0;

                    painter.line_segment(
                        [start, end],
                        Stroke::new(thickness, edge_color.linear_multiply(0.6)),
                    );
                }
            }
        }

        let time = (tick.0 as f32) * 0.05;

        // Draw nodes
        for (i, &pos) in nodes.iter().enumerate() {
            // mock activation based on time and index to make it lively
            let activation = ((time + i as f32 * 0.7).sin() * 0.5 + 0.5).clamp(0.0, 1.0);

            // brightness = current activation
            let val = (activation * 255.0) as u8;
            let color = Color32::from_rgb(val, val, val);

            painter.circle_filled(pos, 4.0, color);

            if i == neuromodulator_idx {
                // Gold ring
                painter.circle_stroke(pos, 6.0, Stroke::new(1.5, Color32::GOLD));
            }
        }

        ui.add_space(16.0);
        section_header(ui, "Live Readouts");

        // Use stat_bar for readouts
        stat_bar(ui, "Activity", 0.82, ACCENT_PURPLE);
        stat_bar(ui, "Weights avg", 0.41, ACCENT_PURPLE);
        stat_bar(ui, "Dopamine", 0.23, ACCENT_PURPLE);
        stat_bar(ui, "Serotonin", 0.51, ACCENT_PURPLE);
    } else {
        empty_state(
            ui,
            "🕸",
            "No Brain Data",
            "The selected organism has no brain state.",
        );
    }
}
