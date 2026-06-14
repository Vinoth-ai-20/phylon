use common::Tick;
use egui::{Color32, Pos2, Stroke, Ui, Vec2};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

pub fn render_brain_inspector(ui: &mut Ui, tick: Tick) {
    ui.heading("Brain Inspector");
    ui.label("Node-graph is the CTRNN");

    let (rect, _response) = ui.allocate_exact_size(Vec2::new(280.0, 280.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);

    // Dark cinematic background
    painter.rect_filled(rect, 4.0, Color32::from_rgb(10, 10, 15));

    // Generate static nodes
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let mut nodes = Vec::new();
    let num_nodes = 20;

    for _ in 0..num_nodes {
        let x = rect.min.x + rng.gen_range(20.0..rect.width() - 20.0);
        let y = rect.min.y + rng.gen_range(20.0..rect.height() - 20.0);
        nodes.push(Pos2::new(x, y));
    }

    // Designate the last node as the neuromodulator output
    let neuromodulator_idx = num_nodes - 1;

    // Generate connections
    let mut edges = Vec::new();
    for i in 0..num_nodes {
        for _ in 0..2 {
            let j = rng.gen_range(0..num_nodes);
            if i != j {
                // weight, is_neuromodulator
                let weight = rng.gen_range(-1.0..1.0);
                let is_neuro = j == neuromodulator_idx || rng.gen_bool(0.1);
                edges.push((i, j, weight, is_neuro));
            }
        }
    }

    let time = (tick.0 as f32) * 0.05;

    // Draw edges
    for &(i, j, weight, is_neuro) in &edges {
        let start = nodes[i];
        let end = nodes[j];

        let edge_color = if is_neuro {
            Color32::from_rgb(0, 255, 100) // Green
        } else if weight > 0.0 {
            Color32::from_rgb(50, 150, 255) // Blue for positive
        } else {
            Color32::from_rgb(255, 50, 150) // Pink/Red for negative
        };

        painter.line_segment(
            [start, end],
            Stroke::new(1.0, edge_color.linear_multiply(0.5)),
        );
    }

    // Draw nodes
    for (i, &pos) in nodes.iter().enumerate() {
        let activation = (time + i as f32 * 0.5).sin() * 0.5 + 0.5;
        let radius = 2.0 + activation * 4.0;

        let color = if i == neuromodulator_idx {
            Color32::from_rgb(255, 50, 255) // Magenta/pink highlight
        } else {
            Color32::from_rgb(100, 100, 150)
        };

        painter.circle_filled(pos, radius, color);

        if i == neuromodulator_idx {
            // Gold ring
            painter.circle_stroke(pos, radius + 2.0, Stroke::new(1.5, Color32::GOLD));
        }
    }

    ui.add_space(10.0);

    // Sliders
    let mut activity_val = 100.0;
    ui.horizontal(|ui| {
        ui.label("Activity");
        ui.add(egui::Slider::new(&mut activity_val, 0.0..=100.0).show_value(true));
    });

    let mut weight_val = 50.0;
    ui.horizontal(|ui| {
        ui.label("Weights");
        ui.add(egui::Slider::new(&mut weight_val, 0.0..=100.0).text("dopamine"));
    });

    let mut neuro_val = 75.0;
    ui.horizontal(|ui| {
        ui.label("Neuromodulator");
        ui.add(egui::Slider::new(&mut neuro_val, 0.0..=100.0).text("serotonin"));
    });
}
