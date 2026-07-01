//! # Phylon Organisms
//!
//! Organism archetype definitions, ECS component bundles, and lifecycle types.
//!
//! Every simulated organism is a set of ECS components. This crate defines
//! the canonical component bundles and the `Diet` enum that governs
//! ecological interactions.
//!
//! ## Phase 0 scope
//!
//! Component type declarations and DietType enum. ECS integration: Phase 3.

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

/// Organism spawning logic.
pub mod spawning;
pub use spawning::spawn_organism;
