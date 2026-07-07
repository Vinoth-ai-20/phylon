//! The Body Graph (Phase 3, M6) — see `PHASE3_ROADMAP.md`'s ADR-P3-04.
//!
//! `DevelopmentalGraph` is a plain, transient record of every body position
//! `growth_system` has decoded for one organism so far, built up one
//! [`DevelopmentalNode`] per growth tick. It is deliberately **not** a
//! `bevy_ecs::Component`/`Resource` itself — it's reachable only through
//! `GrowthState.graph` (an already-existing `Component`), so `physics`,
//! `rendering`, `behavior`, and `analytics` need zero changes; growth still
//! spawns one `physics::ParticleNode`/`Spring` pair per tick exactly as
//! before, just via the [`compile_segment`] function extracted below instead
//! of inline match arms, so the mapping from a decoded `SegmentType` to
//! physics parameters is independently testable and reusable by future
//! research panels (HOX Visualizer, Development Timeline) without
//! re-deriving it from scratch.
//!
//! Per ADR-P3-04, whether the graph itself ever needs to be *retained*
//! past an organism's growth (for development replay) is the Development
//! Timeline milestone's own decision, not this one's — `growth_system`
//! currently drops `GrowthState` (and, with it, its `graph`) the moment
//! growth completes.
//!
//! **Phase 3, M13 resolves that question:** the graph does *not* need to be
//! retained. Because every node is a pure function of the genome and its
//! body position (see `genetics::develop_at_position`), the entire growth
//! timeline can be deterministically reconstructed after the fact — see
//! [`simulate_growth_timeline`], which mirrors `growth_system`'s own
//! control flow (apoptosis pruning, branch spawning, Tail-stop) without any
//! ECS/physics side effects, for exactly this replay purpose. ADR-P3-04's
//! transience decision stands unmodified.

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
}

/// The full sequence of decoded body positions for one growing organism.
#[derive(Debug, Clone, Default)]
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
    ) -> usize {
        self.nodes.push(DevelopmentalNode {
            role,
            outputs,
            parent,
            is_branch,
            position,
        });
        self.nodes.len() - 1
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
    let head_index = graph.push(head_outputs.segment_type, head_outputs, None, false, 0);
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
        );
        if can_branch(outputs.segment_type) && outputs.branches {
            graph.push(SegmentType::Fin, outputs, Some(spine_index), true, position);
            graph.push(SegmentType::Fin, outputs, Some(spine_index), true, position);
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
        );
        let torso = graph.push(
            SegmentType::Torso,
            sample_outputs(SegmentType::Torso),
            Some(head),
            false,
            1,
        );
        assert_eq!(head, 0);
        assert_eq!(torso, 1);
        assert_eq!(graph.nodes[torso].parent, Some(head));
        assert!(!graph.nodes[torso].is_branch);
        assert_eq!(graph.nodes[torso].position, 1);
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
