use crate::components::{EcologyConfig, FoodPellet};
use bevy_ecs::prelude::*;
use common::Vec2;
use rand::Rng;

/// System that spawns food up to the hard cap.
///
/// Fertility is scaled by `atmosphere.season` (see
/// `metabolism::day_night_cycle_system`'s doc comment) — winter halves
/// effective fertility versus summer, so food spawn density itself has a
/// seasonal rhythm on top of each biome's fixed baseline.
pub fn food_spawner_system(
    mut commands: Commands,
    config: Res<EcologyConfig>,
    env: Res<environment::EnvironmentManager>,
    atmosphere: Res<metabolism::GlobalAtmosphere>,
    mut rng: ResMut<common::SimRng>,
    query: Query<(), With<FoodPellet>>,
) {
    let current_count = query.iter().count();
    if current_count < config.max_food_pellets {
        // Winter (season -> 0.0) halves fertility; summer (season -> 1.0)
        // leaves it unchanged.
        let season_fertility_factor = 0.5 + 0.5 * atmosphere.season;

        // Simple rejection sampling to favor fertile biomes
        for _ in 0..10 {
            // Max 10 attempts per tick
            let x = (rng.gen::<f32>() - 0.5) * env.width();
            let y = (rng.gen::<f32>() - 0.5) * env.height();

            let biome = env.get_biome_at(x, y);
            let fertility = biome.fertility() * season_fertility_factor;

            // Rejection sampling: accept if random value is less than fertility
            if rng.gen::<f32>() * 1.5 < fertility {
                commands.spawn(FoodPellet {
                    position: Vec2::new(x, y),
                    energy_value: 50.0,
                });
                break; // Successfully spawned 1 pellet
            }
        }
    }
}
