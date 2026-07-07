//! HOX Visualizer panel (Phase 3, M10) — per-position Hox combinatorial
//! code, decoded segment identity, and morphogen gradients for the
//! selected organism, following `PHASE3_ROADMAP.md` §8's design.
//!
//! At the time this panel was built (Phase 3, M10), `organisms::GrowthState`'s
//! Body Graph was transient (ADR-P3-04) and gone by the time an organism was
//! an adult — so this panel deliberately re-runs the same deterministic,
//! side-effect-free `genetics::develop_at_position` pipeline directly from
//! the organism's `Genome` component, for every position, purely for
//! display, rather than reading any growth-time graph state.
//!
//! **Correction (Phase 4, milestone P4-F1):** `organisms::DevelopmentalGraph`
//! is now a persistent sibling component (`PHASE4_ROADMAP.md`'s ADR-P4-01)
//! that *does* survive adulthood. This panel's logic is intentionally left
//! unchanged here — P4-F1 is infrastructure-only, and switching this panel
//! to read the persisted graph instead of recomputing is a visualization
//! change out of that milestone's scope. The recompute-on-demand approach
//! also still has a real advantage the persisted graph doesn't replace: it
//! works identically for an organism still mid-growth, not just a finished
//! adult. Revisit this panel's data source only as part of whichever future
//! milestone actually needs the persisted graph's runtime-history data
//! (e.g. reflecting injury) that a fresh decode can no longer reconstruct.
//!
//! Morphogen Visualization (§8's second panel) is folded into this same tab
//! as a heatmap strip beneath the body preview, per its own spec: "Overlay
//! on the HOX Visualizer/body preview, not a separate dock panel."
//!
//! Deliberately out of scope for this milestone (documented, not silently
//! dropped): "produced organs" per-segment and deep cross-linking to
//! Lineage Explorer's mutation history (a text pointer to the Lineage tab
//! is provided instead of duplicating its data model).

use bevy_ecs::entity::Entity;

/// Renders the HOX Visualizer tab for `state.selected_entity.or(state.tracked_entity)`.
pub fn hox_visualizer_ui(
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
) {
    let entity = match state.selected_entity.or(state.tracked_entity) {
        Some(e) => e,
        None => {
            crate::widgets::empty_state(ui, "Select an organism to inspect its Hox decode.");
            return;
        }
    };

    let mut genome_q = world.ecs.query::<&genetics::Genome>();
    let Ok(genome) = genome_q.get(&world.ecs, entity) else {
        crate::widgets::empty_state(ui, "Genome not on this node. Select the head node.");
        return;
    };

    let expressed_regulatory_cppn = genome.expressed_regulatory_cppn();
    let total = organisms::MAX_SEGMENTS;

    let grown_positions = crate::timeline::grown_positions(&expressed_regulatory_cppn);
    if let Some(position) =
        crate::timeline::timeline_scrubber_ui(ui, &grown_positions, &mut state.timeline_step)
    {
        if ui.small_button("Show this position's details").clicked() {
            state.hox_visualizer_selected_index = Some(position);
        }
    }
    ui.add_space(crate::theme::SPACE_SM);

    ui.label(egui::RichText::new("Body Plan Decode").strong());
    ui.label(
        egui::RichText::new(
            "Each swatch is one body position, decoded fresh from the genome — click one for details.",
        )
        .small()
        .color(crate::theme::DISABLED_FG),
    );
    ui.add_space(crate::theme::SPACE_SM);

    body_preview_strip(ui, state, &expressed_regulatory_cppn, entity, total);

    ui.add_space(crate::theme::SPACE_MD);
    ui.label(egui::RichText::new("Morphogen Gradients").strong());
    ui.label(
        egui::RichText::new(
            "AP position (top) and distance-from-head decay (bottom), per position.",
        )
        .small()
        .color(crate::theme::DISABLED_FG),
    );
    ui.add_space(crate::theme::SPACE_XS);
    morphogen_heatmap_strip(ui, total);

    ui.add_space(crate::theme::SPACE_MD);
    ui.separator();

    match state.hox_visualizer_selected_index {
        Some(index) if index < total => {
            detail_panel(ui, &expressed_regulatory_cppn, index, total);
        }
        _ => {
            crate::widgets::empty_state(ui, "Click a position above to see its decoded details.");
        }
    }
}

