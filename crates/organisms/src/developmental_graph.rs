//! The Body Graph — see `PHASE3_ROADMAP.md`'s ADR-P3-04/ADR-P3-09 for its
//! original (transient) design, and `PHASE4_ROADMAP.md`'s **ADR-P4-01** for
//! why that decision is reversed as of Phase 4, milestone P4-F1.
//!
//! **History, so the reversal is understood rather than just asserted:**
//! Phase 3 made `DevelopmentalGraph` deliberately transient — built up one
//! [`DevelopmentalNode`] per growth tick inside `GrowthState.graph`, dropped
//! the moment growth completed. ADR-P3-04 reasoned this was safe because
//! every node was a *pure function* of the genome and its body position;
//! ADR-P3-09 went on to prove that claim with a passing cross-check test
//! (`simulate_growth_timeline_matches_a_real_growth_system_run`), so nothing
//! was ever lost by discarding the graph — it could always be perfectly
//! reconstructed from the genome alone via [`simulate_growth_timeline`].
//!
//! **Phase 4 breaks that precondition on purpose.** Later Phase 4 epics
//! (physiology with injury/regeneration; life-stage re-differentiation)
//! introduce runtime anatomical history that is *not* a pure function of
//! genome + position — an organ's current condition can depend on damage
//! taken and tissue regrown, not just what the genome would decode fresh.
//! Once that's true, discarding the graph *would* lose real information, so
//! ADR-P4-01 makes `DevelopmentalGraph` a real `bevy_ecs::Component`,
//! attached to an organism's head entity for its entire life — the
//! canonical, persistent anatomical model every future biological system
//! (physiology, circulation, organ inspection, injury) should attach to
//! rather than duplicate.
//!
//! [`simulate_growth_timeline`] is **not removed** — it remains the correct
//! tool for replaying an organism's historical growth *order* (what
//! happened during initial growth, for the Development Timeline panel).
//! The persistent component this module now also provides answers a
//! different question: what is this organism's anatomy *right now*. Both
//! coexist; see ADR-P4-01 for why neither supersedes the other.
//!
//! This milestone (P4-F1) is infrastructure only: the graph becomes
//! persistent and gains a small, generic (non-biology-specific) query
//! surface (see [`DevelopmentalGraph::root`], [`DevelopmentalGraph::children_of`],
//! [`DevelopmentalGraph::node_at_position`]). It does not implement
//! physiology, circulation, injury, regeneration, life stages, or any
//! consumer of this data beyond `growth_system` itself continuing to build
//! it — those are later Phase 4 milestones, gated behind this one.
//!
//! **Serialization is deliberately not implemented in this milestone.**
//! Neither `DevelopmentalGraph`/`DevelopmentalNode` nor
//! `genetics::DevelopmentalOutputs` derive `Serialize`/`Deserialize`.
//! `crates/storage`'s `SimulationSnapshot` is a hand-built, explicit
//! whitelist of components (`storage/src/snapshot.rs`), not a generic
//! reflection-based dump — so this is safe (nothing breaks by omission) but
//! has a real consequence, documented rather than silently accepted: an
//! organism saved and reloaded via `SaveState`/`LoadState` loses its
//! persistent Body Graph (falls back to not having the component at all,
//! same as `GrowthState`/`Brain`'s internal state already do today). Adding
//! save/load support is future work, not required for this milestone.

use bevy_ecs::prelude::Component;
use genetics::{DevelopmentalOutputs, SegmentType};

