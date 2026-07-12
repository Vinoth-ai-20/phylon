//! Physiology Viewer + Organ Inspector — the UI surface for per-segment
//! `metabolism::ChemicalEconomy` pools, which otherwise exist only in ECS
//! memory with no way for a researcher to see them.
//!
//! One table, one row per Body Graph position (walked via the persistent
//! `organisms::DevelopmentalGraph`) — this doubles as an "Organ Inspector":
//! each row *is* one organ/segment's full resource inspection, rather than
//! a separate drill-down panel, since the whole table already fits the
//! same screen.
use crate::types::*;

/// Renders the Physiology Viewer / Organ Inspector dock panel.
#[allow(clippy::too_many_arguments)]
pub fn physiology_viewer_ui(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
    _actions: &mut Vec<MenuAction>,
) {
    let Some((_entity, graph)) =
        crate::plugins::organism_panel_common::resolve_target_and_graph(state, world)
    else {
        crate::widgets::empty_state(
            ui,
            "Select an organism to inspect its per-segment physiology.",
        );
        return;
    };

    if graph.nodes.is_empty() {
        crate::widgets::empty_state(ui, "This organism's Body Graph is empty.");
        return;
    }

    ui.label(egui::RichText::new("Per-Segment Chemical Economy").strong());
    ui.label(
        egui::RichText::new(
            "One row per Body Graph position — the head's pool is organism-scale; every other segment's is the small P4-F2 pool.",
        )
        .small()
        .color(crate::theme::DISABLED_FG),
    );
    ui.add_space(crate::theme::SPACE_SM);

    let mut chem_q = world.ecs.query::<&metabolism::ChemicalEconomy>();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("physiology_viewer_table")
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Pos").strong());
                    ui.label(egui::RichText::new("Role").strong());
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
                        match node.entity.and_then(|e| chem_q.get(&world.ecs, e).ok()) {
                            Some(chem) => {
                                ui.monospace(format!(
                                    "{:.0}/{:.0}",
                                    chem.glucose, chem.max_glucose
                                ));
                                ui.monospace(format!("{:.0}/{:.0}", chem.o2, chem.max_o2));
                                ui.monospace(format!("{:.0}/{:.0}", chem.co2, chem.max_co2));
                                ui.monospace(format!("{:.0}/{:.0}", chem.atp, chem.max_atp));
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
