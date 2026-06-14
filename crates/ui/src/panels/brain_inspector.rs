use common::Tick;
use egui::{Color32, Pos2, Stroke, Ui, Vec2};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

pub fn render_brain_inspector(ui: &mut Ui, tick: Tick) {
    ui.heading("Neural Activity Visualization");

    let (rect, _response) = ui.allocate_exact_size(Vec2::new(400.0, 400.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);

    // Dark cinematic background
    painter.rect_filled(rect, 4.0, Color32::from_rgb(10, 10, 15));

    // Generate static nodes
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let mut nodes = Vec::new();
    let num_nodes = 30;

    for _ in 0..num_nodes {
        let x = rect.min.x + rng.gen_range(20.0..rect.width() - 20.0);
        let y = rect.min.y + rng.gen_range(20.0..rect.height() - 20.0);
        nodes.push(Pos2::new(x, y));
    }

    // Generate connections
    let mut edges = Vec::new();
    for i in 0..num_nodes {
        // Connect to 2 or 3 nearby nodes
        for _ in 0..3 {
            let j = rng.gen_range(0..num_nodes);
            if i != j {
                edges.push((i, j));
            }
        }
    }

    // Animation time
    let time = (tick.0 as f32) * 0.05;

    // Draw edges with pulses
    for &(i, j) in &edges {
        let start = nodes[i];
        let end = nodes[j];

        // Base line
        painter.line_segment(
            [start, end],
            Stroke::new(1.0, Color32::from_rgba_premultiplied(50, 50, 70, 100)),
        );

        // Pulse logic
        let dist = start.distance(end);
        if dist > 0.0 {
            // Pseudo-random phase per edge
            let phase = ((i * j) as f32 * 0.1) % std::f32::consts::TAU;
            let pulse_pos = (time + phase) % 5.0; // Wraps every 5 seconds

            if (0.0..=1.0).contains(&pulse_pos) {
                let current_pos = start.lerp(end, pulse_pos);
                let pulse_color = if (i + j) % 2 == 0 {
                    Color32::from_rgb(0, 255, 255) // Cyan
                } else {
                    Color32::from_rgb(255, 180, 0) // Amber
                };
                painter.circle_filled(current_pos, 3.0, pulse_color);
            }
        }
    }

    // Draw nodes
    for (i, &pos) in nodes.iter().enumerate() {
        let pulse_intensity = ((time + i as f32 * 0.5).sin() * 0.5 + 0.5) * 200.0;
        let color = Color32::from_rgb(0, pulse_intensity as u8, 255);
        painter.circle_filled(pos, 4.0, color);
    }
}
