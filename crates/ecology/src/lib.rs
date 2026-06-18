//! Food chain, predation, disease spread, fungi networks, and decomposition.

#![warn(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::prelude::*;
use common::Vec2;
use metabolism::Energy;

/// Indicates the diet of an organism.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub enum Diet {
    /// Eats food pellets or plants.
    Herbivore,
    /// Eats other organisms.
    Carnivore,
    /// Eats anything.
    Omnivore,
}

/// A food pellet in the environment.
#[derive(Component, Debug, Clone)]
pub struct FoodPellet {
    /// World position.
    pub position: Vec2,
    /// Energy provided when eaten.
    pub energy_value: f32,
}

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

/// System that spawns food up to the hard cap.
pub fn food_spawner_system(
    mut commands: Commands,
    config: Res<EcologyConfig>,
    query: Query<(), With<FoodPellet>>,
) {
    let current_count = query.iter().count();
    if current_count < config.max_food_pellets {
        // Spawn 1 pellet per tick if under cap
        let x = (fastrand::f32() - 0.5) * 800.0; // Assume 800x600 logical bounds for now
        let y = (fastrand::f32() - 0.5) * 600.0;

        commands.spawn(FoodPellet {
            position: Vec2::new(x, y),
            energy_value: 50.0,
        });
    }
}

/// System that handles passive collision eating.
///
/// For Phase 3, we just check distance between the organism's root/head node (which has `Energy` and `Diet`)
/// and the food pellets.
pub fn foraging_system(
    mut commands: Commands,
    mut organism_query: Query<(&mut Energy, &Diet, &physics::ParticleNode)>,
    food_query: Query<(Entity, &FoodPellet)>,
) {
    for (mut energy, diet, node) in organism_query.iter_mut() {
        if *diet != Diet::Herbivore && *diet != Diet::Omnivore {
            continue; // Carnivores don't eat pellets
        }

        let eat_radius = 20.0; // Interaction radius

        for (food_entity, food) in food_query.iter() {
            let dist = node.position.distance(food.position);
            if dist <= eat_radius {
                // Consume food
                energy.current += food.energy_value;
                if energy.current > energy.max {
                    energy.current = energy.max;
                }

                // Despawn the eaten food pellet
                commands.entity(food_entity).despawn();
                // We only eat one pellet per tick per organism to simplify
                break;
            }
        }
    }
}
