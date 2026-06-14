//! Ecological interactions for Phylon.

use common::Vec2;
use hecs::World;
use organisms::{Energy, FoodPellet, Organism};
use physics::{Position, Radius};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use spatial::UniformGrid;

/// Spawns new food pellets up to a certain cap.
pub fn spawn_food(world: &mut World, rng_seed: u64, tick: u64) {
    puffin::profile_function!();

    let max_food = 5000;
    let mut current_food = 0;
    for _ in world.query_mut::<&FoodPellet>() {
        current_food += 1;
    }

    if current_food < max_food {
        let mut rng = ChaCha8Rng::seed_from_u64(rng_seed.wrapping_add(tick));
        // Spawn a batch of food
        let to_spawn = (max_food - current_food).min(50); // Spawn max 50 per tick

        for _ in 0..to_spawn {
            let pos = Vec2::new(rng.gen_range(-500.0..500.0), rng.gen_range(-500.0..500.0));
            world.spawn((
                FoodPellet,
                Position(pos),
                Energy(10.0),
                Radius(1.0),
                // Note: Food pellets don't have velocity or mass right now.
                // They are static.
            ));
        }
    }
}

/// Allows organisms to consume food pellets via spatial proximity.
pub fn process_foraging(world: &mut World, _grid: &UniformGrid) {
    puffin::profile_function!();

    // First gather positions and energy of all food pellets
    let mut foods = Vec::new();
    for (entity, (_, pos, energy)) in world.query_mut::<(&FoodPellet, &Position, &Energy)>() {
        foods.push((entity, pos.0, energy.0, false)); // entity, pos, energy, is_eaten
    }

    // For each organism, check nearby food
    for (_org_entity, (_, org_pos, org_radius, org_energy)) in
        world.query_mut::<(&Organism, &Position, &Radius, &mut Energy)>()
    {
        let search_radius = org_radius.0 + 2.0;
        let search_sq = search_radius * search_radius;

        for food in &mut foods {
            if !food.3 {
                let dist_sq = (org_pos.0 - food.1).length_squared();
                if dist_sq < search_sq {
                    org_energy.0 += food.2;
                    food.3 = true; // Mark as eaten
                }
            }
        }
    }

    // Despawn consumed food
    for food in foods {
        if food.3 {
            let _ = world.despawn(food.0);
        }
    }
}

/// Processes gas exchange between organisms and the environment.
/// Organisms consume Oxygen (channel 0) and emit Carbon (channel 1).
pub fn process_gas_exchange(
    world: &mut World,
    field_grid: &mut [[f32; 4]],
    grid_width: u32,
    grid_height: u32,
) {
    puffin::profile_function!();

    let half_w = grid_width as f32 / 2.0;
    let half_h = grid_height as f32 / 2.0;

    for (_entity, (pos, _org, energy)) in world.query_mut::<(&Position, &Organism, &mut Energy)>() {
        let gx = (pos.0.x + half_w).floor() as i32;
        let gy = (pos.0.y + half_h).floor() as i32;

        if gx >= 0 && gx < grid_width as i32 && gy >= 0 && gy < grid_height as i32 {
            let idx = (gy as u32 * grid_width + gx as u32) as usize;

            // Consume Oxygen (index 0)
            let oxygen_available = field_grid[idx][0];
            let consume_rate = 0.05;
            let consumed = oxygen_available.min(consume_rate);

            field_grid[idx][0] -= consumed;

            // Emit Carbon (index 1) based on consumed oxygen and energy usage
            field_grid[idx][1] += consumed * 0.8;

            // Emit Scent (index 2)
            field_grid[idx][2] += 0.01;

            // Generate Heat (index 3)
            field_grid[idx][3] += 0.02;

            // Optional: If no oxygen, reduce energy or health
            if consumed < 0.01 {
                energy.0 -= 0.1; // Suffocation penalty
            }
        }
    }
}
