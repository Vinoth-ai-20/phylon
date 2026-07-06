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
//! of inline match arms, so the mapping from a decoded [`SegmentType`] to
//! physics parameters is independently testable and reusable by future
//! research panels (HOX Visualizer, Development Timeline) without
//! re-deriving it from scratch.
//!
//! Per ADR-P3-04, whether the graph itself ever needs to be *retained*
//! past an organism's growth (for development replay) is the Development
//! Timeline milestone's own decision, not this one's — `growth_system`
//! currently drops `GrowthState` (and, with it, its `graph`) the moment
//! growth completes.

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
    pub fn push(
        &mut self,
        role: SegmentType,
        outputs: DevelopmentalOutputs,
        parent: Option<usize>,
        is_branch: bool,
    ) -> usize {
        self.nodes.push(DevelopmentalNode {
            role,
            outputs,
            parent,
            is_branch,
        });
        self.nodes.len() - 1
    }
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
/// `growth_system` needs to spawn its `ParticleNode`/`Spring`. Vascular,
/// Ganglion, and Germinal (Phase 3 M5) share Torso's stiffness — a neutral
/// default, not a designed value; their differentiated physics is DEF-003/
/// DEF-002, deferred to M8/M9, not this milestone's.
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
        SegmentType::Torso
        | SegmentType::Vascular
        | SegmentType::Ganglion
        | SegmentType::Germinal => 15.0,
        SegmentType::Muscle => 8.0,
        SegmentType::Tail => 2.0,
        SegmentType::Fin => 5.0,
    };

    let constraint_type = match role {
        SegmentType::Muscle => physics::ConstraintType::Elastic,
        SegmentType::Tail => physics::ConstraintType::Passive,
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
        }
    }

    #[test]
    fn push_returns_sequential_indices_and_records_parent() {
        let mut graph = DevelopmentalGraph::new();
        let head = graph.push(SegmentType::Head, sample_outputs(SegmentType::Head), None, false);
        let torso = graph.push(
            SegmentType::Torso,
            sample_outputs(SegmentType::Torso),
            Some(head),
            false,
        );
        assert_eq!(head, 0);
        assert_eq!(torso, 1);
        assert_eq!(graph.nodes[torso].parent, Some(head));
        assert!(!graph.nodes[torso].is_branch);
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
}
