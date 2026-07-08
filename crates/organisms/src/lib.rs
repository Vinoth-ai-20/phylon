//! # Phylon Organisms
//!
//! Organism archetype definitions, ECS component bundles, and lifecycle types.
//!
//! Every simulated organism is a set of ECS components. This crate defines
//! the canonical component bundles, spawning/growth systems, and the
//! neuromodulator/Hebbian-plasticity systems that bridge `brain` with
//! `metabolism` each tick (see [`plasticity`]'s doc comments).

#![warn(missing_docs)]
#![warn(clippy::all)]

/// Fixed ceiling on how many body segments `growth_system` will grow for
/// any organism, and the `total_segments` denominator `genetics::develop`'s
/// positional decode uses (Phase 3, M4). Growth can still end earlier than
/// this if a decoded segment is `Tail` — see `growth_system`'s doc comment.
pub const MAX_SEGMENTS: usize = 15;

/// Tools for sandbox mode, presets, and procedural generation.
pub mod sandbox;
pub use sandbox::{PresetDefinition, SandboxTraits};

/// Organism components.
pub mod components;
pub use components::{
    BiologicalComponents, Generation, GrowthState, LifeStage, OrganismColor, SpatialComponents,
    SpawnTick,
};

/// The Body Graph (Phase 3, M6) — a persistent ECS component as of Phase 4,
/// `PHASE4_ROADMAP.md`'s ADR-P4-01 (superseding `PHASE3_ROADMAP.md`'s
/// ADR-P3-04, which made it transient).
pub mod developmental_graph;
pub use developmental_graph::{
    can_branch, compile_segment, simulate_growth_timeline, CompiledSegment, DevelopmentalGraph,
    DevelopmentalNode,
};

/// Organism ECS systems.
pub mod systems;
pub use systems::growth_system;

/// Neural plasticity systems: neuromodulator updates, Hebbian weight
/// adaptation, and periodic synapse pruning.
pub mod plasticity;
pub use plasticity::{hebbian_plasticity_system, neuromodulator_system};

/// Colonial/social coordination: flocking and pack (cooperative) hunting.
pub mod social;
pub use social::{flocking_system, pack_hunting_system, FlockingConfig, PackHuntingConfig};

/// Quorum sensing / biofilm density-scaling aggregation.
pub mod quorum;
pub use quorum::{biofilm_system, BiofilmConfig};

/// Organism spawning logic.
pub mod spawning;
pub use spawning::{spawn_organism, spawn_proto_fish};

/// Intra-body resource transport along the persistent Body Graph (Phase 4,
/// P4-F3).
pub mod transport;
pub use transport::transport_system;

/// Per-region endocrine signalling along the persistent Body Graph (Phase 4,
/// P4-F4).
pub mod endocrine;
pub use endocrine::endocrine_diffusion_system;

/// Per-segment immune response along the persistent Body Graph (Phase 4,
/// P4-F5).
pub mod immune;
pub use immune::segment_infection_system;

/// Intra-organism morphogen diffusion along the persistent Body Graph
/// (Phase 6, Epic D, milestone D1a).
pub mod morphogen_field;
pub use morphogen_field::{morphogen_diffusion_system, MorphogenLevel};

/// Life-stage transitions and re-entrant growth (Phase 4, P4-L1).
pub mod life_cycle;
pub use life_cycle::life_stage_system;
