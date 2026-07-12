//! Brain-wiring for a just-completed organism (Phase 6, Epic C, N1b/N1c;
//! extracted from `systems.rs` as its own file — Phase 9, P9.6 — since it is
//! a genuinely separate concern from body growth: this module turns a
//! finished `DevelopmentalGraph` into a wired `Brain`, while `systems.rs`
//! itself is about growing the body one segment at a time. The two were
//! bundled in one file only because `wire_brain_for_completed_organism` was
//! originally `growth_system`'s own inline "Phase 1" block (see its doc
//! comment below) before Phase 7, W5a extracted it as a named function.

use crate::components::GrowthState;
use crate::developmental_graph::DevelopmentalGraph;
use bevy_ecs::prelude::{Commands, Entity, Query};

/// Base weight-magnitude threshold for wiring a synapse between two nodes
/// in the same neural region (`brain::RegionId`) — unchanged from the
/// single-threshold rule that existed before Phase 6, Epic C's N1b.
const SYNAPSE_WEIGHT_THRESHOLD: f32 = 0.01;

/// Cross-region synapses require a substantially stronger CPPN-evaluated
/// weight to be wired at all — the "sparser cross-region wiring" N1b adds,
/// per `PHASE4_EPIC1_NEURAL_ROADMAP.md`'s N1b goal ("cross-region pairs are
/// wired more sparsely... replacing pure all-pairs-via-index-CPPN with a
/// rule that actually reflects anatomy"). Every node still defaults to
/// `brain::RegionId::Central` for any organism with no decoded
/// `SegmentType::Ganglion` segment (every organism observed so far, since
/// nothing has evolved one yet — N1c, Phase 6, Epic C, added the real
/// detection logic, but finding an actual Ganglion in a real population is
/// a separate, unverified question) — so `same_region` is still `true` for
/// every node pair in practice today, and this constant has no observed
/// effect yet, exactly the graceful-degradation property both N1b's and
/// N1c's own test requirements ask for.
const CROSS_REGION_SYNAPSE_WEIGHT_THRESHOLD: f32 = 0.5;

/// Decides whether a CPPN-evaluated synapse weight is strong enough to
/// wire, given whether the source/target nodes share a neural region.
/// Extracted as its own pure function (Phase 6, Epic C, N1b) so the
/// region-aware wiring rule is unit-testable independent of
/// `growth_system`'s full genome/CPPN machinery.
fn should_wire_synapse(weight: f32, same_region: bool) -> bool {
    let threshold = if same_region {
        SYNAPSE_WEIGHT_THRESHOLD
    } else {
        CROSS_REGION_SYNAPSE_WEIGHT_THRESHOLD
    };
    weight.abs() > threshold
}

/// Assigns every brain node's neural region for one organism (Phase 6,
/// Epic C, N1c) — anchoring each *hidden* CTRNN node to the nearest decoded
/// `genetics::SegmentType::Ganglion` position, by real graph-topological
/// distance (`DevelopmentalGraph::graph_distance`), if the organism has one
/// or more Ganglion segments; `brain::RegionId::Central` otherwise (every
/// organism observed so far, since nothing has evolved a Ganglion segment
/// yet). Input and output nodes are never reassigned — they already have a
/// real identity (a sensor or an effector) that a Ganglion anchor wouldn't
/// add anything to; only hidden nodes are abstract CTRNN units with no
/// inherent body position, which is exactly what this function gives them
/// one for.
///
/// A hidden node has no position of its own, so it's anchored to an
/// evenly-spread target position along the body axis — its own index among
/// just the hidden nodes, scaled across `crate::MAX_SEGMENTS` — then
/// matched to whichever decoded Ganglion is nearest to that target
/// position by graph distance (ties broken by the lower body position, to
/// stay fully deterministic). If no real segment was ever decoded at the
/// exact target position (e.g. pruned by apoptosis), that hidden node
/// falls back to `Central` rather than guessing — a real, disclosed
/// limitation, not silently papered over.
fn assign_hidden_node_regions(
    graph: &DevelopmentalGraph,
    input_count: usize,
    hidden_count: usize,
    total_nodes: usize,
) -> Vec<brain::RegionId> {
    let ganglion_positions: Vec<usize> = graph
        .nodes
        .iter()
        .filter(|n| n.role == genetics::SegmentType::Ganglion)
        .map(|n| n.position)
        .collect();

    (0..total_nodes)
        .map(|i| {
            let is_hidden = i >= input_count && i < input_count + hidden_count;
            if !is_hidden || ganglion_positions.is_empty() {
                return brain::RegionId::Central;
            }
            let hidden_index = i - input_count;
            let target_position = hidden_index * crate::MAX_SEGMENTS / hidden_count.max(1);
            let Some(target_index) = graph.index_at_position(target_position) else {
                return brain::RegionId::Central;
            };
            ganglion_positions
                .iter()
                .filter_map(|&pos| {
                    graph.index_at_position(pos).map(|ganglion_index| {
                        (pos, graph.graph_distance(target_index, ganglion_index))
                    })
                })
                .min_by_key(|&(pos, distance)| (distance, pos))
                .map(|(pos, _)| brain::RegionId::Ganglion(pos))
                .unwrap_or(brain::RegionId::Central)
        })
        .collect()
}

