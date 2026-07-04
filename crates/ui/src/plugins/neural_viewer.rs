//! Neural Viewer plugin — renders the CTRNN brain (nodes + synapses) of the
//! currently selected/tracked organism as a node-link graph.

use crate::types::*;

/// Renders the Neural Viewer panel content.
#[allow(clippy::too_many_arguments)]
pub fn neural_viewer_ui(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
    _actions: &mut Vec<MenuAction>,
) {
    let Some(selected) = state.selected_entity.or(state.tracked_entity) else {
        ui.centered_and_justified(|ui| {
            ui.label(
                egui::RichText::new("Select an organism to view its brain")
                    .color(egui::Color32::GRAY)
                    .italics(),
            );
        });
        return;
    };

    // The `Brain` component lives on the organism's core/head entity. If a
    // non-head body segment is selected instead, fall back to the head node
    // of the same organism.
    let brain_entity = {
        let mut brain_q = world.ecs.query::<&brain::Brain>();
        if brain_q.get(&world.ecs, selected).is_ok() {
            Some(selected)
        } else {
            let organism_id = {
                let mut node_q = world.ecs.query::<&physics::ParticleNode>();
                node_q.get(&world.ecs, selected).ok().map(|n| n.organism_id)
            };
            organism_id.and_then(|organism_id| {
                let mut node_q = world
                    .ecs
                    .query::<(bevy_ecs::entity::Entity, &physics::ParticleNode)>();
                node_q
                    .iter(&world.ecs)
                    .find(|(_, n)| n.organism_id == organism_id && n.segment_type == 0)
                    .map(|(e, _)| e)
            })
        }
    };

    let Some(brain_entity) = brain_entity else {
        ui.centered_and_justified(|ui| {
            ui.label(
                egui::RichText::new("Selected organism has no Brain component")
                    .color(egui::Color32::GRAY)
                    .italics(),
            );
        });
        return;
    };

    let mut brain_q = world.ecs.query::<&brain::Brain>();
    let Ok(b) = brain_q.get(&world.ecs, brain_entity) else {
        ui.centered_and_justified(|ui| {
            ui.label(
                egui::RichText::new("Selected organism has no Brain component")
                    .color(egui::Color32::GRAY)
                    .italics(),
            );
        });
        return;
    };

    ui.label(format!(
        "BrainId {} — {} nodes, {} synapses ({} in / {} out / {} hidden)",
        b.id.0,
        b.nodes.len(),
        b.synapses.len(),
        b.input_count,
        b.output_count,
        b.nodes.len().saturating_sub(b.input_count + b.output_count),
    ));
    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| draw_brain_graph(ui, b));
}

/// Draws a CTRNN brain graph: nodes as circles positioned by column (input /
/// hidden / output) and index-within-column, synapses as lines colored by
/// weight sign and shaded by magnitude.
fn draw_brain_graph(ui: &mut egui::Ui, b: &brain::Brain) {
    if b.nodes.is_empty() {
        ui.label(
            egui::RichText::new("Empty network.")
                .italics()
                .color(egui::Color32::GRAY),
        );
        return;
    }

    let height = 240.0_f32.max(b.nodes.len() as f32 * 4.0).min(480.0);
    let (response, painter) = ui.allocate_painter(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let rect = response.rect;
    painter.rect_filled(
        rect,
        egui::Rounding::same(4.0),
        egui::Color32::from_rgb(16, 16, 20),
    );

    // Classify each node index into a column: 0 = input, 1 = hidden, 2 = output.
    let input_count = b.input_count.min(b.nodes.len());
    let output_count = b
        .output_count
        .min(b.nodes.len().saturating_sub(input_count));
    let hidden_start = input_count;
    let hidden_end = b.nodes.len().saturating_sub(output_count);

    let column_of = |idx: usize| -> usize {
        if idx < input_count {
            0
        } else if idx >= hidden_end {
            2
        } else {
            1
        }
    };

    let mut columns: [Vec<usize>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for idx in 0..b.nodes.len() {
        columns[column_of(idx)].push(idx);
    }
    let _ = hidden_start; // only used conceptually above

    let margin = 24.0;
    let usable_w = (rect.width() - 2.0 * margin).max(1.0);
    let usable_h = (rect.height() - 2.0 * margin).max(1.0);

    let mut positions = vec![egui::Pos2::ZERO; b.nodes.len()];
    for (col_idx, indices) in columns.iter().enumerate() {
        let x = rect.left() + margin + usable_w * (col_idx as f32 / 2.0);
        let n = indices.len();
        for (slot, &node_idx) in indices.iter().enumerate() {
            let y = rect.top()
                + margin
                + if n > 1 {
                    usable_h * (slot as f32 / (n - 1) as f32)
                } else {
                    usable_h * 0.5
                };
            positions[node_idx] = egui::pos2(x, y);
        }
    }

    // Synapses first, so nodes render on top.
    for syn in &b.synapses {
        let (source, target) = (syn.source as usize, syn.target as usize);
        if source >= positions.len() || target >= positions.len() {
            continue;
        }
        let strength = syn.weight.abs().min(3.0) / 3.0;
        let color = if syn.weight >= 0.0 {
            egui::Color32::from_rgba_unmultiplied(90, 200, 255, (80.0 + 140.0 * strength) as u8)
        } else {
            egui::Color32::from_rgba_unmultiplied(255, 100, 100, (80.0 + 140.0 * strength) as u8)
        };
        painter.line_segment(
            [positions[source], positions[target]],
            egui::Stroke::new(0.5 + 2.0 * strength, color),
        );
    }

    // Nodes.
    let node_radius = 6.0;
    for (idx, node) in b.nodes.iter().enumerate() {
        let color = match column_of(idx) {
            0 => egui::Color32::from_rgb(120, 220, 120), // input
            2 => egui::Color32::from_rgb(255, 180, 90),  // output
            _ => egui::Color32::from_rgb(150, 170, 255), // hidden
        };
        painter.circle_filled(positions[idx], node_radius, color);
        painter.circle_stroke(
            positions[idx],
            node_radius,
            egui::Stroke::new(1.0, egui::Color32::from_gray(20)),
        );

        // Fill level indicator: a smaller inner dot brightness-scaled by the
        // node's current activation state, so the graph doubles as a live
        // activity monitor.
        let activity = brain::Brain::apply_activation(node.state + node.bias, node.activation);
        let inner_t = (activity.tanh().abs()).clamp(0.0, 1.0);
        painter.circle_filled(
            positions[idx],
            node_radius * 0.5,
            egui::Color32::from_white_alpha((inner_t * 255.0) as u8),
        );
    }

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("● input")
                .small()
                .color(egui::Color32::from_rgb(120, 220, 120)),
        );
        ui.label(
            egui::RichText::new("● hidden")
                .small()
                .color(egui::Color32::from_rgb(150, 170, 255)),
        );
        ui.label(
            egui::RichText::new("● output")
                .small()
                .color(egui::Color32::from_rgb(255, 180, 90)),
        );
        ui.label(
            egui::RichText::new("— blue = excitatory, red = inhibitory")
                .small()
                .color(egui::Color32::GRAY),
        );
    });
}
