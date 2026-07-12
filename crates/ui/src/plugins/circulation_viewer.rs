//! Circulation Viewer — shows the same per-segment
//! `metabolism::ChemicalEconomy` pools the Physiology Viewer does, framed
//! around what `organisms::transport_system` actually moves between them
//! (glucose/o2/atp/co2), with the Body Graph's parent/child structure shown
//! alongside each row so a researcher can read the table as "what's flowing
//! along this edge," not just a flat list.
//!
//! **Disclosed scope limitation:** `organisms::transport::TRANSPORT_RATE`
//! and `relax_toward_equilibrium` are private to that module (deliberately —
//! see `transport.rs`'s own doc comment) — this panel shows current
//! per-segment *levels*, not a live per-tick *flow rate*, since exposing the
//! latter would need new `pub` API in `organisms::transport` this panel
//! doesn't otherwise need. Flow-rate visualization could be added later if
//! it turns out to matter.
use crate::types::*;

/// Renders the Circulation Viewer dock panel.
#[allow(clippy::too_many_arguments)]
pub fn circulation_viewer_ui(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
    _actions: &mut Vec<MenuAction>,
) {
    let Some((_entity, graph)) =
        crate::plugins::organism_panel_common::resolve_target_and_graph(state, world)
    else {
        crate::widgets::empty_state(ui, "Select an organism to inspect its circulation.");
        return;
    };

    if graph.nodes.is_empty() {
        crate::widgets::empty_state(ui, "This organism's Body Graph is empty.");
        return;
    }

    ui.label(egui::RichText::new("Intra-Body Transport (P4-F3)").strong());
    ui.label(
        egui::RichText::new(
            "Current levels per segment, along the same Body Graph edges organisms::transport_system relaxes each tick. Shows levels, not live flow rate — see this panel's module doc comment.",
        )
        .small()
        .color(crate::theme::DISABLED_FG),
    );
    crate::plugins::organism_panel_common::viewport_overlay_toggle(
        ui,
        state,
        crate::types::PhysiologyOverlayLayer::Circulation,
    );
    ui.add_space(crate::theme::SPACE_SM);

    let mut chem_q = world.ecs.query::<&metabolism::ChemicalEconomy>();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("circulation_viewer_table")
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Pos").strong());
                    ui.label(egui::RichText::new("Role").strong());
                    ui.label(egui::RichText::new("Parent Pos").strong());
                    ui.label(egui::RichText::new("Glucose").strong());
                    ui.label(egui::RichText::new("O2").strong());
                    ui.label(egui::RichText::new("CO2").strong());
                    ui.label(egui::RichText::new("ATP").strong());
                    ui.end_row();

                    for node in &graph.nodes {
                        crate::plugins::organism_panel_common::segment_identity_cells(
                            ui,
                            node.position,
                            node.role,
                            node.is_branch,
                        );
                        let parent_pos = node
                            .parent
                            .and_then(|p| graph.nodes.get(p))
                            .map(|p| p.position.to_string())
                            .unwrap_or_else(|| "—".to_string());
                        ui.label(parent_pos);

                        match node.entity.and_then(|e| chem_q.get(&world.ecs, e).ok()) {
                            Some(chem) => {
                                ui.monospace(format!("{:.0}", chem.glucose));
                                ui.monospace(format!("{:.0}", chem.o2));
                                ui.monospace(format!("{:.0}", chem.co2));
                                ui.monospace(format!("{:.0}", chem.atp));
                            }
                            None => {
                                ui.label(egui::RichText::new("—").color(crate::theme::DISABLED_FG));
                                ui.label("");
                                ui.label("");
                                ui.label("");
                            }
                        }
                        ui.end_row();
                    }
                });
        });
}
