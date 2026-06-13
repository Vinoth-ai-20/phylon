//! Core biological state and tags for entities in Phylon.

/// Tag component marking an entity as an alive organism.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Organism;

/// Tag component marking an entity as a food pellet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FoodPellet;

/// Tracks the energy reserve of an organism. If this reaches 0, health depletes.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Energy(pub f32);

/// Tracks the physical health of an organism. If this reaches 0, the organism dies.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Age(pub u64);
