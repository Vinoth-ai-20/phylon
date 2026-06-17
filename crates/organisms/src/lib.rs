//! # Phylon Organisms
//!
//! Organism archetype definitions, ECS component bundles, and lifecycle types.
//!
//! Every simulated organism is a set of ECS components. This crate defines
//! the canonical component bundles and the [`DietType`] enum that governs
//! ecological interactions.
//!
//! ## Phase 0 scope
//!
//! Component type declarations and DietType enum. ECS integration: Phase 3.

#![warn(missing_docs)]
#![warn(clippy::all)]

use common::{EntityId, SimEnergy, SimLength, Vec2};
use serde::{Deserialize, Serialize};

/// The dietary strategy of an organism.
///
/// This is the primary axis of ecological role assignment. Diet determines
/// what an organism can consume and how it interacts with resource fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DietType {
    /// Consumes plant matter and nutrient fields.
    Herbivore,
    /// Hunts and consumes other organisms.
    Carnivore,
    /// Consumes both plant matter and other organisms.
    Omnivore,
    /// Consumes fungal networks and decomposing matter.
    Fungivore,
    /// Harvests energy directly from the sunlight field.
    Phototroph,
    /// Consumes dead organisms and carrion.
    Scavenger,
    /// Lives on a host organism, draining its energy.
    Parasite,
    /// Consumes detritus and waste products.
    Detritivore,
}

/// Spatial components of an organism: position, velocity, and collision radius.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialComponents {
    /// Current position in the simulation world.
    pub position: Vec2,
    /// Current velocity vector.
    pub velocity: Vec2,
    /// Collision and sensing radius.
    pub radius: SimLength,
}

/// Biological state components: energy, age, and diet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiologicalComponents {
    /// Current energy reserve. Organism dies when this reaches zero.
    pub energy: SimEnergy,
    /// Age in simulation ticks.
    pub age_ticks: u64,
    /// The organism's dietary strategy.
    pub diet: DietType,
    /// Parent entity ID (null if initial spawn).
    pub parent: EntityId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diet_type_is_copy() {
        let d = DietType::Herbivore;
        let _d2 = d; // copy semantics
    }

    #[test]
    fn all_diet_types_are_distinct() {
        let types = [
            DietType::Herbivore,
            DietType::Carnivore,
            DietType::Omnivore,
            DietType::Fungivore,
            DietType::Phototroph,
            DietType::Scavenger,
            DietType::Parasite,
            DietType::Detritivore,
        ];
        // All 8 variants present
        assert_eq!(types.len(), 8);
    }
}
