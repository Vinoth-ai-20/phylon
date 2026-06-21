//! Food chain, predation, disease spread, fungi networks, and decomposition.

#![warn(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::prelude::*;
use common::Vec2;
use metabolism::Energy;
use serde::{Deserialize, Serialize};

/// Subsystem for random and manual environmental catastrophes.
pub mod catastrophe;

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
    /// World position.
    pub position: Vec2,
    /// Energy provided when eaten.
    pub energy_value: f32,
}

/// An inorganic mineral nutrient in the environment.
#[derive(Component, Debug, Clone)]
pub struct MineralPellet {
    /// World position.
    pub position: Vec2,
    /// Energy provided when consumed by Producers.
    pub energy_value: f32,
}

/// A dead organism that can be decomposed.
#[derive(Component, Debug, Clone)]
pub struct Corpse {
    /// World position.
    pub position: Vec2,
    /// Total energy contained.
    pub energy_value: f32,
    /// Ticks until the corpse automatically decays into a mineral pellet.
    pub decay_timer: u32,
    /// Max decay ticks.
    pub max_decay: u32,
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
    env: Res<environment::EnvironmentManager>,
    query: Query<(), With<FoodPellet>>,
) {
    let current_count = query.iter().count();
    if current_count < config.max_food_pellets {
        // Simple rejection sampling to favor fertile biomes
        for _ in 0..10 {
            // Max 10 attempts per tick
            let x = (fastrand::f32() - 0.5) * env.width();
            let y = (fastrand::f32() - 0.5) * env.height();

            let biome = env.get_biome_at(x, y);
            let fertility = biome.fertility();

            // Rejection sampling: accept if random value is less than fertility
            if fastrand::f32() * 1.5 < fertility {
                commands.spawn(FoodPellet {
                    position: Vec2::new(x, y),
                    energy_value: 50.0,
                });
                break; // Successfully spawned 1 pellet
            }
        }
    }
}

/// System that handles collision eating based on ecological roles.
pub fn foraging_system(
    mut commands: Commands,
    mut organism_query: Query<(Entity, &mut Energy, &Diet, &physics::ParticleNode)>,
    food_query: Query<(Entity, &FoodPellet)>,
    mineral_query: Query<(Entity, &MineralPellet)>,
    corpse_query: Query<(Entity, &Corpse)>,
) {
    // We will do interactions by iterating organisms and then checking the environment queries.
    // For Carnivores eating Herbivores, we need to iterate pairs, which requires a nested query or `iter_combinations_mut`.
    // Since Bevy 0.14 `iter_combinations_mut` requires equal components.
    // We will leave Carnivore eating Herbivore for the physics system or handle it safely here.

    let eat_radius = 20.0;

    for (_entity, mut energy, diet, node) in organism_query.iter_mut() {
        match diet {
            Diet::Producer => {
                // Producers eat Minerals
                for (mineral_entity, mineral) in mineral_query.iter() {
                    if node.position.distance(mineral.position) <= eat_radius {
                        energy.current = (energy.current + mineral.energy_value).min(energy.max);
                        commands.entity(mineral_entity).despawn();
                        break;
                    }
                }
            }
            Diet::Herbivore | Diet::Omnivore => {
                // Herbivores eat FoodPellets
                for (food_entity, food) in food_query.iter() {
                    if node.position.distance(food.position) <= eat_radius {
                        energy.current = (energy.current + food.energy_value).min(energy.max);
                        commands.entity(food_entity).despawn();
                        break;
                    }
                }
            }
            Diet::Decomposer => {
                // Decomposers eat Corpses and spawn Minerals
                for (corpse_entity, corpse) in corpse_query.iter() {
                    if node.position.distance(corpse.position) <= eat_radius {
                        energy.current = (energy.current + corpse.energy_value).min(energy.max);
                        commands.entity(corpse_entity).despawn();

                        // Recycle into mineral
                        commands.spawn(MineralPellet {
                            position: corpse.position,
                            energy_value: corpse.energy_value * 0.8, // 80% recycled
                        });
                        break;
                    }
                }
            }
            Diet::Carnivore => {
                // To be implemented: Carnivore vs Herbivore predation
            }
        }
    }
}