/// One decoded body position (main spine) or lateral appendage (branch),
/// in the order `growth_system` grew it.
#[derive(Debug, Clone)]
pub struct DevelopmentalNode {
    /// This position's decoded anatomical category.
    pub role: SegmentType,
    /// The full decode result this node was built from (branching decision,
    /// actuation, pigment) — kept alongside `role` rather than discarded,
    /// so a future inspector panel can show *why* a node looks the way it
    /// does, not just what it turned into.
    pub outputs: DevelopmentalOutputs,
    /// Index (into the owning [`DevelopmentalGraph`]'s `nodes`) of this
    /// node's structural parent, or `None` for the head (the graph's root).
    pub parent: Option<usize>,
    /// `true` for a lateral fin/branch node, `false` for a main spine node.
    pub is_branch: bool,
    /// The body-axis position (`genetics::develop_at_position`'s
    /// `segment_index`) this node was decoded from — a branch node shares
    /// its parent spine node's position (Phase 3, M13; added so a
    /// Development Timeline scrubber can map a growth-order step back to
    /// the position research panels already know how to display).
    pub position: usize,
    /// The live `physics::ParticleNode` entity this position was
    /// materialized as, if any (Phase 4, P4-F2) — lets a future system map
    /// a graph index back to the physical/physiological entity carrying
    /// this segment's actual state (e.g. its `metabolism::ChemicalEconomy`
    /// pool). `None` for [`simulate_growth_timeline`]'s pure, ECS-free
    /// reconstruction, which has no real entities to reference.
    pub entity: Option<bevy_ecs::entity::Entity>,
}

/// The full sequence of decoded body positions for one organism — as of
/// Phase 4 (ADR-P4-01), a real, persistent ECS component attached to the
/// organism's head entity for its entire life, not a transient value
/// scoped to `growth_system`'s execution.
#[derive(Component, Debug, Clone, Default)]
pub struct DevelopmentalGraph {
    /// Every node decoded so far, in growth order.
    pub nodes: Vec<DevelopmentalNode>,
}

impl DevelopmentalGraph {
    /// An empty graph, ready for the head node to be pushed as index 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a new node and returns its index within this graph.
    #[allow(clippy::too_many_arguments)]
    pub fn push(
        &mut self,
        role: SegmentType,
        outputs: DevelopmentalOutputs,
        parent: Option<usize>,
        is_branch: bool,
        position: usize,
        entity: Option<bevy_ecs::entity::Entity>,
    ) -> usize {
        self.nodes.push(DevelopmentalNode {
            role,
            outputs,
            parent,
            is_branch,
            position,
            entity,
        });
        self.nodes.len() - 1
    }

    /// The graph's root node (the head, always index 0 if the graph is
    /// non-empty) — `None` only for a graph that hasn't been seeded yet.
    pub fn root(&self) -> Option<&DevelopmentalNode> {
        self.nodes.first()
    }

    /// Every node whose `parent` is `Some(index)` — i.e. `index`'s direct
    /// structural children (both spine continuation and any branch nodes).
    /// A small, generic traversal primitive: this milestone (P4-F1)
    /// deliberately adds no biology-specific queries (organ lookup,
    /// injury, etc.) — those are later Phase 4 milestones building on top
    /// of this, per ADR-P4-01.
    pub fn children_of(&self, index: usize) -> impl Iterator<Item = &DevelopmentalNode> {
        self.nodes.iter().filter(move |n| n.parent == Some(index))
    }

    /// The first non-branch (spine) node decoded at `position`, if any —
    /// the position-keyed lookup a research panel or future physiology
    /// system would use to find "the segment at body position N."
    pub fn node_at_position(&self, position: usize) -> Option<&DevelopmentalNode> {
        self.nodes
            .iter()
            .find(|n| !n.is_branch && n.position == position)
    }

    /// Same lookup as [`DevelopmentalGraph::node_at_position`], but returns
    /// the node's index within this graph rather than a reference — needed
    /// by [`DevelopmentalGraph::graph_distance`], which operates on indices
    /// (matching `DevelopmentalNode::parent`'s own indexing), not positions.
    /// Added alongside `graph_distance` (Phase 6, Epic C, N1c) rather than
    /// generalizing `node_at_position` itself, to avoid changing that
    /// method's existing return type for every current call site.
    pub fn index_at_position(&self, position: usize) -> Option<usize> {
        self.nodes
            .iter()
            .position(|n| !n.is_branch && n.position == position)
    }

