//! Per-region endocrine signalling — propagates the head's
//! [`brain::Neuromodulators`] reading out to every other body segment's own
//! [`brain::HormoneLevel`], along the same persistent Body Graph edges
//! `transport::transport_system` walks.
//!
//! **Why this is a different model from `transport`'s resource exchange:**
//! glucose/o2/atp are physical stuff — moving some to a neighbor means the
//! source has less.
//! A hormone reading is not consumed by being sensed elsewhere — the head's
//! own `Neuromodulators` state is unaffected by how far its signal has
//! reached. So each edge here is one-directional: the downstream segment's
//! `HormoneLevel` relaxes toward its parent's level; the parent's own value
//! (whether `Neuromodulators` on the head, or an upstream segment's own
//! `HormoneLevel`) is left untouched by this system.

use crate::developmental_graph::DevelopmentalGraph;
use bevy_ecs::prelude::{Entity, Query};
use brain::{HormoneLevel, Neuromodulators};
use std::collections::HashMap;

/// Fraction of the gap to a segment's upstream parent closed per tick —
/// an untuned placeholder, same status as `transport::TRANSPORT_RATE`.
const ENDOCRINE_RATE: f32 = 0.2;

/// A snapshot of the three channels [`Neuromodulators`]/[`HormoneLevel`]
/// both carry, so edge relaxation doesn't need to know which of the two
/// component types a given entity actually has.
#[derive(Clone, Copy)]
struct Channels {
    dopamine: f32,
    serotonin: f32,
    noradrenaline: f32,
}

impl From<&Neuromodulators> for Channels {
    fn from(n: &Neuromodulators) -> Self {
        Self {
            dopamine: n.dopamine,
            serotonin: n.serotonin,
            noradrenaline: n.noradrenaline,
        }
    }
}

impl From<&HormoneLevel> for Channels {
    fn from(h: &HormoneLevel) -> Self {
        Self {
            dopamine: h.dopamine,
            serotonin: h.serotonin,
            noradrenaline: h.noradrenaline,
        }
    }
}

