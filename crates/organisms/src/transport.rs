//! Intra-body resource transport — moves glucose, oxygen, ATP, and carbon
//! dioxide along the persistent
//! [`crate::developmental_graph::DevelopmentalGraph`]'s parent/child edges,
//! the same edges `physics::Spring` uses to hold the body together
//! structurally. This is the consumer every body segment's per-segment
//! `metabolism::ChemicalEconomy` pool was seeded for: without it, those
//! pools would be inert placeholder data with nothing connecting them to
//! each other.
//!
//! **Waste expulsion reuses the same mechanism, not a separate one:** no
//! segment produces co2 locally — only `metabolism::compute_metabolism`
//! (organism-level respiration, head-only) does, and `metabolism_system`
//! actively vents the head's co2 toward `GlobalAtmosphere` every tick,
//! keeping the head's own co2 low relative to any that spreads out to
//! segments. Simply including co2 in the same equalizing relaxation below
//! is therefore sufficient to model waste expulsion: the head-vs-segment
//! gradient the venting maintains pulls co2 back toward the head (and out)
//! on its own, with no separate "expulsion" system needed.
//!
//! **Model:** each edge independently relaxes toward equalizing
//! concentration with its neighbor, one bounded step per tick — the same
//! `new = old + (target - old) * rate` shape already used by
//! `metabolism::atmosphere_homeostasis_system`, applied per-edge instead of
//! to a single global pool. Transfers are mass-conserving (what leaves one
//! side is exactly what arrives at the other) and capacity-aware (never
//! pushes a pool over its max or below zero).

use crate::developmental_graph::DevelopmentalGraph;
use bevy_ecs::prelude::{Entity, ParamSet, Query};
use metabolism::ChemicalEconomy;
use std::collections::HashMap;

/// Fraction of the concentration gradient exchanged per tick, per resource,
/// per edge. Not biologically tuned — a placeholder rate, same status as
/// `ChemicalEconomy::segment_default()`'s pool sizes.
const TRANSPORT_RATE: f32 = 0.15;

/// Moves `amount` of one resource from `from` to `to` (or the reverse, if
/// the gradient points that way), bounded by both sides' capacity and
/// current availability — so a transfer never creates or destroys mass,
/// and never drives a pool negative or over its max.
fn relax_toward_equilibrium(from: f32, from_max: f32, to: f32, to_max: f32) -> (f32, f32) {
    let raw = (from - to) * TRANSPORT_RATE;
    if raw >= 0.0 {
        let room = (to_max - to).max(0.0);
        let available = from.max(0.0);
        let actual = raw.min(room).min(available);
        (from - actual, to + actual)
    } else {
        let raw = -raw;
        let room = (from_max - from).max(0.0);
        let available = to.max(0.0);
        let actual = raw.min(room).min(available);
        (from + actual, to - actual)
    }
}

