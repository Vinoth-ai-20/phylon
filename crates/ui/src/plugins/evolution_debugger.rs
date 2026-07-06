//! Evolution Debugger panel (Phase 3, M12) — cross-organism mutation diff,
//! parent-vs-offspring and arbitrary-pair comparison, and a
//! development-failure inspector, following `PHASE3_ROADMAP.md` §8's design.
//!
//! Unlike the HOX Visualizer / GRN Viewer (Sidebar tabs, scoped to the
//! single selected/tracked organism), this is a dock panel per §8's own
//! spec ("cross-organism/cross-run, like Research Dashboard") — it compares
//! *any two* organisms, not just "selected vs. its parent."
//!
//! Reuses `crate::regulatory_view`'s network-building/bias-diff helpers
//! (built for the GRN Viewer, M11) rather than duplicating that logic —
//! "mutation diff" here is the same per-gene bias comparison, just for an
//! arbitrary pair instead of a fixed parent link.
//!
//! **Deliberately out of scope, documented not silently dropped:** a
//! "development event log" (§8's fifth bullet). No event-emission
//! infrastructure exists today for development events (`growth_system`
//! doesn't publish to `events::PhylonEvent` at all) — building one would be
//! a materially separate feature (new event variants, emission wiring in
//! `organisms::growth_system`, a log display), not an additive reuse of
//! data this phase's pipeline already produces. Flagged for a future
//! milestone rather than built here.

use crate::types::MenuAction;
use bevy_ecs::entity::Entity;

/// Renders the Evolution Debugger panel content.
pub fn evolution_debugger_ui(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
    _actions: &mut Vec<MenuAction>,
) {
    let entity_a = match state.selected_entity.or(state.tracked_entity) {
        Some(e) => e,
        None => {
            crate::widgets::empty_state(
                ui,
                "Select an organism (Organism A) to compare against another.",
            );
            development_failure_inspector(ui, state, world);
            return;
        }
    };

    ui.label(egui::RichText::new("Mutation Diff").strong());
    comparison_section(ui, state, world, entity_a);

    ui.add_space(crate::theme::SPACE_MD);
    ui.separator();
    development_failure_inspector(ui, state, world);
}

fn comparison_section(
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
    entity_a: Entity,
) {
    let genome_a = {
        let mut q = world.ecs.query::<&genetics::Genome>();
        let Ok(g) = q.get(&world.ecs, entity_a) else {
            crate::widgets::empty_state(ui, "Organism A has no Genome. Select the head node.");
            return;
        };
        g.clone()
    };

    // Resolve Organism B: an explicit pick, or Organism A's lineage parent.
    let mut entity_by_id: std::collections::HashMap<common::EntityId, Entity> =
        std::collections::HashMap::new();
    {
        let mut q = world.ecs.query::<(Entity, &genetics::Genome)>();
        for (e, _) in q.iter(&world.ecs) {
            entity_by_id.insert(common::EntityId(e.to_bits()), e);
        }
    }

    let auto_parent = world
        .ecs
        .get_resource::<evolution::LineageTracker>()
        .and_then(|tracker| tracker.get_record(common::EntityId(entity_a.to_bits())))
        .and_then(|record| record.parent_id)
        .and_then(|parent_id| entity_by_id.get(&parent_id).copied());

    ui.horizontal(|ui| {
        ui.label(format!("Organism A: {}", entity_a.index()));
        ui.label(
            egui::RichText::new(match state.evo_debugger_entity_b {
                Some(b) => format!("Organism B: {} (picked)", b.index()),
                None => match auto_parent {
                    Some(p) => format!("Organism B: {} (parent)", p.index()),
                    None => "Organism B: none available".to_string(),
                },
            })
            .color(crate::theme::DISABLED_FG),
        );
        if state.evo_debugger_entity_b.is_some() && ui.small_button("Use parent instead").clicked()
        {
            state.evo_debugger_entity_b = None;
        }
    });

    organism_picker(ui, state, &entity_by_id, entity_a);

    let entity_b = state.evo_debugger_entity_b.or(auto_parent);
    let Some(entity_b) = entity_b else {
        crate::widgets::empty_state(
            ui,
            "No Organism B available — this organism founded its lineage, or pick one below.",
        );
        return;
    };

    let genome_b = {
        let mut q = world.ecs.query::<&genetics::Genome>();
        let Ok(g) = q.get(&world.ecs, entity_b) else {
            crate::widgets::empty_state(ui, "Organism B is no longer alive.");
            return;
        };
        g.clone()
    };

    let expressed_a = genome_a.expressed_regulatory_cppn();
    let expressed_b = genome_b.expressed_regulatory_cppn();
    // A fixed reference position/step (head, fully developed) — this panel
    // compares genomes at one representative point rather than duplicating
    // GRN Viewer's full position/step scrubbing.
    let position = 0;
    let step = genetics::develop::DEVELOPMENT_STEPS;
    let network_a = crate::regulatory_view::developed_network(&expressed_a, position, step);
    let network_b = crate::regulatory_view::developed_network(&expressed_b, position, step);

    crate::widgets::kv_row(
        ui,
        "Topology",
        &format!(
            "A: {} nodes / {} edges — B: {} nodes / {} edges",
            network_a.nodes.len(),
            network_a.edges.len(),
            network_b.nodes.len(),
            network_b.edges.len()
        ),
    );
    crate::widgets::kv_row(ui, "Segment sequence (A)", &segment_sequence(&expressed_a));
    crate::widgets::kv_row(ui, "Segment sequence (B)", &segment_sequence(&expressed_b));

    let rows = crate::regulatory_view::bias_diff_rows(&network_a, &network_b);
    crate::regulatory_view::render_bias_diff_grid(ui, "evo_debugger_bias_diff", &rows);
}

