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

/// Tools for sandbox mode, presets, and procedural generation.
pub mod sandbox;
pub use sandbox::{PresetDefinition, SandboxTraits};

/// Organism components.
pub mod components;
pub use components::{
    BiologicalComponents, Generation, GrowthState, OrganismColor, SpatialComponents, SpawnTick,
};

/// Organism ECS systems.
pub mod systems;
pub use systems::growth_system;

/// Neural plasticity systems: neuromodulator updates, Hebbian weight
/// adaptation, and periodic synapse pruning.
pub mod plasticity;
pub use plasticity::{hebbian_plasticity_system, neuromodulator_system};

/// Organism spawning logic.
pub mod spawning;
pub use spawning::{spawn_organism, spawn_proto_fish};