/// # Intra-Body Transport System
///
/// ## 1. What Happens
/// For every organism's persistent Body Graph, walks each parent/child edge
/// and exchanges glucose, oxygen, ATP, and carbon dioxide between the two
/// segments' own `ChemicalEconomy` pools, moving each toward the other's
/// concentration.
///
/// ## 2. Why It Happens
/// Every body segment has its own small resource pool, but without this
/// system nothing connects them to each other or to the organism's main
/// (head) pool — a segment that used up its glucose would have no way to
/// receive more, and the head's large pool would have no way to reach the
/// rest of the body. This system is that connective pass: a
/// circulatory/respiratory/digestive transport model scoped to the
/// organism's own anatomy, rather than the world-space `diffusion` crate's
/// PDE grid.
///
/// ## 3. How It Happens
/// 1. **Collect edges:** every `(parent_entity, child_entity)` pair implied
///    by each organism's `DevelopmentalGraph.nodes[i].parent`, skipping any
///    node whose own or parent's `entity` is `None` (not yet materialized,
///    or a pruned/apoptotic position).
/// 2. **Snapshot:** read each referenced entity's current `ChemicalEconomy`
///    once into a local map — entities that no longer have the component
///    (e.g. despawned by a broken `Spring`) are silently skipped, not
///    treated as an error.
/// 3. **Relax:** process edges in the graph's own (parent-before-child)
///    insertion order, updating the snapshot map in place — this lets a
///    multi-hop chain (head → spine → spine → fin) propagate more than one
///    edge per tick, deterministically, since the order is fixed by growth
///    order, not iteration/thread scheduling.
/// 4. **Apply:** write every touched entity's final values back.
///
/// No RNG, no cross-organism shared state, no `rayon` — the per-organism
/// edge lists are small (`organisms::MAX_SEGMENTS`-bounded) and processing
/// them sequentially keeps the ordering trivially deterministic, matching
/// this codebase's standing requirement (see `metabolism_system`'s doc
/// comment for why that determinism is treated as load-bearing, not
/// incidental).
pub fn transport_system(
    graphs: Query<&DevelopmentalGraph>,
    mut chem_params: ParamSet<(Query<&ChemicalEconomy>, Query<&mut ChemicalEconomy>)>,
) {
    // 1. Collect edges, in deterministic (per-graph, parent-before-child)
    // order.
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

    // 2. Snapshot every entity referenced by at least one edge.
    let chem_reader = chem_params.p0();
    let mut snapshot: HashMap<Entity, ChemicalEconomy> = HashMap::new();
    for &(a, b) in &edges {
        for entity in [a, b] {
            if let std::collections::hash_map::Entry::Vacant(slot) = snapshot.entry(entity) {
                if let Ok(chem) = chem_reader.get(entity) {
                    slot.insert(chem.clone());
                }
            }
        }
    }

    // 3. Relax each edge, in order, against the running snapshot.
    for (parent, child) in edges {
        let (Some(p), Some(c)) = (snapshot.get(&parent), snapshot.get(&child)) else {
            continue;
        };
        let (p_glucose, c_glucose) =
            relax_toward_equilibrium(p.glucose, p.max_glucose, c.glucose, c.max_glucose);
        let (p_o2, c_o2) = relax_toward_equilibrium(p.o2, p.max_o2, c.o2, c.max_o2);
        let (p_atp, c_atp) = relax_toward_equilibrium(p.atp, p.max_atp, c.atp, c.max_atp);
        let (p_co2, c_co2) = relax_toward_equilibrium(p.co2, p.max_co2, c.co2, c.max_co2);

        if let Some(p) = snapshot.get_mut(&parent) {
            p.glucose = p_glucose;
            p.o2 = p_o2;
            p.atp = p_atp;
            p.co2 = p_co2;
        }
        if let Some(c) = snapshot.get_mut(&child) {
            c.glucose = c_glucose;
            c.o2 = c_o2;
            c.atp = c_atp;
            c.co2 = c_co2;
        }
    }

    // 4. Apply final values back to the ECS.
    let mut chem_writer = chem_params.p1();
    for (entity, chem) in snapshot {
        if let Ok(mut existing) = chem_writer.get_mut(entity) {
            existing.glucose = chem.glucose;
            existing.o2 = chem.o2;
            existing.atp = chem.atp;
            existing.co2 = chem.co2;
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

    fn rich_economy() -> ChemicalEconomy {
        ChemicalEconomy {
            glucose: 200.0,
            o2: 100.0,
            co2: 0.0,
            atp: 200.0,
            max_glucose: 200.0,
            max_o2: 100.0,
            max_co2: 100.0,
            max_atp: 200.0,
        }
    }

    /// A head that has been actively venting co2 (per `metabolism_system`'s
    /// own behavior) — low co2 relative to a segment that has accumulated
    /// some, modeling the gradient that pulls co2 back toward the head.
    fn vented_head_economy() -> ChemicalEconomy {
        ChemicalEconomy {
            co2: 0.0,
            ..rich_economy()
        }
    }

    fn congested_segment_economy() -> ChemicalEconomy {
        ChemicalEconomy {
            co2: 80.0,
            ..ChemicalEconomy::segment_default()
        }
    }

    #[test]
    fn relax_toward_equilibrium_conserves_total_mass() {
        let (from, to) = relax_toward_equilibrium(100.0, 200.0, 0.0, 200.0);
        assert!((from + to - 100.0).abs() < 1e-6);
        assert!(from < 100.0);
        assert!(to > 0.0);
    }

    #[test]
    fn relax_toward_equilibrium_never_exceeds_capacity_or_goes_negative() {
        let (from, to) = relax_toward_equilibrium(10.0, 10.0, 9.9, 10.0);
        assert!(to <= 10.0);
        assert!(from >= 0.0);
    }

    #[test]
    fn transport_system_moves_resources_from_a_full_head_to_an_empty_segment() {
        let mut world = World::new();
        let head = world.spawn(rich_economy()).id();
        let segment = world.spawn(ChemicalEconomy::segment_default()).id();

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
        world.entity_mut(head).insert(graph);

        let segment_glucose_before = world.get::<ChemicalEconomy>(segment).unwrap().glucose;
        world.run_system_once(transport_system);
        let segment_glucose_after = world.get::<ChemicalEconomy>(segment).unwrap().glucose;

        assert!(
            segment_glucose_after > segment_glucose_before,
            "segment should have received glucose from the head's fuller pool"
        );
    }

    #[test]
    fn transport_system_skips_a_node_with_no_materialized_entity() {
        // `simulate_growth_timeline`'s pure reconstruction leaves `entity`
        // `None` — a graph built that way must not panic or error when fed
        // through this system, it should simply produce no edges.
        let mut world = World::new();
        let head = world.spawn(rich_economy()).id();

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
            entity: None,
        });
        world.entity_mut(head).insert(graph);

        let glucose_before = world.get::<ChemicalEconomy>(head).unwrap().glucose;
        world.run_system_once(transport_system);
        let glucose_after = world.get::<ChemicalEconomy>(head).unwrap().glucose;
        assert_eq!(glucose_before, glucose_after);
    }

    #[test]
    fn transport_system_pulls_co2_from_a_congested_segment_toward_a_vented_head() {
        // co2 moves too — a segment with accumulated co2 next to a head
        // kept low by `metabolism_system`'s own venting
        // should see its co2 decrease (flowing toward, and eventually out
        // through, the head).
        let mut world = World::new();
        let head = world.spawn(vented_head_economy()).id();
        let segment = world.spawn(congested_segment_economy()).id();

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
        world.entity_mut(head).insert(graph);

        let segment_co2_before = world.get::<ChemicalEconomy>(segment).unwrap().co2;
        world.run_system_once(transport_system);
        let segment_co2_after = world.get::<ChemicalEconomy>(segment).unwrap().co2;
        let head_co2_after = world.get::<ChemicalEconomy>(head).unwrap().co2;

        assert!(
            segment_co2_after < segment_co2_before,
            "segment's co2 should decrease as it flows toward the lower-co2 head"
        );
        assert!(
            head_co2_after > 0.0,
            "head should have received the co2 that left the segment"
        );
    }

    /// Builds a 3-node chain (head → torso → tail) with a full head and two
    /// empty downstream segments, returning the final `(glucose, o2, atp)`
    /// of each of the three entities after one `transport_system` tick.
    fn run_three_node_chain() -> [(f32, f32, f32); 3] {
        let mut world = World::new();
        let head = world.spawn(rich_economy()).id();
        let torso = world.spawn(ChemicalEconomy::segment_default()).id();
        let tail = world.spawn(ChemicalEconomy {
            glucose: 0.0,
            o2: 0.0,
            co2: 0.0,
            atp: 0.0,
            max_glucose: 200.0,
            max_o2: 100.0,
            max_co2: 100.0,
            max_atp: 200.0,
        });
        let tail = tail.id();

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
            entity: Some(torso),
        });
        graph.nodes.push(DevelopmentalNode {
            role: SegmentType::Tail,
            outputs: sample_outputs(SegmentType::Tail),
            parent: Some(1),
            is_branch: false,
            position: 2,
            entity: Some(tail),
        });
        world.entity_mut(head).insert(graph);

        world.run_system_once(transport_system);

        [head, torso, tail].map(|e| {
            let chem = world.get::<ChemicalEconomy>(e).unwrap();
            (chem.glucose, chem.o2, chem.atp)
        })
    }

    #[test]
    fn transport_system_propagates_through_a_multi_hop_chain_in_one_tick() {
        // The tail (two hops from the head) should still receive some
        // glucose in a single tick, since edges are relaxed in
        // parent-before-child order against a running snapshot.
        let [_head, _torso, tail] = run_three_node_chain();
        assert!(
            tail.0 > 0.0,
            "tail should receive glucose relayed through the torso within one tick"
        );
    }

    #[test]
    fn transport_system_is_deterministic_across_repeated_runs() {
        let run_a = run_three_node_chain();
        let run_b = run_three_node_chain();
        assert_eq!(
            run_a, run_b,
            "identical starting state must produce identical output"
        );
    }
}