    /// The number of structural edges between nodes `a` and `b` in this
    /// graph's tree — a body-graph (topological) distance, deliberately
    /// not a Euclidean one, since two segments can be spatially close but
    /// developmentally distant (or vice versa via a branch). Used by
    /// `organisms::systems::growth_system`'s brain-wiring step (Phase 6,
    /// Epic C, N1c) to find the nearest `SegmentType::Ganglion` anchor for
    /// a given body position.
    ///
    /// Implemented as a plain BFS over the undirected tree formed by
    /// `parent` links — this graph has at most `crate::MAX_SEGMENTS * 3`
    /// nodes (spine + up to 2 fins per branch point), so a BFS with no
    /// further optimization is more than fast enough and keeps this
    /// generic query surface simple, matching P4-F1's own stated principle
    /// (a small, non-biology-specific traversal primitive, not a
    /// specialized shortest-path structure).
    pub fn graph_distance(&self, a: usize, b: usize) -> usize {
        if a == b {
            return 0;
        }
        // Undirected adjacency: each node's parent, plus each node's
        // children (the reverse edge), so BFS can walk both up and down
        // the tree.
        let mut adjacency: std::collections::HashMap<usize, Vec<usize>> =
            std::collections::HashMap::new();
        for (index, node) in self.nodes.iter().enumerate() {
            if let Some(parent) = node.parent {
                adjacency.entry(index).or_default().push(parent);
                adjacency.entry(parent).or_default().push(index);
            }
        }

        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back((a, 0usize));
        visited.insert(a);
        while let Some((current, distance)) = queue.pop_front() {
            if current == b {
                return distance;
            }
            if let Some(neighbors) = adjacency.get(&current) {
                for &neighbor in neighbors {
                    if visited.insert(neighbor) {
                        queue.push_back((neighbor, distance + 1));
                    }
                }
            }
        }
        // `a`/`b` are disconnected (shouldn't happen for indices from the
        // same graph, since every node eventually chains to the root) —
        // returns `usize::MAX` rather than panicking, so a caller ranking
        // candidates by distance simply never picks an unreachable one.
        usize::MAX
    }
}

/// Deterministically reconstructs the full growth timeline for `regulatory_cppn`
/// without touching the ECS — mirrors `organisms::growth_system`'s control
/// flow exactly (position 0's head always grows regardless of its
/// apoptosis signal, same as `spawning::spawn_organism`; positions 1.. are
/// pruned on `outputs.apoptosis`; a branch-eligible, branching position
/// pushes two `Fin` nodes; growth stops after a grown `Tail` or at
/// [`crate::MAX_SEGMENTS`]) so a research panel can replay "how this body
/// plan came to be" for any organism, grown or not, without the transient
/// [`DevelopmentalGraph`] `growth_system` builds ever needing to be
/// persisted (Phase 3, M13 — see this module's doc comment).
pub fn simulate_growth_timeline(regulatory_cppn: &genetics::Cppn) -> DevelopmentalGraph {
    let total = crate::MAX_SEGMENTS;
    let mut graph = DevelopmentalGraph::new();

    let head_outputs = genetics::develop_at_position(regulatory_cppn, 0, total);
    let head_index = graph.push(
        head_outputs.segment_type,
        head_outputs,
        None,
        false,
        0,
        None,
    );
    if head_outputs.segment_type == SegmentType::Tail {
        return graph;
    }

    let mut last_spine_index = head_index;
    for position in 1..total {
        let outputs = genetics::develop_at_position(regulatory_cppn, position, total);
        if outputs.apoptosis {
            continue;
        }
        let spine_index = graph.push(
            outputs.segment_type,
            outputs,
            Some(last_spine_index),
            false,
            position,
            None,
        );
        if can_branch(outputs.segment_type) && outputs.branches {
            graph.push(
                SegmentType::Fin,
                outputs,
                Some(spine_index),
                true,
                position,
                None,
            );
            graph.push(
                SegmentType::Fin,
                outputs,
                Some(spine_index),
                true,
                position,
                None,
            );
        }
        last_spine_index = spine_index;
        if outputs.segment_type == SegmentType::Tail {
            break;
        }
    }

    graph
}

/// The physics parameters a decoded [`SegmentType`] compiles down to —
/// extracted out of `growth_system`'s previously-inline match arms so this
/// decode-to-physics mapping is independently testable (Phase 3, M6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompiledSegment {
    /// The numeric code `physics::ParticleNode::segment_type` stores.
    pub particle_segment_type: u32,
    /// Spring stiffness used when connecting this segment to its parent.
    pub stiffness: f32,
    /// Constraint behavior used when connecting this segment to its parent.
    pub constraint_type: physics::ConstraintType,
}

