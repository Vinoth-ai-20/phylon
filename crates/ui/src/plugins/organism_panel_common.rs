//! Shared helper for every Phase 4 P4-R-tier physiology panel (Physiology,
//! Circulation, Hormone, and Immune Viewers) — each is fundamentally "take
//! the selected/tracked organism, walk its persistent Body Graph, show one
//! specific component per segment," differing only in *which* component.
//! This module owns the first half (resolving the organism + its graph);
//! each panel owns the second half (its own per-segment table).

/// Resolves the currently selected-or-tracked organism's entity and a clone
/// of its persistent `organisms::DevelopmentalGraph` (Phase 4, ADR-P4-01) —
/// `None` if nothing is selected/tracked, or the entity has no Body Graph
/// (e.g. it isn't a head node). Callers render their own "nothing to show"
/// message on `None`, since the right hint text differs per panel.
pub fn resolve_target_and_graph(
    state: &crate::WorkbenchState,
    world: &mut world::World,
) -> Option<(bevy_ecs::entity::Entity, organisms::DevelopmentalGraph)> {
    let entity = state.selected_entity.or(state.tracked_entity)?;
    let mut graph_q = world.ecs.query::<&organisms::DevelopmentalGraph>();
    let graph = graph_q.get(&world.ecs, entity).ok()?.clone();
    Some((entity, graph))
}

/// A body position's identity columns every per-segment table in this
/// panel family starts with — position index, role, and whether it's a
/// lateral branch (fin) rather than a spine segment.
pub fn segment_identity_cells(
    ui: &mut egui::Ui,
    position: usize,
    role: genetics::SegmentType,
    is_branch: bool,
) {
    ui.label(position.to_string());
    ui.label(format!(
        "{role:?}{}",
        if is_branch { " (branch)" } else { "" }
    ));
}
