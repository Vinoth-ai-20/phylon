//! GRN Viewer panel (Phase 3, M11) — graph layout of the selected
//! organism's `genetics::RegulatoryNetwork`, developmental-step time
//! playback, and a mutation-vs-parent comparison, following
//! `PHASE3_ROADMAP.md` §8's design.
//!
//! Reuses `crate::graph_canvas`'s pan/zoom/hit-test helpers (extracted from
//! `plugins::neural_viewer` for this milestone) rather than reimplementing
//! node-link graph navigation — `RegulatoryNetwork` is structurally the
//! same shape (nodes + signed weighted edges) as the `Cppn`/`Brain` graphs
//! Neural Viewer already draws.

const NODE_HOX: egui::Color32 = egui::Color32::from_rgb(255, 180, 90);
const NODE_DIFFERENTIATION: egui::Color32 = egui::Color32::from_rgb(150, 170, 255);
const NODE_EFFECTOR: egui::Color32 = egui::Color32::from_rgb(120, 220, 120);
const NODE_PIGMENT: egui::Color32 = egui::Color32::from_rgb(230, 140, 210);
const EDGE_ACTIVATOR_BASE: egui::Color32 = egui::Color32::from_rgb(90, 200, 255);
const EDGE_REPRESSOR_BASE: egui::Color32 = egui::Color32::from_rgb(255, 100, 100);
const CANVAS_BG: egui::Color32 = egui::Color32::from_rgb(14, 18, 26);

/// Renders the GRN Viewer tab for `state.selected_entity.or(state.tracked_entity)`.
pub fn grn_viewer_ui(
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
) {
    let entity = match state.selected_entity.or(state.tracked_entity) {
        Some(e) => e,
        None => {
            crate::widgets::empty_state(
                ui,
                "Select an organism to inspect its regulatory network.",
            );
            return;
        }
    };

    let genome = {
        let mut genome_q = world.ecs.query::<&genetics::Genome>();
        let Ok(genome) = genome_q.get(&world.ecs, entity) else {
            crate::widgets::empty_state(ui, "Genome not on this node. Select the head node.");
            return;
        };
        genome.clone()
    };

    playback_controls(ui, state);
    ui.add_space(crate::theme::SPACE_SM);

    let expressed = genome.expressed_regulatory_cppn();
    let network = developed_network(&expressed, state.grn_position, state.grn_step);
    draw_grn_graph(ui, &network, &mut state.grn_view);

    ui.add_space(crate::theme::SPACE_MD);
    ui.separator();
    ui.label(egui::RichText::new("Mutation vs. Parent").strong());
    mutation_comparison(
        ui,
        world,
        entity,
        &genome,
        state.grn_position,
        state.grn_step,
    );
}

use crate::regulatory_view::{developed_network, node_label};

fn playback_controls(ui: &mut egui::Ui, state: &mut crate::WorkbenchState) {
    ui.horizontal(|ui| {
        ui.label("Position");
        ui.add(egui::Slider::new(
            &mut state.grn_position,
            0..=(organisms::MAX_SEGMENTS - 1),
        ));
    });
    ui.horizontal(|ui| {
        ui.label("Developmental step");
        ui.add(egui::Slider::new(
            &mut state.grn_step,
            0..=genetics::develop::DEVELOPMENT_STEPS,
        ));
    });
}

fn node_color(role: genetics::RegulatoryGeneRole) -> egui::Color32 {
    match role {
        genetics::RegulatoryGeneRole::Hox => NODE_HOX,
        genetics::RegulatoryGeneRole::Differentiation => NODE_DIFFERENTIATION,
        genetics::RegulatoryGeneRole::Effector => NODE_EFFECTOR,
        genetics::RegulatoryGeneRole::Pigment => NODE_PIGMENT,
    }
}