fn body_preview_strip(
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    regulatory_cppn: &genetics::Cppn,
    _entity: Entity,
    total: usize,
) {
    ui.horizontal_wrapped(|ui| {
        for index in 0..total {
            let outputs = genetics::develop_at_position(regulatory_cppn, index, total);
            let color = pigment_to_color32(outputs.pigment);
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());

            ui.painter().rect_filled(rect, 2.0, color);
            let is_selected = state.hox_visualizer_selected_index == Some(index);
            let stroke_color = if is_selected {
                crate::theme::FOCUS_RING
            } else {
                crate::theme::DISABLED_FG
            };
            ui.painter()
                .rect_stroke(rect, 2.0, egui::Stroke::new(1.5, stroke_color));

            response.clone().on_hover_text(format!(
                "Position {index}: {:?}{}",
                outputs.segment_type,
                if outputs.apoptosis { " (pruned)" } else { "" }
            ));
            if response.clicked() {
                state.hox_visualizer_selected_index = Some(index);
            }
        }
    });
}

fn morphogen_heatmap_strip(ui: &mut egui::Ui, total: usize) {
    ui.horizontal_wrapped(|ui| {
        for index in 0..total {
            let value = genetics::ap_position(index, total).clamp(0.0, 1.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 8.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 1.0, grayscale(value));
        }
    });
    ui.horizontal_wrapped(|ui| {
        for index in 0..total {
            let value = genetics::distance_from_head_gradient(index, total).clamp(0.0, 1.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 8.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 1.0, grayscale(value));
        }
    });
}

fn detail_panel(ui: &mut egui::Ui, regulatory_cppn: &genetics::Cppn, index: usize, total: usize) {
    let outputs = genetics::develop_at_position(regulatory_cppn, index, total);
    let hox_states = genetics::hox_states_at_position(regulatory_cppn, index, total);
    let hox_code: String = hox_states
        .iter()
        .map(|&s| if s > 0.5 { '1' } else { '0' })
        .collect();
    let hox_raw: String = hox_states
        .iter()
        .map(|s| format!("{s:.2}"))
        .collect::<Vec<_>>()
        .join(", ");

    egui::Grid::new("hox_visualizer_detail")
        .striped(true)
        .show(ui, |ui| {
            crate::widgets::kv_row(ui, "Position", &index.to_string());
            crate::widgets::kv_row(ui, "Segment Type", &format!("{:?}", outputs.segment_type));
            crate::widgets::kv_row(ui, "Hox code", &hox_code);
            crate::widgets::kv_row(ui, "Hox raw states", &hox_raw);
            crate::widgets::kv_row(ui, "Branches", &outputs.branches.to_string());
            crate::widgets::kv_row(ui, "Apoptosis", &outputs.apoptosis.to_string());
            crate::widgets::kv_row(
                ui,
                "Actuation amplitude",
                &format!("{:.2}", outputs.actuation_amplitude),
            );
            crate::widgets::kv_row(
                ui,
                "Actuation phase",
                &format!("{:.2}", outputs.actuation_phase),
            );
            crate::widgets::kv_row(
                ui,
                "AP position",
                &format!("{:.2}", genetics::ap_position(index, total)),
            );
            crate::widgets::kv_row(
                ui,
                "Distance-from-head gradient",
                &format!("{:.2}", genetics::distance_from_head_gradient(index, total)),
            );
        });

    ui.add_space(crate::theme::SPACE_SM);
    ui.label(
        egui::RichText::new(format!(
            "{} For ancestry and mutation history, see the Lineage tab.",
            egui_remixicon::icons::TREE_LINE
        ))
        .color(crate::theme::DISABLED_FG)
        .italics(),
    );
}

fn pigment_to_color32(pigment: [f32; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(
        (pigment[0].clamp(0.0, 1.0) * 255.0) as u8,
        (pigment[1].clamp(0.0, 1.0) * 255.0) as u8,
        (pigment[2].clamp(0.0, 1.0) * 255.0) as u8,
    )
}

fn grayscale(value: f32) -> egui::Color32 {
    let v = (value * 255.0) as u8;
    egui::Color32::from_rgb(v, v, v)
}