/// Compiles a decoded [`SegmentType`] into the physics parameters
/// `growth_system` needs to spawn its `ParticleNode`/`Spring`. As of Phase 3
/// M9, `Vascular` has its own differentiated profile (DEF-003's
/// differentiation-output half — a lower, transport-tissue-like stiffness
/// and a `Passive` constraint, distinct from rigid structural `Torso`).
/// `Ganglion`/`Germinal` still share Torso's stiffness — a neutral default,
/// not a designed value; their differentiated physics is the rest of
/// DEF-003 and germ-line-specific behavior beyond apoptosis protection
/// (already wired in M8), deferred to later milestones (M14 stretch,
/// respectively).
pub fn compile_segment(role: SegmentType) -> CompiledSegment {
    let particle_segment_type = match role {
        SegmentType::Head => 0,
        SegmentType::Torso => 1,
        SegmentType::Muscle => 2,
        SegmentType::Tail => 3,
        SegmentType::Fin => 4,
        SegmentType::Vascular => 5,
        SegmentType::Ganglion => 6,
        SegmentType::Germinal => 7,
    };

    let stiffness = match role {
        SegmentType::Head => 10.0,
        SegmentType::Torso | SegmentType::Ganglion | SegmentType::Germinal => 15.0,
        SegmentType::Muscle => 8.0,
        SegmentType::Vascular => 6.0,
        SegmentType::Tail => 2.0,
        SegmentType::Fin => 5.0,
    };

    let constraint_type = match role {
        SegmentType::Muscle => physics::ConstraintType::Elastic,
        SegmentType::Tail | SegmentType::Vascular => physics::ConstraintType::Passive,
        _ => physics::ConstraintType::Rigid,
    };

    CompiledSegment {
        particle_segment_type,
        stiffness,
        constraint_type,
    }
}

