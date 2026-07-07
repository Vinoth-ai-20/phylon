//! Immune Viewer (Phase 4, `PHASE4_ROADMAP.md` milestone P4-R4) — shows the
//! organism-wide `ecology::disease::Infection` state alongside every
//! segment's `SegmentInfection`/`SegmentImmunity` (P4-F5), so a researcher
//! can see how far an infection has actually spread through the body, not
//! just whether the organism as a whole is infected.
use crate::types::*;

/// Renders the Immune Viewer dock panel.
#[allow(clippy::too_many_arguments)]
pub fn immune_viewer_ui(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
    _actions: &mut Vec<MenuAction>,
) {
    let Some((entity, graph)) =
        crate::plugins::organism_panel_common::resolve_target_and_graph(state, world)
    else {
        crate::widgets::empty_state(ui, "Select an organism to inspect its immune state.");
        return;
    };

    if graph.nodes.is_empty() {
        crate::widgets::empty_state(ui, "This organism's Body Graph is empty.");
        return;
    }

    let mut infection_q = world.ecs.query::<&ecology::disease::Infection>();
    match infection_q.get(&world.ecs, entity) {
        Ok(infection) => {
            ui.label(egui::RichText::new("Organism-Wide Infection").strong());
            egui::Grid::new("immune_viewer_head")
                .striped(true)
                .show(ui, |ui| {
                    crate::widgets::kv_row(ui, "State", &format!("{:?}", infection.state));
                    crate::widgets::kv_row_mono(
                        ui,
                        "Virulence",
                        &format!("{:.2}", infection.virulence),
                    );
                    crate::widgets::kv_row_mono(
                        ui,
                        "Transmissibility",
                        &format!("{:.2}", infection.transmissibility),
                    );
                });
        }
        Err(_) => {
            ui.label(
                egui::RichText::new("Not currently infected.")
                    .color(crate::theme::GOOD)
                    .italics(),
            );
        }
    }
    ui.add_space(crate::theme::SPACE_SM);

    ui.label(egui::RichText::new("Per-Segment Severity/Resistance (P4-F5)").strong());
    crate::plugins::organism_panel_common::viewport_overlay_toggle(
        ui,
        state,
        crate::types::PhysiologyOverlayLayer::Immune,
    );
    ui.label(
        egui::RichText::new(
            "Severity spreads outward from the head's Infection (if Infectious); each segment's own resistance clears it over time.",
        )
        .small()
        .color(crate::theme::DISABLED_FG),
    );
    ui.add_space(crate::theme::SPACE_SM);

    let mut infection_severity_q = world.ecs.query::<&ecology::disease::SegmentInfection>();
    let mut immunity_q = world.ecs.query::<&ecology::disease::SegmentImmunity>();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("immune_viewer_table")
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Pos").strong());
                    ui.label(egui::RichText::new("Role").strong());
                    ui.label(egui::RichText::new("Severity").strong());
                    ui.label(egui::RichText::new("Resistance").strong());
                    ui.end_row();

                    for node in &graph.nodes {
                        crate::plugins::organism_panel_common::segment_identity_cells(
                            ui,
                            node.position,
                            node.role,
                            node.is_branch,
                        );
                        let severity = node
                            .entity
                            .and_then(|e| infection_severity_q.get(&world.ecs, e).ok())
                            .map(|s| format!("{:.2}", s.severity));
                        let resistance = node
                            .entity
                            .and_then(|e| immunity_q.get(&world.ecs, e).ok())
                            .map(|r| format!("{:.2}", r.resistance));

                        match severity {
                            Some(s) => {
                                ui.monospace(s);
                            }
                            None => {
                                ui.label(
                                    egui::RichText::new("head").color(crate::theme::DISABLED_FG),
                                );
                            }
                        }
                        match resistance {
                            Some(r) => {
                                ui.monospace(r);
                            }
                            None => {
                                ui.label("");
                            }
                        }
                        ui.end_row();
                    }
                });
        });
}
