//! Per-segment immune response — extends `ecology::disease`'s organism-wide
//! [`ecology::disease::Infection`] with a spatial dimension, using the same
//! Body Graph edge-walking approach `transport::transport_system`
//! established.
//!
//! **Model:** the head's own [`ecology::disease::Infection`] (if
//! `Infectious`) is the infection's source severity; each tick, that
//! severity relaxes outward into every segment's own
//! [`ecology::disease::SegmentInfection`] one-directionally — the same
//! broadcast-not-conserved shape the `endocrine` module uses, since "how
//! infected is this tissue" isn't a physical quantity moved from the source
//! the way glucose is. Each segment's own [`ecology::disease::SegmentImmunity`]
//! then clears a fixed fraction of its severity every tick, independent of
//! the organism-wide recovery roll `ecology::disease_progression_system`
//! already performs. A segment whose severity is nonzero drains a small
//! amount of its own `metabolism::ChemicalEconomy.atp` — reusing the
//! per-segment physiology pools every grown segment already carries to give
//! infection a real, observable consequence rather than inert tracking
//! state.
//!
//! **Scope note:** a true diffused concentration-field disease model is a
//! possible future extension, not implemented here — this module is
//! intra-body spread of an existing proximity-transmitted infection, not a
//! replacement for `disease_spread_system`'s inter-organism model.

use crate::developmental_graph::DevelopmentalGraph;
use bevy_ecs::prelude::{Entity, Query};
use ecology::disease::{Infection, InfectionState, SegmentImmunity, SegmentInfection};
use metabolism::ChemicalEconomy;
use std::collections::HashMap;

/// Fraction of the gap to a segment's upstream parent's severity closed per
/// tick — untuned placeholder, same status as `transport::TRANSPORT_RATE`
/// and `endocrine::ENDOCRINE_RATE`.
const SPREAD_RATE: f32 = 0.15;

/// ATP drained per tick from a segment at maximal (`1.0`) severity, scaled
/// linearly down to `0.0` at zero severity — a placeholder cost, same
/// status as `SegmentImmunity::baseline()`'s resistance value.
const MAX_ATP_DRAIN_PER_TICK: f32 = 2.0;