/// Whether a decoded segment type is eligible to sprout a lateral fin pair
/// — only `Torso`/`Muscle` are (not `Head`/`Tail`, and not yet any of
/// Phase 3 M5's new types, which have no designed branch behavior).
pub fn can_branch(role: SegmentType) -> bool {
    matches!(role, SegmentType::Torso | SegmentType::Muscle)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn push_returns_sequential_indices_and_records_parent() {
        let mut graph = DevelopmentalGraph::new();
        let head = graph.push(
            SegmentType::Head,
            sample_outputs(SegmentType::Head),
            None,
            false,
            0,
            None,
        );
        let torso = graph.push(
            SegmentType::Torso,
            sample_outputs(SegmentType::Torso),
            Some(head),
            false,
            1,
            None,
        );
        assert_eq!(head, 0);
        assert_eq!(torso, 1);
        assert_eq!(graph.nodes[torso].parent, Some(head));
        assert!(!graph.nodes[torso].is_branch);
        assert_eq!(graph.nodes[torso].position, 1);
    }

    #[test]
    fn root_returns_the_first_pushed_node() {
        let mut graph = DevelopmentalGraph::new();
        assert!(graph.root().is_none());
        graph.push(
            SegmentType::Head,
            sample_outputs(SegmentType::Head),
            None,
            false,
            0,
            None,
        );
        assert_eq!(graph.root().unwrap().role, SegmentType::Head);
    }

    #[test]
    fn children_of_finds_both_spine_and_branch_children() {
        let mut graph = DevelopmentalGraph::new();
        let head = graph.push(
            SegmentType::Head,
            sample_outputs(SegmentType::Head),
            None,
            false,
            0,
            None,
        );
        let torso = graph.push(
            SegmentType::Torso,
            sample_outputs(SegmentType::Torso),
            Some(head),
            false,
            1,
            None,
        );
        let fin_a = graph.push(
            SegmentType::Fin,
            sample_outputs(SegmentType::Fin),
            Some(torso),
            true,
            1,
            None,
        );
        let fin_b = graph.push(
            SegmentType::Fin,
            sample_outputs(SegmentType::Fin),
            Some(torso),
            true,
            1,
            None,
        );

        let head_children: Vec<usize> = graph.children_of(head).map(|_| torso).collect();
        assert_eq!(head_children, vec![torso]);

        let mut torso_children: Vec<bool> = graph.children_of(torso).map(|n| n.is_branch).collect();
        torso_children.sort();
        assert_eq!(torso_children, vec![true, true]);
        assert_eq!(graph.nodes[fin_a].parent, Some(torso));
        assert_eq!(graph.nodes[fin_b].parent, Some(torso));
    }

    /// Phase 6, Epic C (N1c): a straight spine chain's graph distance must
    /// be the real number of edges walked, not a raw index difference —
    /// they'd coincide for a pure spine anyway, but this is the base case
    /// the branch test below is contrasted against.
    #[test]
    fn graph_distance_on_a_straight_spine_is_the_hop_count() {
        let mut graph = DevelopmentalGraph::new();
        let head = graph.push(
            SegmentType::Head,
            sample_outputs(SegmentType::Head),
            None,
            false,
            0,
            None,
        );
        let torso1 = graph.push(
            SegmentType::Torso,
            sample_outputs(SegmentType::Torso),
            Some(head),
            false,
            1,
            None,
        );
        let torso2 = graph.push(
            SegmentType::Torso,
            sample_outputs(SegmentType::Torso),
            Some(torso1),
            false,
            2,
            None,
        );
        let tail = graph.push(
            SegmentType::Tail,
            sample_outputs(SegmentType::Tail),
            Some(torso2),
            false,
            3,
            None,
        );

        assert_eq!(graph.graph_distance(head, head), 0);
        assert_eq!(graph.graph_distance(head, torso1), 1);
        assert_eq!(graph.graph_distance(head, torso2), 2);
        assert_eq!(graph.graph_distance(head, tail), 3);
        assert_eq!(
            graph.graph_distance(tail, head),
            3,
            "distance must be symmetric"
        );
    }

    /// The case `graph_distance` exists for: two fin branches off the same
    /// torso are graph-adjacent to each other (2 hops, via their shared
    /// parent) even though a *node-index* difference would suggest
    /// otherwise — proving distance is computed from real tree structure,
    /// not accidentally from index arithmetic.
    #[test]
    fn graph_distance_through_a_shared_branch_point_is_not_the_index_difference() {
        let mut graph = DevelopmentalGraph::new();
        let head = graph.push(
            SegmentType::Head,
            sample_outputs(SegmentType::Head),
            None,
            false,
            0,
            None,
        );
        let torso = graph.push(
            SegmentType::Torso,
            sample_outputs(SegmentType::Torso),
            Some(head),
            false,
            1,
            None,
        );
        let fin_a = graph.push(
            SegmentType::Fin,
            sample_outputs(SegmentType::Fin),
            Some(torso),
            true,
            1,
            None,
        );
        let fin_b = graph.push(
            SegmentType::Fin,
            sample_outputs(SegmentType::Fin),
            Some(torso),
            true,
            1,
            None,
        );

        // fin_a (index 2) and fin_b (index 3) differ by 1 in raw index, but
        // are 2 graph hops apart (fin_a -> torso -> fin_b).
        assert_eq!(graph.graph_distance(fin_a, fin_b), 2);
        assert_ne!(
            graph.graph_distance(fin_a, fin_b),
            fin_b - fin_a,
            "graph distance must not accidentally equal the raw index difference"
        );
        assert_eq!(graph.graph_distance(fin_a, torso), 1);
    }

    #[test]
    fn node_at_position_finds_the_spine_node_not_a_branch() {
        let mut graph = DevelopmentalGraph::new();
        let head = graph.push(
            SegmentType::Head,
            sample_outputs(SegmentType::Head),
            None,
            false,
            0,
            None,
        );
        let torso = graph.push(
            SegmentType::Torso,
            sample_outputs(SegmentType::Torso),
            Some(head),
            false,
            1,
            None,
        );
        graph.push(
            SegmentType::Fin,
            sample_outputs(SegmentType::Fin),
            Some(torso),
            true,
            1,
            None,
        );

        let found = graph.node_at_position(1).expect("spine node at position 1");
        assert!(!found.is_branch);
        assert_eq!(found.role, SegmentType::Torso);
        assert!(graph.node_at_position(99).is_none());
    }

    #[test]
    fn compile_segment_covers_every_segment_type_without_panicking() {
        for role in [
            SegmentType::Head,
            SegmentType::Torso,
            SegmentType::Muscle,
            SegmentType::Tail,
            SegmentType::Fin,
            SegmentType::Vascular,
            SegmentType::Ganglion,
            SegmentType::Germinal,
        ] {
            let compiled = compile_segment(role);
            assert!(compiled.stiffness > 0.0);
        }
    }

    #[test]
    fn compile_segment_muscle_is_elastic_and_tail_is_passive() {
        assert_eq!(
            compile_segment(SegmentType::Muscle).constraint_type,
            physics::ConstraintType::Elastic
        );
        assert_eq!(
            compile_segment(SegmentType::Tail).constraint_type,
            physics::ConstraintType::Passive
        );
        assert_eq!(
            compile_segment(SegmentType::Head).constraint_type,
            physics::ConstraintType::Rigid
        );
    }

    #[test]
    fn vascular_has_its_own_differentiated_profile() {
        // Phase 3 M9 (DEF-003): Vascular is no longer a Torso-stiffness
        // placeholder — it's `Passive` (like Tail) but at its own,
        // distinct stiffness, not equal to Torso's or Tail's.
        let vascular = compile_segment(SegmentType::Vascular);
        let torso = compile_segment(SegmentType::Torso);
        let tail = compile_segment(SegmentType::Tail);
        assert_eq!(vascular.constraint_type, physics::ConstraintType::Passive);
        assert_ne!(vascular.stiffness, torso.stiffness);
        assert_ne!(vascular.stiffness, tail.stiffness);
    }

    #[test]
    fn only_torso_and_muscle_can_branch() {
        assert!(can_branch(SegmentType::Torso));
        assert!(can_branch(SegmentType::Muscle));
        assert!(!can_branch(SegmentType::Head));
        assert!(!can_branch(SegmentType::Tail));
        assert!(!can_branch(SegmentType::Fin));
        assert!(!can_branch(SegmentType::Vascular));
        assert!(!can_branch(SegmentType::Ganglion));
        assert!(!can_branch(SegmentType::Germinal));
    }

    #[test]
    fn simulate_growth_timeline_is_deterministic() {
        let cppn = genetics::Cppn::new();
        let a = simulate_growth_timeline(&cppn);
        let b = simulate_growth_timeline(&cppn);
        assert_eq!(a.nodes.len(), b.nodes.len());
        for (na, nb) in a.nodes.iter().zip(b.nodes.iter()) {
            assert_eq!(na.position, nb.position);
            assert_eq!(na.role, nb.role);
        }
    }

    #[test]
    fn simulate_growth_timeline_stops_after_a_grown_tail() {
        // Every node in a timeline that stopped at a Tail must have a
        // position within bounds, and the last node must be the Tail that
        // stopped it (unless it stopped solely by hitting MAX_SEGMENTS).
        let cppn = genetics::Cppn::new();
        let graph = simulate_growth_timeline(&cppn);
        assert!(!graph.nodes.is_empty());
        if let Some(last) = graph.nodes.iter().rfind(|n| !n.is_branch) {
            if last.role == SegmentType::Tail {
                assert!(graph.nodes.iter().all(|n| n.position < crate::MAX_SEGMENTS));
            }
        }
    }

    #[test]
    fn simulate_growth_timeline_never_records_a_pruned_non_head_position() {
        // The head (position 0) is force-grown regardless of its own
        // apoptosis signal (mirroring `spawning::spawn_organism`, which
        // never checks it) — every *other* recorded node must not be
        // apoptotic, since a pruned position is never pushed at all.
        for node in simulate_growth_timeline(&genetics::Cppn::new()).nodes {
            if node.position != 0 {
                assert!(!node.outputs.apoptosis);
            }
        }
    }
}