/// Draws `network` as a node-link graph: nodes arranged in a circle
/// (a fixed, deterministic layout — appropriate for this milestone's small,
/// fixed 10-gene vocabulary; a force-directed layout would be overkill),
/// edges colored by activator (blue, positive weight) vs. repressor (red,
/// negative weight) exactly like Neural Viewer's CPPN canvas, and node fill
/// brightness encoding the gene's live expression level (`state`, in
/// `[0, 1]` after `RegulatoryGeneNode`'s sigmoid activation).
fn draw_grn_graph(
    ui: &mut egui::Ui,
    network: &genetics::RegulatoryNetwork,
    view: &mut crate::state::GraphViewState,
) {
    if network.nodes.is_empty() {
        ui.label(
            egui::RichText::new("Empty regulatory network.")
                .italics()
                .color(crate::theme::DISABLED_FG),
        );
        return;
    }

    let height = 260.0;
    let (response, painter) = ui.allocate_painter(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click_and_drag(),
    );
    crate::graph_canvas::handle_pan_zoom(ui, &response, view);
    let rect = response.rect;
    painter.rect_filled(rect, egui::Rounding::same(4.0), CANVAS_BG);

    let n = network.nodes.len();
    let radius = (rect.width().min(rect.height()) / 2.0 - 24.0).max(10.0);
    let center = rect.center();
    let mut positions = vec![egui::Pos2::ZERO; n];
    for (i, pos) in positions.iter_mut().enumerate() {
        let angle = std::f32::consts::TAU * (i as f32) / (n as f32) - std::f32::consts::FRAC_PI_2;
        *pos = center + egui::vec2(angle.cos(), angle.sin()) * radius;
        *pos = crate::graph_canvas::apply_view(*pos, rect, view);
    }
    let node_radius = 7.0 * view.zoom;

    for edge in &network.edges {
        if edge.source >= positions.len() || edge.target >= positions.len() {
            continue;
        }
        let strength = (edge.weight.abs() / 3.0).min(1.0);
        let alpha = (80.0 + 140.0 * strength) as u8;
        let base = if edge.weight >= 0.0 {
            EDGE_ACTIVATOR_BASE
        } else {
            EDGE_REPRESSOR_BASE
        };
        let color = egui::Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), alpha);
        painter.line_segment(
            [positions[edge.source], positions[edge.target]],
            egui::Stroke::new(0.5 + 2.0 * strength, color),
        );
    }

    for (i, node) in network.nodes.iter().enumerate() {
        let role = genetics::REGULATORY_GENE_ROLES[i];
        let base = node_color(role);
        // Expression level modulates brightness so "live expression levels"
        // (§8) reads directly from the graph, not just the hover tooltip.
        let expression = node.state.clamp(0.0, 1.0);
        let fill = egui::Color32::from_rgb(
            (base.r() as f32 * expression) as u8,
            (base.g() as f32 * expression) as u8,
            (base.b() as f32 * expression) as u8,
        );
        painter.circle_filled(positions[i], node_radius, fill);
        painter.circle_stroke(
            positions[i],
            node_radius,
            egui::Stroke::new(1.0, egui::Color32::from_gray(200)),
        );
        painter.text(
            positions[i] + egui::vec2(0.0, node_radius + 10.0),
            egui::Align2::CENTER_CENTER,
            node_label(i),
            egui::FontId::proportional(9.0),
            crate::theme::DISABLED_FG,
        );
    }

    if let Some(pointer) = response.hover_pos() {
        if let Some(idx) = crate::graph_canvas::hit_test_node(pointer, &positions, node_radius) {
            let node = &network.nodes[idx];
            let role = genetics::REGULATORY_GENE_ROLES[idx];
            egui::show_tooltip_at_pointer(
                ui.ctx(),
                ui.layer_id(),
                egui::Id::new("grn_node_tooltip"),
                |ui| {
                    ui.label(format!("{} (gene {idx}, {role:?})", node_label(idx)));
                    ui.label(format!("Bias: {:.3}", node.bias));
                    ui.label(format!("Expression: {:.3}", node.state));
                },
            );
        }
    }

    ui.label(
        egui::RichText::new("Scroll to zoom · drag to pan")
            .small()
            .color(crate::theme::DISABLED_FG),
    );
}

/// Compares the selected organism's regulatory genes against its recorded
/// parent's (via `evolution::LineageTracker`), reusing Recent Selections'
/// comparison instinct (Phase 2, M13) at gene granularity rather than
/// building a new diff UI from scratch. Gracefully reports "no parent
/// data" when the lineage record, parent id, or parent entity isn't
/// available — a dead/despawned parent is the common case, not an error.
fn mutation_comparison(
    ui: &mut egui::Ui,
    world: &mut world::World,
    entity: bevy_ecs::entity::Entity,
    genome: &genetics::Genome,
    position: usize,
    step: usize,
) {
    let Some(tracker) = world.ecs.get_resource::<evolution::LineageTracker>() else {
        crate::widgets::empty_state(ui, "Lineage tracking not available.");
        return;
    };
    let self_id = common::EntityId(entity.to_bits());
    let Some(record) = tracker.get_record(self_id) else {
        crate::widgets::empty_state(ui, "No lineage record for this organism.");
        return;
    };
    let Some(parent_id) = record.parent_id else {
        crate::widgets::empty_state(
            ui,
            "This organism founded its lineage — no parent to compare.",
        );
        return;
    };

    let mut entity_by_id: std::collections::HashMap<common::EntityId, bevy_ecs::entity::Entity> =
        std::collections::HashMap::new();
    {
        let mut q = world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &genetics::Genome)>();
        for (e, _) in q.iter(&world.ecs) {
            entity_by_id.insert(common::EntityId(e.to_bits()), e);
        }
    }
    let Some(&parent_entity) = entity_by_id.get(&parent_id) else {
        crate::widgets::empty_state(ui, "Parent is no longer alive — no live genome to compare.");
        return;
    };

    let mut genome_q = world.ecs.query::<&genetics::Genome>();
    let Ok(parent_genome) = genome_q.get(&world.ecs, parent_entity) else {
        crate::widgets::empty_state(ui, "Parent is no longer alive — no live genome to compare.");
        return;
    };

    let self_expressed = genome.expressed_regulatory_cppn();
    let parent_expressed = parent_genome.expressed_regulatory_cppn();
    let self_network = developed_network(&self_expressed, position, step);
    let parent_network = developed_network(&parent_expressed, position, step);

    crate::widgets::kv_row(
        ui,
        "Topology",
        &format!(
            "{} nodes / {} edges  (parent: {} nodes / {} edges)",
            self_network.nodes.len(),
            self_network.edges.len(),
            parent_network.nodes.len(),
            parent_network.edges.len()
        ),
    );

    let rows = crate::regulatory_view::bias_diff_rows(&self_network, &parent_network);
    crate::regulatory_view::render_bias_diff_grid(ui, "grn_mutation_comparison", &rows);
}
