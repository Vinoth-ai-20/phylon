//! Food chain, predation, disease spread, fungi networks, and decomposition.

#![warn(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::prelude::*;
use common::Vec2;

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
    mut organism_query: Query<(
        Entity,
        &mut metabolism::ChemicalEconomy,
        &Diet,
        &physics::ParticleNode,
    )>,
    food_query: Query<(Entity, &FoodPellet)>,
    mineral_query: Query<(Entity, &MineralPellet)>,
    corpse_query: Query<(Entity, &Corpse)>,
) {
    // Phase 1: Organism vs Organism predation
    let organism_eat_radius = 40.0;
    let mut combos = organism_query.iter_combinations_mut();
    while let Some([(e1, mut chem1, diet1, node1), (e2, mut chem2, diet2, node2)]) =
        combos.fetch_next()
    {
        if chem1.atp <= 0.0 || chem2.atp <= 0.0 {
            continue;
        }

        let dist = node1.position.distance(node2.position);
        if dist <= organism_eat_radius {
            let one_eats_two = matches!(
                (diet1, diet2),
                (Diet::Carnivore, Diet::Herbivore | Diet::Omnivore)
                    | (Diet::Herbivore | Diet::Omnivore, Diet::Producer)
            );
            let two_eats_one = matches!(
                (diet2, diet1),
                (Diet::Carnivore, Diet::Herbivore | Diet::Omnivore)
                    | (Diet::Herbivore | Diet::Omnivore, Diet::Producer)
            );

            if one_eats_two {
                chem1.glucose =
                    (chem1.glucose + chem2.max_glucose + chem2.max_atp).min(chem1.max_glucose);
                chem2.glucose = 0.0;
                chem2.atp = 0.0;
                if let Some(mut entity_cmds) = commands.get_entity(e2) {
                    entity_cmds.insert(Eaten);
                }
            } else if two_eats_one {
                chem2.glucose =
                    (chem2.glucose + chem1.max_glucose + chem1.max_atp).min(chem2.max_glucose);
                chem1.glucose = 0.0;
                chem1.atp = 0.0;
                if let Some(mut entity_cmds) = commands.get_entity(e1) {
                    entity_cmds.insert(Eaten);
                }
            }
        }
    }

    // Phase 2: Organism vs Environment (Pellets, Minerals, Corpses)
    let eat_radius = 20.0;

    for (_entity, mut chem, diet, node) in organism_query.iter_mut() {
        if chem.atp <= 0.0 {
            continue;
        }

        match diet {
            Diet::Producer => {
                // Producers eat Minerals for structural growth
                for (mineral_entity, mineral) in mineral_query.iter() {
                    if node.position.distance(mineral.position) <= eat_radius {
                        chem.glucose = (chem.glucose + mineral.energy_value).min(chem.max_glucose);
                        if let Some(mut e) = commands.get_entity(mineral_entity) {
                            e.despawn();
                        }
                        break;
                    }
                }
            }
            Diet::Herbivore | Diet::Omnivore => {
                // Herbivores eat FoodPellets
                for (food_entity, food) in food_query.iter() {
                    if node.position.distance(food.position) <= eat_radius {
                        chem.glucose = (chem.glucose + food.energy_value).min(chem.max_glucose);
                        if let Some(mut e) = commands.get_entity(food_entity) {
                            e.despawn();
                        }
                        break;
                    }
                }
            }
            Diet::Decomposer => {
                // Decomposers eat Corpses and spawn Minerals
                for (corpse_entity, corpse) in corpse_query.iter() {
                    if node.position.distance(corpse.position) <= eat_radius {
                        chem.glucose = (chem.glucose + corpse.energy_value).min(chem.max_glucose);
                        if let Some(mut e) = commands.get_entity(corpse_entity) {
                            e.despawn();
                        }

                        // Recycle into mineral
                        commands.spawn(MineralPellet {
                            position: corpse.position,
                            energy_value: corpse.energy_value * 0.8, // 80% recycled
                        });
                        break;
                    }
                }
            }
            Diet::Carnivore => {}
        }
    }
}

/// System that handles photosynthesis for Producers.
pub fn photosynthesis_system(
    mut atmosphere: ResMut<metabolism::GlobalAtmosphere>,
    mut query: Query<(
        &Diet,
        &metabolism::Metabolism,
        &mut metabolism::ChemicalEconomy,
    )>,
) {
    let sunlight = atmosphere.sunlight;

    for (diet, metabolism, mut chem) in query.iter_mut() {
        if *diet == Diet::Producer && chem.atp > 0.0 {
            // Plants consume CO2 and Sunlight to make Glucose and O2
            let co2_needed = 4.0 * metabolism.mass * sunlight;

            if atmosphere.co2 >= co2_needed {
                atmosphere.co2 -= co2_needed;

                // 1 CO2 -> 1 Glucose + 1 O2 (simplified)
                chem.glucose = (chem.glucose + co2_needed).min(chem.max_glucose);
                chem.o2 = (chem.o2 + co2_needed).min(chem.max_o2);
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
            if let Some(mut e) = commands.get_entity(entity) {
                e.despawn();
            }
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
    mut organisms: Query<(
        &mut metabolism::ChemicalEconomy,
        &physics::ParticleNode,
        Option<&mut Corpse>,
    )>,
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
    for (mut chem, node, mut corpse_opt) in organisms.iter_mut() {
        let mut in_hazard = false;
        for (center, radius) in &active_hazards {
            if node.position.distance(*center) <= *radius {
                in_hazard = true;
                break;
            }
        }

        if in_hazard {
            chem.atp = (chem.atp - config.energy_drain_rate).max(0.0);

            // If they died from catastrophe, maybe accelerate decay if they are already a corpse
            if let Some(corpse) = corpse_opt.as_mut() {
                corpse.energy_value = (corpse.energy_value - config.energy_drain_rate).max(0.0);
            }
        }
    }
}