/// A compact one-letter-per-segment string (e.g. "HMMMTX") for the whole
/// body plan — reuses the same decode `hox_visualizer` shows in full detail,
/// condensed here for quick side-by-side comparison.
fn segment_sequence(regulatory_cppn: &genetics::Cppn) -> String {
    let total = organisms::MAX_SEGMENTS;
    (0..total)
        .map(|i| {
            let outputs = genetics::develop_at_position(regulatory_cppn, i, total);
            match outputs.segment_type {
                genetics::SegmentType::Head => 'H',
                genetics::SegmentType::Torso => 'T',
                genetics::SegmentType::Muscle => 'M',
                genetics::SegmentType::Tail => 'X',
                genetics::SegmentType::Fin => 'F',
                genetics::SegmentType::Vascular => 'V',
                genetics::SegmentType::Ganglion => 'G',
                genetics::SegmentType::Germinal => 'Z',
            }
        })
        .collect()
}

/// A filterable, capped list of live organisms to pick as Organism B.
fn organism_picker(
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    entity_by_id: &std::collections::HashMap<common::EntityId, Entity>,
    entity_a: Entity,
) {
    ui.collapsing("Pick Organism B", |ui| {
        ui.text_edit_singleline(&mut state.evo_debugger_search);
        egui::ScrollArea::vertical()
            .max_height(120.0)
            .show(ui, |ui| {
                let mut shown = 0;
                for &entity in entity_by_id.values() {
                    if entity == entity_a {
                        continue;
                    }
                    let label = entity.index().to_string();
                    if !state.evo_debugger_search.is_empty()
                        && !label.contains(state.evo_debugger_search.as_str())
                    {
                        continue;
                    }
                    if ui
                        .selectable_label(false, format!("Entity {label}"))
                        .clicked()
                    {
                        state.evo_debugger_entity_b = Some(entity);
                    }
                    shown += 1;
                    if shown >= 50 {
                        ui.label(
                            egui::RichText::new("(showing first 50 matches)")
                                .small()
                                .color(crate::theme::DISABLED_FG),
                        );
                        break;
                    }
                }
            });
    });
}

/// Lists live organisms that finished growing (have a `brain::Brain`) but
/// produced zero actuated effectors — a genuine developmental failure
/// (they can never move), cheap to detect from data the pipeline already
/// produces.
fn development_failure_inspector(
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
) {
    ui.label(egui::RichText::new("Development Failures").strong());
    ui.label(
        egui::RichText::new("Organisms that finished growing with zero actuated effectors.")
            .small()
            .color(crate::theme::DISABLED_FG),
    );

    let mut failures: Vec<Entity> = Vec::new();
    {
        let mut q = world
            .ecs
            .query::<(Entity, &brain::Brain, &behavior::MotorSystem)>();
        for (entity, _, motor) in q.iter(&world.ecs) {
            if motor.effectors.is_empty() {
                failures.push(entity);
            }
        }
    }

    if failures.is_empty() {
        crate::widgets::empty_state(ui, "No development failures in the current population.");
        return;
    }

    egui::ScrollArea::vertical()
        .max_height(120.0)
        .show(ui, |ui| {
            for entity in failures.into_iter().take(50) {
                if ui
                    .selectable_label(false, format!("Entity {} — 0 effectors", entity.index()))
                    .clicked()
                {
                    state.selected_entity = Some(entity);
                }
            }
        });
}
