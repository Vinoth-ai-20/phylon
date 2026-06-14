//! Core biological state and tags for entities in Phylon.

use serde::{Deserialize, Serialize};

/// Tag component marking an entity as an alive organism.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Organism;

/// Tag component marking an entity as a food pellet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FoodPellet;

/// Tracks the energy reserve of an organism. If this reaches 0, health depletes.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Serialize, Deserialize)]
pub struct Energy(pub f32);

/// Tracks the physical health of an organism. If this reaches 0, the organism dies.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            current: 100.0,
            max: 100.0,
        }
    }
}

/// Tracks the age in ticks of an organism.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct Age(pub u64);
