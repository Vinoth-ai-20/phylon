//! Food chain, predation, disease spread, fungi networks, and decomposition.

#![warn(missing_docs)]
#![warn(clippy::all)]

/// Subsystem for random and manual environmental catastrophes.
pub mod catastrophe;

/// Pathogen infection state, spread, and progression.
pub mod disease;
pub use disease::{
    disease_progression_system, disease_spread_system, DiseaseConfig, Infection, InfectionState,
    SegmentImmunity, SegmentInfection,
};

/// Fungal (Decomposer) nutrient-redistribution network.
pub mod fungi;
pub use fungi::{fungal_network_system, FungalNetworkConfig};

/// Component/resource types shared by this crate's systems (Phase 7, W5d —
/// extracted from this file, which previously held both types and systems
/// inline).
mod components;
pub use components::{
    Corpse, Diet, Eaten, EcologicalCategory, EcologyConfig, FoodPellet, MineralPellet,
    ResourceSpatialGrids,
};

/// This crate's 6 systems, one file per system (Phase 7, W5d).
mod systems;
pub use systems::{
    build_resource_grids_system, catastrophe_system, corpse_decay_system, food_spawner_system,
    foraging_system, photosynthesis_system,
};

#[cfg(test)]
mod tests;