/// # Segment Infection System
///
/// ## 1. What Happens
/// Spreads the organism-wide [`Infection`]'s severity out along the Body
/// Graph into every segment's own [`SegmentInfection`], applies each
/// segment's [`SegmentImmunity`] as local clearance, and drains ATP from
/// segments carrying nonzero severity.
///
/// ## 2. Why It Happens
/// Before this milestone, disease was a single pass/fail organism-wide
/// state with no spatial texture — every segment was equally "infected" in
/// the sense that none of them were affected at all beyond the head's own
/// ATP/health drain. This gives infection an actual footprint across the
/// anatomy the Body Graph already models.
///
/// ## 3. How It Happens
/// Structurally identical to `transport::transport_system`/
/// `endocrine::endocrine_diffusion_system`: edges collected from the Body
/// Graph in deterministic parent-before-child order, relaxed against a
/// snapshot, applied back. The source's own severity (the head's actual
/// `Infection`, if `Infectious`) is read directly and never written by this
/// system — `ecology::disease_progression_system` remains the sole owner of
/// organism-wide infection state.
pub fn segment_infection_system(
    graphs: Query<&DevelopmentalGraph>,
    infections: Query<&Infection>,
    mut segment_infections: Query<(Entity, &mut SegmentInfection, Option<&SegmentImmunity>)>,
    mut chem: Query<&mut ChemicalEconomy>,
) {
    // 1. Collect edges, in deterministic (per-graph, parent-before-child)
    // order — identical shape to `transport`/`endocrine`'s first pass.
    let mut edges: Vec<(Entity, Entity)> = Vec::new();
    for graph in graphs.iter() {
        for node in &graph.nodes {
            let (Some(parent_index), Some(child_entity)) = (node.parent, node.entity) else {
                continue;
            };
            let Some(parent_entity) = graph.nodes[parent_index].entity else {
                continue;
            };
            edges.push((parent_entity, child_entity));
        }
    }

    if edges.is_empty() {
        return;
    }

    // 2. Snapshot every segment's current severity.
    let mut snapshot: HashMap<Entity, f32> = segment_infections
        .iter()
        .map(|(entity, infection, _)| (entity, infection.severity))
        .collect();

    // 3. Relax each edge, in order. The parent side is read from `Infection`
    // (head, constant — `0.0` unless currently `Infectious`) or the running
    // snapshot (an upstream segment); only the child side is ever written.
    for (parent, child) in edges {
        let parent_severity = if let Ok(infection) = infections.get(parent) {
            match infection.state {
                InfectionState::Infectious => (infection.virulence / 10.0).clamp(0.0, 1.0),
                _ => 0.0,
            }
        } else if let Some(&severity) = snapshot.get(&parent) {
            severity
        } else {
            continue;
        };

        if let Some(child_severity) = snapshot.get_mut(&child) {
            *child_severity += (parent_severity - *child_severity) * SPREAD_RATE;
        }
    }

    // 4. Apply local immune clearance, then write severity back and drain
    // ATP proportional to the final severity.
    for (entity, mut infection, immunity) in segment_infections.iter_mut() {
        let Some(&spread_severity) = snapshot.get(&entity) else {
            continue;
        };
        let resistance = immunity.map_or(0.0, |i| i.resistance);
        let cleared = (spread_severity - resistance).max(0.0);
        infection.severity = cleared;

        if cleared > 0.0 {
            if let Ok(mut economy) = chem.get_mut(entity) {
                economy.atp = (economy.atp - cleared * MAX_ATP_DRAIN_PER_TICK).max(0.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::developmental_graph::DevelopmentalNode;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;
    use genetics::{DevelopmentalOutputs, SegmentType};

    fn sample_outputs(segment_type: SegmentType) -> DevelopmentalOutputs {
        DevelopmentalOutputs {
            segment_type,
            branches: false,
            actuation_amplitude: 0.0,
            actuation_phase: 0.0,
            pigment: [0.5, 0.5, 0.5],
            apoptosis: false,
        }
    }

    fn full_economy() -> ChemicalEconomy {
        ChemicalEconomy {
            glucose: 100.0,
            o2: 100.0,
            co2: 0.0,
            atp: 100.0,
            max_glucose: 100.0,
            max_o2: 100.0,
            max_co2: 100.0,
            max_atp: 100.0,
        }
    }

    fn build_graph(head: Entity, segment: Entity) -> DevelopmentalGraph {
        let mut graph = DevelopmentalGraph::new();
        graph.nodes.push(DevelopmentalNode {
            role: SegmentType::Head,
            outputs: sample_outputs(SegmentType::Head),
            parent: None,
            is_branch: false,
            position: 0,
            entity: Some(head),
        });
        graph.nodes.push(DevelopmentalNode {
            role: SegmentType::Torso,
            outputs: sample_outputs(SegmentType::Torso),
            parent: Some(0),
            is_branch: false,
            position: 1,
            entity: Some(segment),
        });
        graph
    }

    #[test]
    fn segment_infection_spreads_from_an_infectious_head() {
        let mut world = World::new();
        let head = world
            .spawn(Infection {
                state: InfectionState::Infectious,
                ticks_in_state: 0,
                virulence: 5.0,
                transmissibility: 0.1,
            })
            .id();
        let segment = world
            .spawn((full_economy(), SegmentInfection::healthy()))
            .id();
        let graph = build_graph(head, segment);
        world.entity_mut(head).insert(graph);

        world.run_system_once(segment_infection_system);

        let severity = world.get::<SegmentInfection>(segment).unwrap().severity;
        assert!(
            severity > 0.0,
            "segment should have picked up some severity from the infectious head"
        );
    }

    #[test]
    fn segment_immunity_clears_severity_when_head_is_not_infectious() {
        let mut world = World::new();
        let head = world
            .spawn(Infection {
                state: InfectionState::Recovered,
                ticks_in_state: 0,
                virulence: 5.0,
                transmissibility: 0.1,
            })
            .id();
        let segment = world
            .spawn((
                full_economy(),
                SegmentInfection { severity: 0.5 },
                SegmentImmunity::baseline(),
            ))
            .id();
        let graph = build_graph(head, segment);
        world.entity_mut(head).insert(graph);

        world.run_system_once(segment_infection_system);

        let severity = world.get::<SegmentInfection>(segment).unwrap().severity;
        assert!(
            severity < 0.5,
            "resistance should reduce severity once the source infection has cleared"
        );
    }

    #[test]
    fn high_severity_drains_the_segments_own_atp() {
        let mut world = World::new();
        let head = world
            .spawn(Infection {
                state: InfectionState::Infectious,
                ticks_in_state: 0,
                virulence: 5.0,
                transmissibility: 0.1,
            })
            .id();
        let segment = world
            .spawn((full_economy(), SegmentInfection { severity: 0.8 }))
            .id();
        let graph = build_graph(head, segment);
        world.entity_mut(head).insert(graph);

        let atp_before = world.get::<ChemicalEconomy>(segment).unwrap().atp;
        world.run_system_once(segment_infection_system);
        let atp_after = world.get::<ChemicalEconomy>(segment).unwrap().atp;

        assert!(
            atp_after < atp_before,
            "a segment with nonzero severity should have its ATP drained"
        );
    }

    #[test]
    fn segment_infection_system_is_deterministic_across_repeated_runs() {
        let run = || {
            let mut world = World::new();
            let head = world
                .spawn(Infection {
                    state: InfectionState::Infectious,
                    ticks_in_state: 0,
                    virulence: 5.0,
                    transmissibility: 0.1,
                })
                .id();
            let segment = world
                .spawn((full_economy(), SegmentInfection::healthy()))
                .id();
            let graph = build_graph(head, segment);
            world.entity_mut(head).insert(graph);
            world.run_system_once(segment_infection_system);
            world.get::<SegmentInfection>(segment).unwrap().severity
        };
        assert_eq!(run(), run());
    }
}