/// Phase 1 of `growth_system` (Phase 7, W5a): once an organism's body is
/// fully grown, wires its CTRNN brain — input/hidden/output nodes,
/// Braitenberg fin wiring, CPPN-evolved biases/weights/regions — and
/// removes `GrowthState`, marking growth complete. Verbatim extraction of
/// `growth_system`'s original inline `if is_finished` block; no logic
/// changed, only named and separated from the segment-growth phases below.
#[allow(clippy::too_many_arguments)]
pub(crate) fn wire_brain_for_completed_organism(
    commands: &mut Commands,
    entity: Entity,
    state: &GrowthState,
    graph: &DevelopmentalGraph,
    spring_query: &Query<&physics::Spring>,
    chem_query: &Query<&metabolism::ChemicalEconomy>,
    expressed_brain_cppn: &genetics::Cppn,
) {
    // 3 scalar inputs (Olfaction, ATP, Age) + 9 Vision inputs (Phase 8,
    // Epic 8.7, ADR-P8-07's 3×3 azimuth×elevation grid, up from 3
    // pre-8.7) + 1 Signal input + 1 Hazard input + 1 Pacemaker
    let input_count = 15;
    // effectors + 1 SignalEmitter output
    let output_count = state.effectors.len() + 1;

    let hidden_count = 4;
    let total_nodes = input_count + hidden_count + output_count;

    let mut nodes = Vec::new();
    let mut synapses = Vec::new();

    for i in 0..total_nodes {
        let mut bias = 0.0;
        let mut time_constant = 0.5;
        let mut activation = 1; // Tanh

        if i < input_count {
            time_constant = 1.0;
            activation = 7; // Linear
        } else {
            // Evolve node properties via Brain CPPN
            if !expressed_brain_cppn.nodes.is_empty() {
                let w_inputs = [
                    (i as f32) / (total_nodes as f32),
                    (i as f32) / (total_nodes as f32),
                ];
                let w_outputs = expressed_brain_cppn.evaluate(&w_inputs);
                if w_outputs.len() >= 3 {
                    bias = w_outputs[1] * 1.5;
                    // Time constant must be strictly positive and low enough to allow fast 2 Hz oscillations.
                    time_constant = w_outputs[2].abs().clamp(0.1, 2.0);
                }
            }
        }

        nodes.push(brain::CtrnnNode {
            state: 0.0,
            time_constant,
            bias,
            activation,
            first_synapse: 0,
            synapse_count: 0,
        });
    }

    // Find fins for Braitenberg wiring
    let mut left_fin_idx = None;
    let mut right_fin_idx = None;
    for (out_idx, &effector_entity) in state.effectors.iter().enumerate() {
        if let Ok(spring) = spring_query.get(effector_entity) {
            if spring.is_fin == 1 {
                if left_fin_idx.is_none() {
                    left_fin_idx = Some(input_count + hidden_count + out_idx);
                } else if right_fin_idx.is_none() {
                    right_fin_idx = Some(input_count + hidden_count + out_idx);
                }
            }
        }
    }

    // Phase 6, Epic C (N1c): every node's neural region, parallel
    // to `nodes`. Built here (not left to `Brain::new`'s own
    // default) so the wiring loop below can consult it while
    // deciding which synapses to keep, and so the exact same
    // vector becomes the constructed `Brain`'s `node_regions`
    // afterward. See `assign_hidden_node_regions`'s own doc
    // comment for the actual Ganglion-anchoring logic — extracted
    // as its own function so it's unit-testable against a
    // hand-built `DevelopmentalGraph` fixture, independent of
    // genome/CPPN decoding (mirroring N1b's `should_wire_synapse`
    // extraction for the same reason).
    let node_regions = assign_hidden_node_regions(graph, input_count, hidden_count, total_nodes);

    for i in 0..total_nodes {
        // Connections can only target hidden and output nodes (not inputs)
        for j in input_count..total_nodes {
            let mut weight = 0.0;

            // Neocortex: Evolved CPPN Weights
            if !expressed_brain_cppn.nodes.is_empty() {
                let w_inputs = [
                    (i as f32) / (total_nodes as f32),
                    (j as f32) / (total_nodes as f32),
                ];
                let w_outputs = expressed_brain_cppn.evaluate(&w_inputs);
                if !w_outputs.is_empty() {
                    weight += w_outputs[0] * 1.5;
                }
            }

            let same_region = node_regions[i] == node_regions[j];
            if should_wire_synapse(weight, same_region) {
                synapses.push(brain::CtrnnSynapse {
                    source: i as u32,
                    target: j as u32,
                    weight,
                    _padding: 0,
                });
            }
        }
    }

    let initial_atp = chem_query.get(entity).map(|c| c.atp).unwrap_or(0.0);

    let mut brain = brain::Brain::new(
        brain::BrainId(0),
        nodes,
        synapses,
        input_count,
        output_count,
    );
    // `Brain::new` already defaults every node to `RegionId::Central`
    // (N1a), so this is a no-op today — written explicitly so N1c's
    // real Ganglion-detected `node_regions` (computed above, for the
    // wiring loop's own use) propagates into the constructed `Brain`
    // once that milestone replaces the all-`Central` vector above
    // with real detection, without needing a second change here.
    brain.node_regions = node_regions;

    commands.entity(entity).insert((
        brain,
        brain::Neuromodulators::new(initial_atp),
        sensing::SensoryState::new(input_count),
        behavior::MotorSystem {
            effectors: state.effectors.clone(),
        },
        diffusion::SignalEmitter::default(),
    ));
    commands.entity(entity).remove::<GrowthState>();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Phase 6, Epic C (N1b)'s own named test requirement: `should_wire_synapse`'s
    /// same-region branch must behave *identically* to the single-threshold
    /// rule that existed before N1b, for any weight — proving the region-aware
    /// wiring rule is mathematically a no-op whenever every node is
    /// `RegionId::Central` (true for every organism today, since N1c hasn't
    /// landed to assign anything else). This is a direct proof of "graceful
    /// degradation," not an indirect golden-topology snapshot.
    #[test]
    fn should_wire_synapse_same_region_matches_the_pre_n1b_single_threshold_rule() {
        let weights: [f32; 11] = [
            -2.0, -0.5, -0.011, -0.01, -0.005, 0.0, 0.005, 0.01, 0.011, 0.5, 2.0,
        ];
        for weight in weights {
            let old_rule = weight.abs() > 0.01;
            assert_eq!(
                should_wire_synapse(weight, true),
                old_rule,
                "same-region wiring decision changed for weight {weight} — this must stay identical to the pre-N1b rule"
            );
        }
    }

    /// The other half of N1b's own named test requirement: cross-region
    /// pairs must be wired more sparsely than same-region pairs for the
    /// same weight — the actual new behavior N1b adds, tested directly
    /// since nothing in this milestone's own scope produces a real
    /// cross-region organism yet (that's N1c).
    #[test]
    fn should_wire_synapse_cross_region_requires_a_stronger_weight_than_same_region() {
        // A weight that would wire within the same region but is too weak
        // to cross a region boundary — proving cross-region density is
        // measurably lower for any weight distribution with mass in this range.
        let weight = 0.2;
        assert!(should_wire_synapse(weight, true));
        assert!(!should_wire_synapse(weight, false));
    }

    /// Builds a minimal `DevelopmentalGraph` fixture directly (bypassing
    /// genome/CPPN decoding, mirroring N1b's own `should_wire_synapse` test
    /// strategy), with one explicit `SegmentType::Ganglion` segment — since
    /// nothing in `growth_system` itself can decode a real one yet (that
    /// would require reverse-engineering a genome that happens to produce
    /// one, which is out of scope here). Proves N1c's own first named test
    /// requirement: a hidden node's assigned region must match the nearest
    /// Ganglion *by body-graph distance*, not by coincidentally matching
    /// raw node index.
    fn linear_fixture_graph_with_ganglion_at(position: usize) -> DevelopmentalGraph {
        use genetics::{DevelopmentalOutputs, SegmentType};
        let outputs = |t: SegmentType| DevelopmentalOutputs {
            segment_type: t,
            branches: false,
            actuation_amplitude: 0.0,
            actuation_phase: 0.0,
            pigment: [0.5, 0.5, 0.5],
            apoptosis: false,
        };
        let mut graph = DevelopmentalGraph::new();
        let mut last = graph.push(
            SegmentType::Head,
            outputs(SegmentType::Head),
            None,
            false,
            0,
            None,
        );
        for pos in 1..crate::MAX_SEGMENTS {
            let role = if pos == position {
                SegmentType::Ganglion
            } else {
                SegmentType::Torso
            };
            last = graph.push(role, outputs(role), Some(last), false, pos, None);
        }
        graph
    }

    #[test]
    fn assign_hidden_node_regions_anchors_hidden_nodes_to_the_nearest_ganglion_by_graph_distance() {
        // A single Ganglion near the tail (position 10 of 0..15) — every
        // hidden node's evenly-spread target position should end up
        // anchored to this one Ganglion, since it's the only candidate,
        // regardless of raw index proximity.
        let graph = linear_fixture_graph_with_ganglion_at(10);
        let input_count = 9;
        let hidden_count = 4;
        let output_count = 2;
        let total_nodes = input_count + hidden_count + output_count;

        let regions = assign_hidden_node_regions(&graph, input_count, hidden_count, total_nodes);

        assert_eq!(regions.len(), total_nodes);
        // Inputs and outputs must never be reassigned.
        for region in &regions[0..input_count] {
            assert_eq!(*region, brain::RegionId::Central);
        }
        for region in &regions[input_count + hidden_count..total_nodes] {
            assert_eq!(*region, brain::RegionId::Central);
        }
        // Every hidden node has exactly one Ganglion candidate, so all must
        // anchor to it.
        for region in &regions[input_count..input_count + hidden_count] {
            assert_eq!(*region, brain::RegionId::Ganglion(10));
        }
    }

    /// N1c's second named test requirement: with two Ganglion segments,
    /// hidden nodes must split between them correctly by graph distance —
    /// nodes whose target position is nearer the head-side Ganglion get
    /// anchored there, and nodes nearer the tail-side Ganglion get anchored
    /// to that one instead.
    #[test]
    fn assign_hidden_node_regions_splits_hidden_nodes_between_two_ganglia_by_distance() {
        use genetics::{DevelopmentalOutputs, SegmentType};
        let outputs = |t: SegmentType| DevelopmentalOutputs {
            segment_type: t,
            branches: false,
            actuation_amplitude: 0.0,
            actuation_phase: 0.0,
            pigment: [0.5, 0.5, 0.5],
            apoptosis: false,
        };
        let mut graph = DevelopmentalGraph::new();
        let mut last = graph.push(
            SegmentType::Head,
            outputs(SegmentType::Head),
            None,
            false,
            0,
            None,
        );
        for pos in 1..crate::MAX_SEGMENTS {
            let role = if pos == 2 || pos == 12 {
                SegmentType::Ganglion
            } else {
                SegmentType::Torso
            };
            last = graph.push(role, outputs(role), Some(last), false, pos, None);
        }
        let _ = last;

        let input_count = 9;
        let hidden_count = 4;
        let output_count = 2;
        let total_nodes = input_count + hidden_count + output_count;

        let regions = assign_hidden_node_regions(&graph, input_count, hidden_count, total_nodes);
        let hidden_regions = &regions[input_count..input_count + hidden_count];

        // Hidden index 0 targets position 0 (nearest the head-side Ganglion
        // at 2); hidden index 3 targets position 3*15/4=11 (nearest the
        // tail-side Ganglion at 12). Both real Ganglia must actually be
        // used by *some* hidden node — proving this is a real split, not
        // every node coincidentally picking the same one.
        assert!(hidden_regions.contains(&brain::RegionId::Ganglion(2)));
        assert!(hidden_regions.contains(&brain::RegionId::Ganglion(12)));
    }
}
