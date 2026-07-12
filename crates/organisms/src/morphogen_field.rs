//! Intra-organism morphogen diffusion. A morphogen is a simulated diffusible
//! signal that provides positional information during growth — named after
//! the biological concept (e.g. the Bicoid gradient in *Drosophila*
//! embryos), where a concentration gradient tells cells at different
//! positions how to differentiate. This module gives the growing tip of an
//! organism a real, decaying, graph-propagated signal that influences
//! *later* positions' own decode — the intra-organism half of the
//! morphogen model (an inter-organism/environmental half exists too, as a
//! world-space GPU diffusion layer sampled directly by `organisms::systems::growth_system`).
//!
//! **Why this is a "reaction-diffusion" system and not another
//! `transport_system`/`endocrine_diffusion_system` copy:** those two move or
//! relax an *existing, conserved or externally-anchored* quantity between
//! segments. A developmental morphogen has no such source — the source is
//! growth itself. Each newly-grown segment is seeded at
//! `MORPHOGEN_SEED_CONCENTRATION` (the "reaction"/emission term), then this
//! system both diffuses that value toward its neighbors along the Body Graph
//! edges *and* decays it every tick (the term neither `transport_system` nor
//! `endocrine_diffusion_system` needs, since neither models a signal that
//! fades on its own).
//!
//! **Where the signal actually reaches development:** `genetics::develop`'s
//! `develop_at_position_with_life_stage` already provides exactly the seam
//! this needs — a scalar folded additively into every regulatory gene's
//! external input, with `0.0` reproducing `develop_at_position`'s original
//! output exactly. Rather than adding a second, parallel parameter to
//! `genetics`, `organisms::systems::growth_system` simply adds this system's
//! field reading into the same additive signal channel before calling that
//! existing function. This also means `genetics::develop_at_position`/
//! `simulate_growth_timeline` are untouched by this system, so they remain a
//! pure, zero-field reference reconstruction — see
//! `developmental_graph::simulate_growth_timeline`'s doc comment for what
//! that implies about matching a live run exactly.

use crate::developmental_graph::DevelopmentalGraph;
use bevy_ecs::prelude::{Component, Entity, Query};
use std::collections::HashMap;

/// Fraction of the concentration gradient exchanged per tick between two
/// adjacent segments — untuned placeholder, same status as
/// `transport::TRANSPORT_RATE`/`endocrine::ENDOCRINE_RATE`.
const MORPHOGEN_DIFFUSION_RATE: f32 = 0.3;

/// Fraction of a segment's own concentration lost per tick, independent of
/// diffusion — the "reaction" (decay) term that makes this a real
/// reaction-diffusion signal rather than a pure relaxation.
const MORPHOGEN_DECAY_RATE: f32 = 0.1;

/// Concentration a freshly-grown segment (or the head, at spawn) starts at —
/// the emission source is growth itself, not a separate system.
pub const MORPHOGEN_SEED_CONCENTRATION: f32 = 1.0;

/// One segment's local morphogen concentration. Attached to every
/// `ParticleNode` entity (head, spine, and fin segments alike), mirroring
/// `metabolism::ChemicalEconomy`/`brain::HormoneLevel`'s existing per-segment
/// component pattern rather than a per-organism `Vec` indexed by graph
/// position — this crate already has two precedents for "per-segment
/// component + Body-Graph-edge-walking system" (`transport_system`,
/// `endocrine_diffusion_system`); a third bespoke shape would be needless
/// variation for the same underlying idea.
#[derive(Component, Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct MorphogenLevel {
    /// This segment's local morphogen concentration.
    pub concentration: f32,
}

