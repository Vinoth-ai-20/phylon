//! Cell Lineage Viewer / Development Replay (Phase 4, `PHASE4_ROADMAP.md`
//! milestone P4-R5) — reads the selected organism's `evolution::LineageTracker`
//! record (ancestry, generation, species) alongside its **persistent**
//! `organisms::DevelopmentalGraph` (P4-F1), which is what makes this panel
//! meaningfully different from Phase 3's `organisms::simulate_growth_timeline`
//! replay the HOX/GRN Viewers already use: that replay is a stateless
//! re-decode from the genome alone (no real entities), while this panel
//! shows the organism's actual, currently-live anatomy — the same graph
//! P4-F2 through P4-F5's physiology systems act on every tick.
use crate::types::*;

/// Renders the Cell Lineage Viewer dock panel.
#[allow(clippy::too_many_arguments)]
pub fn lineage_viewer_ui(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
    _actions: &mut Vec<MenuAction>,
) {
    let Some(entity) = state.selected_entity.or(state.tracked_entity) else {
        crate::widgets::empty_state(ui, "Select an organism to inspect its lineage.");
        return;
    };

    ui.label(egui::RichText::new("Ancestry").strong());
    let entity_id = common::EntityId(entity.to_bits());
    let tracker = world.ecs.get_resource::<evolution::LineageTracker>();
    match tracker.and_then(|t| t.get_record(entity_id)) {
        Some(record) => {
            egui::Grid::new("lineage_viewer_ancestry")
                .striped(true)
                .show(ui, |ui| {
                    crate::widgets::kv_row_mono(ui, "Lineage", &record.lineage.0.to_string());
                    crate::widgets::kv_row_mono(ui, "Species", &record.species.0.to_string());
                    crate::widgets::kv_row_mono(ui, "Generation", &record.generation.to_string());
                    crate::widgets::kv_row_mono(ui, "Birth tick", &record.birth_tick.to_string());
                });
        }
        None => {
            crate::widgets::empty_state(
                ui,
                "No lineage record for this organism yet (tracker may not be enabled).",
            );
        }
    }

    ui.add_space(crate::theme::SPACE_SM);

    // Phase 5, SX-3c: a real multi-generation ancestor/descendant tree,
    // replacing the previous "one raw parent id" row — see
    // `evolution::LineageTracker::ancestors`/`children`'s doc comments for
    // why these two directions behave very differently in practice (a
    // parent is usually already dead-and-extracted; a child usually isn't).
    const MAX_ANCESTOR_DEPTH: usize = 5;
    if let Some(tracker) = tracker {
        ui.label(egui::RichText::new("Ancestors").strong());
        let chain = tracker.ancestors(entity_id, MAX_ANCESTOR_DEPTH);
        if chain.is_empty() {
            ui.label(
                egui::RichText::new("— (founder, or parent no longer tracked)")
                    .small()
                    .color(crate::theme::DISABLED_FG),
            );
        } else {
            egui::Grid::new("lineage_viewer_ancestors")
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Hops up").strong());
                    ui.label(egui::RichText::new("Entity").strong());
                    ui.label(egui::RichText::new("Gen").strong());
                    ui.label(egui::RichText::new("Status").strong());
                    ui.end_row();
                    for (hop, ancestor) in chain.iter().enumerate() {
                        ui.monospace((hop + 1).to_string());
                        ui.monospace(ancestor.entity.0.to_string());
                        ui.monospace(ancestor.generation.to_string());
                        let status = if ancestor.death_tick.is_some() {
                            "Deceased"
                        } else {
                            "Alive"
                        };
                        ui.label(status);
                        ui.end_row();
                    }
                });
            if chain.len() == MAX_ANCESTOR_DEPTH {
                ui.label(
                    egui::RichText::new(format!(
                        "Stopped at {MAX_ANCESTOR_DEPTH} generations — further ancestors not fetched."
                    ))
                    .small()
                    .color(crate::theme::DISABLED_FG),
                );
            } else {
                ui.label(
                    egui::RichText::new(
                        "Chain ends here — the next ancestor has already died and been \
                         moved to permanent storage (this tracker only holds the currently \
                         active window, not full history).",
                    )
                    .small()
                    .color(crate::theme::DISABLED_FG),
                );
            }
        }

        ui.add_space(crate::theme::SPACE_SM);

        ui.label(egui::RichText::new("Descendants").strong());
        let children = tracker.children(entity_id);
        if children.is_empty() {
            ui.label(
                egui::RichText::new("No offspring yet.")
                    .small()
                    .color(crate::theme::DISABLED_FG),
            );
        } else {
            egui::Grid::new("lineage_viewer_children")
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Entity").strong());
                    ui.label(egui::RichText::new("Gen").strong());
                    ui.label(egui::RichText::new("Status").strong());
                    ui.label(egui::RichText::new("Grandchildren").strong());
                    ui.end_row();
                    for child in &children {
                        ui.monospace(child.entity.0.to_string());
                        ui.monospace(child.generation.to_string());
                        let status = if child.death_tick.is_some() {
                            "Deceased"
                        } else {
                            "Alive"
                        };
                        ui.label(status);
                        ui.monospace(tracker.children(child.entity).len().to_string());
                        ui.end_row();
                    }
                });
        }
    }

    ui.add_space(crate::theme::SPACE_MD);
    ui.separator();
    ui.add_space(crate::theme::SPACE_SM);

    ui.label(egui::RichText::new("Live Body Graph (P4-F1, persistent)").strong());
    ui.label(
        egui::RichText::new(
            "This organism's actual current anatomy — not a re-decode from its genome. Unlike the HOX/GRN Viewers' replay scrubber, this reflects real physiology/injury state as it stands right now.",
        )
        .small()
        .color(crate::theme::DISABLED_FG),
    );
    ui.add_space(crate::theme::SPACE_SM);

    let mut graph_q = world.ecs.query::<&organisms::DevelopmentalGraph>();
    match graph_q.get(&world.ecs, entity) {
        Ok(graph) => {
            let spine_count = graph.nodes.iter().filter(|n| !n.is_branch).count();
            let branch_count = graph.nodes.iter().filter(|n| n.is_branch).count();
            egui::Grid::new("lineage_viewer_graph_summary")
                .striped(true)
                .show(ui, |ui| {
                    crate::widgets::kv_row_mono(ui, "Total nodes", &graph.nodes.len().to_string());
                    crate::widgets::kv_row_mono(ui, "Spine segments", &spine_count.to_string());
                    crate::widgets::kv_row_mono(ui, "Branch segments", &branch_count.to_string());
                });

            ui.add_space(crate::theme::SPACE_SM);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(200.0)
                .show(ui, |ui| {
                    egui::Grid::new("lineage_viewer_graph_nodes")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Pos").strong());
                            ui.label(egui::RichText::new("Role").strong());
                            ui.label(egui::RichText::new("Parent").strong());
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
                                    .unwrap_or_else(|| "— (root)".to_string());
                                ui.label(parent_pos);
                                ui.end_row();
                            }
                        });
                });
        }
        Err(_) => {
            crate::widgets::empty_state(ui, "This entity has no persistent Body Graph.");
        }
    }
}