/// # Endocrine Diffusion System
///
/// ## 1. What Happens
/// For every organism's persistent Body Graph, walks each parent/child edge
/// and relaxes the child segment's `HormoneLevel` toward its parent's own
/// channel reading (the head's `Neuromodulators`, or an upstream segment's
/// `HormoneLevel`).
///
/// ## 2. Why It Happens
/// `Neuromodulators` was always an organism-wide scalar with no spatial
/// concept — a distant segment "felt" a stress/reward signal exactly
/// as fast and exactly as strongly as the head itself did, which has no
/// physical analogue (real endocrine signalling takes time and attenuates
/// over distance from the source). This system gives that signal an actual
/// travel path along the organism's own anatomy.
///
/// ## 3. How It Happens
/// Same structural approach as `transport::transport_system` (edges
/// collected from the Body Graph, processed in deterministic
/// parent-before-child order so a multi-hop chain can propagate more than
/// one edge per tick) but with a one-directional relaxation instead of a
/// mass-conserving exchange — see this module's doc comment.
pub fn endocrine_diffusion_system(
    graphs: Query<&DevelopmentalGraph>,
    neuromodulators: Query<&Neuromodulators>,
    mut hormone_levels: Query<(Entity, &mut HormoneLevel)>,
) {
    // 1. Collect edges, in deterministic (per-graph, parent-before-child)
    // order — identical shape to `transport::transport_system`'s first pass.
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

    // 2. Snapshot every segment's current `HormoneLevel` (the only side that
    // can change here).
    let mut snapshot: HashMap<Entity, Channels> = hormone_levels
        .iter()
        .map(|(entity, level)| (entity, Channels::from(level)))
        .collect();

    // 3. Relax each edge, in order. The parent side is read from
    // `Neuromodulators` (head, constant) or the running snapshot (an
    // upstream segment, possibly already updated this tick); only the child
    // side is ever written.
    for (parent, child) in edges {
        let parent_channels = if let Ok(neuro) = neuromodulators.get(parent) {
            Channels::from(neuro)
        } else if let Some(channels) = snapshot.get(&parent) {
            *channels
        } else {
            continue;
        };

        if let Some(child_channels) = snapshot.get_mut(&child) {
            child_channels.dopamine +=
                (parent_channels.dopamine - child_channels.dopamine) * ENDOCRINE_RATE;
            child_channels.serotonin +=
                (parent_channels.serotonin - child_channels.serotonin) * ENDOCRINE_RATE;
            child_channels.noradrenaline +=
                (parent_channels.noradrenaline - child_channels.noradrenaline) * ENDOCRINE_RATE;
        }
    }

    // 4. Apply final values back to the ECS.
    for (entity, mut level) in hormone_levels.iter_mut() {
        if let Some(channels) = snapshot.get(&entity) {
            level.dopamine = channels.dopamine;
            level.serotonin = channels.serotonin;
            level.noradrenaline = channels.noradrenaline;
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

    fn build_chain(n_segments: usize) -> (World, Entity, Vec<Entity>) {
        let mut world = World::new();
        // `Neuromodulators` has a private field (`last_atp`), so it's built
        // via its own constructor rather than a struct literal, then driven
        // to a known non-zero state through `update`.
        let mut neuro = Neuromodulators::new(100.0);
        neuro.update(100.0, 100.0); // drives serotonin to 1.0, noradrenaline to 0.0
        let head = world.spawn(neuro).id();

        let mut graph = DevelopmentalGraph::new();
        graph.nodes.push(DevelopmentalNode {
            role: SegmentType::Head,
            outputs: sample_outputs(SegmentType::Head),
            parent: None,
            is_branch: false,
            position: 0,
            entity: Some(head),
        });

        let mut segments = Vec::new();
        let mut parent_index = 0;
        for i in 0..n_segments {
            let segment = world.spawn(HormoneLevel::default()).id();
            graph.nodes.push(DevelopmentalNode {
                role: SegmentType::Torso,
                outputs: sample_outputs(SegmentType::Torso),
                parent: Some(parent_index),
                is_branch: false,
                position: i + 1,
                entity: Some(segment),
            });
            parent_index = graph.nodes.len() - 1;
            segments.push(segment);
        }
        world.entity_mut(head).insert(graph);

        (world, head, segments)
    }

    #[test]
    fn endocrine_diffusion_moves_a_segment_toward_the_heads_level() {
        let (mut world, _head, segments) = build_chain(1);
        let segment = segments[0];

        world.run_system_once(endocrine_diffusion_system);
        let level = world.get::<HormoneLevel>(segment).unwrap();
        assert!(
            level.serotonin > 0.0,
            "segment should have moved toward the head's serotonin reading"
        );
    }

    #[test]
    fn endocrine_diffusion_never_modifies_the_heads_own_neuromodulators() {
        let (mut world, head, _segments) = build_chain(1);
        let before = *world.get::<Neuromodulators>(head).unwrap();

        world.run_system_once(endocrine_diffusion_system);

        let after = world.get::<Neuromodulators>(head).unwrap();
        assert_eq!(before.dopamine, after.dopamine);
        assert_eq!(before.serotonin, after.serotonin);
        assert_eq!(before.noradrenaline, after.noradrenaline);
    }

    #[test]
    fn endocrine_diffusion_propagates_through_a_multi_hop_chain_in_one_tick() {
        let (mut world, _head, segments) = build_chain(3);
        world.run_system_once(endocrine_diffusion_system);
        let tip = world.get::<HormoneLevel>(segments[2]).unwrap();
        assert!(
            tip.serotonin > 0.0,
            "the tip of a 3-segment chain should still receive some signal within one tick"
        );
    }

    #[test]
    fn endocrine_diffusion_is_deterministic_across_repeated_runs() {
        let run = || {
            let (mut world, _head, segments) = build_chain(3);
            world.run_system_once(endocrine_diffusion_system);
            segments
                .iter()
                .map(|&e| {
                    let level = world.get::<HormoneLevel>(e).unwrap();
                    (level.dopamine, level.serotonin, level.noradrenaline)
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }
}