/// System that decays Corpses into MineralPellets over time.
pub fn corpse_decay_system(mut commands: Commands, mut corpse_query: Query<(Entity, &mut Corpse)>) {
    for (entity, mut corpse) in corpse_query.iter_mut() {
        if corpse.decay_timer > 0 {
            corpse.decay_timer -= 1;
        } else {
            // Decay into mineral
            commands.spawn(MineralPellet {
                position: corpse.position,
                energy_value: corpse.energy_value * 0.5, // 50% energy lost to environment if not eaten directly
            });
            commands.entity(entity).despawn();
        }
    }
}

/// System that manages catastrophes, updates the hazard field, and drains energy from organisms in active hazards.
pub fn catastrophe_system(
    mut local_tick: Local<u64>,
    mut manager: ResMut<catastrophe::CatastropheManager>,
    config: Res<catastrophe::CatastropheConfig>,
    mut hazard_field: ResMut<diffusion::CpuHazardFieldState>,
    env: Res<environment::EnvironmentManager>,
    mut hazard_events: EventWriter<catastrophe::HazardSpawned>,
    mut organisms: Query<(&mut Energy, &physics::ParticleNode, Option<&mut Corpse>)>,
) {
    *local_tick += 1;
    let tick = common::Tick(*local_tick);

    // Spawn random hazards
    if fastrand::f32() < config.spawn_probability {
        let x = (fastrand::f32() - 0.5) * env.width();
        let y = (fastrand::f32() - 0.5) * env.height();
        manager.spawn_hazard(tick, Vec2::new(x, y));
        hazard_events.send(catastrophe::HazardSpawned(Vec2::new(x, y)));
    }

    hazard_field.clear();

    let mut active_hazards = Vec::new();

    // Update hazards and splat to field
    manager.hazards.retain_mut(|hazard| {
        match hazard.state {
            catastrophe::HazardState::Impending { start_tick } => {
                let elapsed = tick.0.saturating_sub(start_tick.0);
                if elapsed >= config.impending_duration as u64 {
                    hazard.state = catastrophe::HazardState::Active { start_tick: tick };
                    // splat with active severity
                    hazard_field.splat(hazard.center, config.hazard_radius, 1.0);
                    active_hazards.push((hazard.center, config.hazard_radius));
                } else {
                    // Splat impending severity (grows over time)
                    let severity = elapsed as f32 / config.impending_duration as f32;
                    hazard_field.splat(hazard.center, config.hazard_radius, severity);
                }
                true
            }
            catastrophe::HazardState::Active { start_tick } => {
                let elapsed = tick.0.saturating_sub(start_tick.0);
                if elapsed >= config.active_duration as u64 {
                    false // Remove hazard
                } else {
                    hazard_field.splat(hazard.center, config.hazard_radius, 1.0);
                    active_hazards.push((hazard.center, config.hazard_radius));
                    true
                }
            }
        }
    });

    // Apply energy drain to organisms in active hazards
    for (mut energy, node, mut corpse_opt) in organisms.iter_mut() {
        let mut in_hazard = false;
        for (center, radius) in &active_hazards {
            if node.position.distance(*center) <= *radius {
                in_hazard = true;
                break;
            }
        }

        if in_hazard {
            energy.current = (energy.current - config.energy_drain_rate).max(0.0);

            // If they died from catastrophe, maybe accelerate decay if they are already a corpse
            if let Some(corpse) = corpse_opt.as_mut() {
                corpse.energy_value = (corpse.energy_value - config.energy_drain_rate).max(0.0);
            }
        }
    }
}