/// # Morphogen Diffusion System
///
/// ## 1. What Happens
/// For every organism's persistent Body Graph, walks each parent/child edge
/// and relaxes both segments' [`MorphogenLevel`] toward each other, then
/// applies a uniform per-tick decay to every segment's concentration.
///
/// ## 2. Why It Happens
/// A newly-grown segment is seeded at [`MORPHOGEN_SEED_CONCENTRATION`]
/// (`organisms::systems::growth_system`, at spawn). Without this system that
/// value would sit inert on exactly one segment forever — this is the system
/// that actually makes it a spreading, fading developmental signal older
/// positions (and, each subsequent tick, the growing tip's own next decode)
/// can read a meaningfully different value from.
///
/// ## 3. How It Happens
/// Same edge-collection shape as `transport::transport_system`/
/// `endocrine::endocrine_diffusion_system` (deterministic parent-before-child
/// order from `DevelopmentalGraph.nodes`), but the relaxation is
/// bidirectional-and-decaying rather than mass-conserving
/// (`transport_system`) or one-directional-from-a-fixed-source
/// (`endocrine_diffusion_system`) — see this module's doc comment for why.
pub fn morphogen_diffusion_system(
    graphs: Query<&DevelopmentalGraph>,
    mut levels: Query<(Entity, &mut MorphogenLevel)>,
) {
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

    let mut snapshot: HashMap<Entity, f32> = levels
        .iter()
        .map(|(entity, level)| (entity, level.concentration))
        .collect();

    if snapshot.is_empty() {
        return;
    }

    for (parent, child) in edges {
        let (Some(&p), Some(&c)) = (snapshot.get(&parent), snapshot.get(&child)) else {
            continue;
        };
        let delta = (p - c) * MORPHOGEN_DIFFUSION_RATE;
        if let Some(slot) = snapshot.get_mut(&parent) {
            *slot -= delta;
        }
        if let Some(slot) = snapshot.get_mut(&child) {
            *slot += delta;
        }
    }

    for concentration in snapshot.values_mut() {
        *concentration *= 1.0 - MORPHOGEN_DECAY_RATE;
    }

    for (entity, mut level) in levels.iter_mut() {
        if let Some(&concentration) = snapshot.get(&entity) {
            level.concentration = concentration;
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

    fn fixture_outputs() -> DevelopmentalOutputs {
        DevelopmentalOutputs {
            segment_type: SegmentType::Torso,
            branches: false,
            actuation_amplitude: 0.0,
            actuation_phase: 0.0,
            pigment: [0.5, 0.5, 0.5],
            apoptosis: false,
        }
    }

    /// Builds a 3-node linear chain (head -> mid -> tail), each a real
    /// entity carrying `MorphogenLevel`, with the head seeded and the rest
    /// at zero — the minimal fixture for observing diffusion/decay.
    fn linear_fixture(world: &mut World) -> (Entity, Entity, Entity) {
        let head = world
            .spawn(MorphogenLevel {
                concentration: MORPHOGEN_SEED_CONCENTRATION,
            })
            .id();
        let mid = world.spawn(MorphogenLevel { concentration: 0.0 }).id();
        let tail = world.spawn(MorphogenLevel { concentration: 0.0 }).id();

        let mut graph = DevelopmentalGraph::new();
        graph.nodes.push(DevelopmentalNode {
            role: SegmentType::Head,
            outputs: fixture_outputs(),
            parent: None,
            is_branch: false,
            position: 0,
            entity: Some(head),
        });
        graph.nodes.push(DevelopmentalNode {
            role: SegmentType::Torso,
            outputs: fixture_outputs(),
            parent: Some(0),
            is_branch: false,
            position: 1,
            entity: Some(mid),
        });
        graph.nodes.push(DevelopmentalNode {
            role: SegmentType::Tail,
            outputs: fixture_outputs(),
            parent: Some(1),
            is_branch: false,
            position: 2,
            entity: Some(tail),
        });
        world.spawn(graph);

        (head, mid, tail)
    }

    #[test]
    fn morphogen_diffusion_system_spreads_concentration_from_a_seeded_head_toward_the_tail() {
        let mut world = World::new();
        let (head, mid, tail) = linear_fixture(&mut world);

        world.run_system_once(morphogen_diffusion_system);

        let head_level = world.get::<MorphogenLevel>(head).unwrap().concentration;
        let mid_level = world.get::<MorphogenLevel>(mid).unwrap().concentration;
        let tail_level = world.get::<MorphogenLevel>(tail).unwrap().concentration;

        // Mid received some of the head's concentration; tail (two hops
        // away) also received some in this same tick — edges are processed
        // in deterministic parent-before-child order against a running
        // snapshot (mirroring `transport_system`'s documented multi-hop
        // propagation), so mid's just-updated value is what tail's own edge
        // relaxes against, not its pre-tick value. Tail's share is smaller
        // than mid's, since it's relaxing against an already-partial value.
        assert!(mid_level > 0.0);
        assert!(tail_level > 0.0);
        assert!(tail_level < mid_level);
        // The head itself lost concentration to diffusion and decay.
        assert!(head_level < MORPHOGEN_SEED_CONCENTRATION);
    }

    #[test]
    fn morphogen_diffusion_system_eventually_decays_an_isolated_segment_toward_zero() {
        let mut world = World::new();
        let head = world
            .spawn(MorphogenLevel {
                concentration: MORPHOGEN_SEED_CONCENTRATION,
            })
            .id();
        // No `DevelopmentalGraph` at all — an isolated segment with no
        // edges, so only the decay term should apply.
        let mut graph = DevelopmentalGraph::new();
        graph.nodes.push(DevelopmentalNode {
            role: SegmentType::Head,
            outputs: fixture_outputs(),
            parent: None,
            is_branch: false,
            position: 0,
            entity: Some(head),
        });
        world.spawn(graph);

        for _ in 0..200 {
            world.run_system_once(morphogen_diffusion_system);
        }

        let level = world.get::<MorphogenLevel>(head).unwrap().concentration;
        assert!(level < 0.001, "expected near-zero decay, got {level}");
    }

    #[test]
    fn morphogen_diffusion_system_is_deterministic_for_the_same_starting_state() {
        let run_once = || {
            let mut world = World::new();
            let (_, mid, _) = linear_fixture(&mut world);
            for _ in 0..10 {
                world.run_system_once(morphogen_diffusion_system);
            }
            world.get::<MorphogenLevel>(mid).unwrap().concentration
        };

        assert_eq!(run_once(), run_once());
    }
}
