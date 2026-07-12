//! Ecology component/resource types shared by this crate's systems: diets,
//! consumable pellets/corpses, and the spatial grids used to find them
//! efficiently.

use bevy_ecs::prelude::*;
use common::Vec3;
use serde::{Deserialize, Serialize};

/// Indicates the diet of an organism.
#[derive(Component, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Diet {
    /// Autotrophs: generate energy from environment / minerals
    Producer,
    /// Eats plants/producers and food pellets.
    Herbivore,
    /// Eats other living organisms.
    Carnivore,
    /// Eats both plants and animals.
    Omnivore,
    /// Eats corpses, recycling them into minerals.
    Decomposer,
}

impl Diet {
    /// The one canonical skin color for this diet, used everywhere an
    /// organism is spawned (sandbox tool and simulation-start seeding) so
    /// the same diet always looks the same regardless of spawn path.
    ///
    /// Values are linear-space RGB, gamma-decoded from the sRGB hex swatch
    /// noted in each comment (matching the convention already used by
    /// existing color literals in this codebase, e.g. `x_linear = (x_srgb/255)^2.2`).
    pub fn standard_color(&self) -> [f32; 3] {
        match self {
            Diet::Producer => [0.070, 0.437, 0.078],  // #4CAF50 green
            Diet::Herbivore => [0.065, 0.591, 0.776], // #48CAE4 blue
            Diet::Carnivore => [0.871, 0.089, 0.089], // #F05454 red
            // A bright, fully saturated yellow rather than the more
            // intuitive amber/orange: `docs/design/accessibility.md`'s
            // Deuteranopia (red-green color blindness) simulation found
            // Carnivore and Omnivore converge to a near-identical
            // yellow-olive under that condition, and shifting toward
            // orange/brown makes the collision *worse* (it converges harder
            // with red). A fully saturated bright yellow measurably
            // improves separation from Carnivore (simulated-color distance
            // +43%), Producer (+35%), and Decomposer (+8%), at the cost of
            // a smaller (but still large) reduction in separation from
            // Herbivore (-7%).
            Diet::Omnivore => [1.0, 0.737972, 0.0], // #FFDE00 bright yellow
            Diet::Decomposer => [0.334, 0.109, 0.789], // #9B5DE5 purple
        }
    }
}

/// Identifies special ecological traits of an organism.
#[derive(Component, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EcologicalCategory {
    /// Default trait, no special category.
    None,
    /// Disproportionately important species.
    Keystone,
    /// Proxy for overall health.
    Indicator,
    /// Hyper-specialized to a niche.
    Endemic,
    /// Highly aggressive reproductive behavior.
    Invasive,
}

/// A food pellet in the environment (biomass).
#[derive(Component, Debug, Clone)]
pub struct FoodPellet {
    /// World position. Stored as `Vec3` for consistency with other
    /// positioned entities in the world, though the `z` component is
    /// currently always `0.0` (pellets live on a flat ground plane).
    pub position: Vec3,
    /// Energy provided when eaten.
    pub energy_value: f32,
}

/// An inorganic mineral nutrient in the environment.
#[derive(Component, Debug, Clone)]
pub struct MineralPellet {
    /// World position (see [`FoodPellet::position`] for the `Vec3`/`z` note).
    pub position: Vec3,
    /// Energy provided when consumed by Producers.
    pub energy_value: f32,
}

/// A dead organism that can be decomposed.
#[derive(Component, Debug, Clone)]
pub struct Corpse {
    /// World position (see [`FoodPellet::position`] for the `Vec3`/`z` note).
    pub position: Vec3,
    /// Total energy contained.
    pub energy_value: f32,
    /// Ticks until the corpse automatically decays into a mineral pellet.
    pub decay_timer: u32,
    /// Max decay ticks.
    pub max_decay: u32,
}

/// Marker component indicating an organism's biomass was entirely consumed by a predator.
#[derive(Component)]
pub struct Eaten;

/// Config for the food spawner.
#[derive(Resource, Debug, Clone)]
pub struct EcologyConfig {
    /// Max number of food pellets allowed in the world.
    pub max_food_pellets: usize,
    /// Max number of organisms allowed in the world.
    pub max_organisms: usize,
}

impl Default for EcologyConfig {
    fn default() -> Self {
        Self {
            max_food_pellets: 200,
            max_organisms: 50,
        }
    }
}

/// Spatial index over environmental resource pellets (food/minerals/
/// corpses), rebuilt once per tick by `systems::resource_grids::build_resource_grids_system`
/// and shared by `sensing::sensing_system` and `systems::foraging::foraging_system` so
/// neither has to independently rebuild the same 3 grids from the same
/// underlying data every tick.
#[derive(Resource)]
pub struct ResourceSpatialGrids {
    /// Broad-phase index over `FoodPellet` positions.
    pub food: spatial::UniformGrid,
    /// Broad-phase index over `MineralPellet` positions.
    pub minerals: spatial::UniformGrid,
    /// Broad-phase index over `Corpse` positions.
    pub corpses: spatial::UniformGrid,
}

impl ResourceSpatialGrids {
    /// Creates empty grids with the given cell size.
    pub fn new(cell_size: f32) -> Self {
        Self {
            food: spatial::UniformGrid::new(cell_size).unwrap(),
            minerals: spatial::UniformGrid::new(cell_size).unwrap(),
            corpses: spatial::UniformGrid::new(cell_size).unwrap(),
        }
    }
}
