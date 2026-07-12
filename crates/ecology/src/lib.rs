//! # Ecology
//!
//! ## Purpose
//! "Ecology" in Phylon covers everything that governs how energy and matter
//! move through the simulated world *outside* an individual organism's own
//! body: where food comes from, how organisms consume it and each other,
//! how disease spreads between them, how dead organisms decompose, and how
//! localized environmental hazards ("catastrophes") affect the population.
//! An organism's internal chemistry (glucose/ATP/O2 pools, growth, aging)
//! lives in the `metabolism` crate; this crate is the world-level economy
//! that feeds and drains it.
//!
//! ## Architecture
//! The crate is organized as component/resource *types* (`components`,
//! [`disease`], [`fungi`], [`catastrophe`]) plus one system per ecological
//! process under `systems`:
//!
//! - [`build_resource_grids_system`] rebuilds the shared spatial indices
//!   ([`ResourceSpatialGrids`]) over food/mineral/corpse positions. It must
//!   run before any system that queries those grids.
//! - [`photosynthesis_system`] lets `Diet::Producer` organisms (simulated
//!   plants) convert atmospheric sunlight and CO2 into glucose — the
//!   primary energy source at the base of the food web.
//! - [`foraging_system`] resolves predation and grazing: an organism eating
//!   another organism, a food pellet, a mineral pellet, or a corpse,
//!   transferring energy from the eaten to the eater.
//! - [`fungal_network_system`] models a fungal network — a simulated
//!   decomposer system, analogous to real fungal mycelium, that draws
//!   energy out of corpses from a distance and redistributes part of it as
//!   fresh soil nutrients elsewhere, complementing `foraging_system`'s
//!   eat-on-contact decomposition.
//! - [`corpse_decay_system`] ages corpses over time, outgassing CO2 back to
//!   the atmosphere and eventually mineralizing what's left.
//! - [`disease_spread_system`] / [`disease_progression_system`] model
//!   pathogen transmission between nearby organisms and each infection's
//!   progression through incubation, infectious, and recovered states.
//! - [`catastrophe_system()`] spawns and evolves localized environmental
//!   hazards that drain energy from organisms caught inside them.
//!
//! Each tick, these systems run in the order above (grids first, so
//! everything downstream sees fresh spatial data), reading and writing
//! shared resources like `metabolism::GlobalAtmosphere` (the global
//! CO2/O2/sunlight pool) and each organism's `metabolism::ChemicalEconomy`.
//! No system spawns organisms directly; they only create/destroy resource
//! entities (food, minerals, corpses) and modify existing organisms'
//! chemistry.

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

/// Fungal (decomposer) nutrient-redistribution network.
pub mod fungi;
pub use fungi::{fungal_network_system, FungalNetworkConfig};

/// Component/resource types shared by this crate's systems.
mod components;
pub use components::{
    Corpse, Diet, Eaten, EcologicalCategory, EcologyConfig, FoodPellet, MineralPellet,
    ResourceSpatialGrids,
};

/// This crate's systems, one file per ecological process.
mod systems;
pub use systems::{
    build_resource_grids_system, catastrophe_system, corpse_decay_system, food_spawner_system,
    foraging_system, photosynthesis_system,
};

#[cfg(test)]
mod tests;
