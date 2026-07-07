//! Hormone Viewer (Phase 4, `PHASE4_ROADMAP.md` milestone P4-R3) — shows the
//! head's organism-wide `brain::Neuromodulators` reading alongside every
//! other segment's `brain::HormoneLevel` (P4-F4), so a researcher can see
//! the endocrine signal's actual spread across the body, not just the
//! single scalar that existed before P4-F4.
use crate::types::*;

/// Renders the Hormone Viewer dock panel.
#[allow(clippy::too_many_arguments)]
pub fn hormone_viewer_ui(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
    _actions: &mut Vec<MenuAction>,
) {
    let Some((entity, graph)) =
        crate::plugins::organism_panel_common::resolve_target_and_graph(state, world)
    else {
        crate::widgets::empty_state(ui, "Select an organism to inspect its hormone state.");
        return;
    };

    if graph.nodes.is_empty() {
        crate::widgets::empty_state(ui, "This organism's Body Graph is empty.");
        return;
    }

    let mut neuro_q = world.ecs.query::<&brain::Neuromodulators>();
    if let Ok(neuro) = neuro_q.get(&world.ecs, entity) {
        ui.label(egui::RichText::new("Head — Neuromodulators (source)").strong());
        egui::Grid::new("hormone_viewer_head")
            .striped(true)
            .show(ui, |ui| {
                crate::widgets::kv_row_mono(ui, "Dopamine", &format!("{:.2}", neuro.dopamine));
                crate::widgets::kv_row_mono(ui, "Serotonin", &format!("{:.2}", neuro.serotonin));
                crate::widgets::kv_row_mono(
                    ui,
                    "Noradrenaline",
                    &format!("{:.2}", neuro.noradrenaline),
                );
            });
        ui.add_space(crate::theme::SPACE_SM);
    }

    ui.label(egui::RichText::new("Per-Segment Hormone Level (P4-F4)").strong());
    crate::plugins::organism_panel_common::viewport_overlay_toggle(
        ui,
        state,
        crate::types::PhysiologyOverlayLayer::Hormone,
    );
    ui.label(
        egui::RichText::new(
            "Each segment relaxes toward its parent's reading every tick — the head's own Neuromodulators above is the source, unaffected by this spread.",
        )
        .small()
        .color(crate::theme::DISABLED_FG),
    );
    ui.add_space(crate::theme::SPACE_SM);

    let mut level_q = world.ecs.query::<&brain::HormoneLevel>();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("hormone_viewer_table")
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Pos").strong());
                    ui.label(egui::RichText::new("Role").strong());
                    ui.label(egui::RichText::new("Dopamine").strong());
                    ui.label(egui::RichText::new("Serotonin").strong());
                    ui.label(egui::RichText::new("Noradrenaline").strong());
                    ui.end_row();

                    for node in &graph.nodes {
                        crate::plugins::organism_panel_common::segment_identity_cells(
                            ui,
                            node.position,
                            node.role,
                            node.is_branch,
                        );
                        match node.entity.and_then(|e| level_q.get(&world.ecs, e).ok()) {
                            Some(level) => {
                                ui.monospace(format!("{:.2}", level.dopamine));
                                ui.monospace(format!("{:.2}", level.serotonin));
                                ui.monospace(format!("{:.2}", level.noradrenaline));
                            }
                            None => {
                                ui.label(
                                    egui::RichText::new("head (see above)")
                                        .color(crate::theme::DISABLED_FG),
                                );
                                ui.label("");
                                ui.label("");
                            }
                        }
                        ui.end_row();
                    }
                });
        });
}
