//! Ecological interactions for Phylon.

use common::Vec2;
use events::EventBus;
use events::{DeathCause, PhylonEvent};
use genetics::{Diet, Genome};
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

/// Allows organisms to consume food pellets or hunt prey based on their Diet.
pub fn process_foraging(
    world: &mut World,
    grid: &UniformGrid,
    events: &EventBus,
    field_grid: &[[f32; 4]],
    grid_width: u32,
    grid_height: u32,
) {
    puffin::profile_function!();

    // We still collect food states because we need to mutate energy of foragers while checking food.
    let mut foods = rustc_hash::FxHashMap::default();
    for (entity, (_, pos, energy)) in world.query::<(&FoodPellet, &Position, &Energy)>().iter() {
        foods.insert(
            common::EntityId(entity.to_bits().get()),
            (entity, pos.0, energy.0, false),
        );
    }

    // Collect prey states for the same reason
    let mut preys = rustc_hash::FxHashMap::default();
    for (entity, (_, pos, genome, energy)) in world
        .query::<(&Organism, &Position, &Genome, &Energy)>()
        .iter()
    {
        preys.insert(
            common::EntityId(entity.to_bits().get()),
            (entity, pos.0, genome.size, energy.0, false),
        );
    }

    let mut deaths = Vec::new();
    let cell_size = grid.cell_size();

    // 3. Process each foraging organism
    for (org_entity, (_, pos, genome, radius, energy)) in
        world.query_mut::<(&Organism, &Position, &Genome, &Radius, &mut Energy)>()
    {
        let search_radius = radius.0 + 2.0;
        let search_sq = search_radius * search_radius;
        let org_entity_bits = org_entity.to_bits().get();

        let center_cell = grid.pos_to_cell(pos.0);
        let search_range = (search_radius / cell_size).ceil() as i32;

        match genome.diet {
            Diet::Herbivore => {
                for dx in -search_range..=search_range {
                    for dy in -search_range..=search_range {
                        let cell = common::IVec2::new(center_cell.x + dx, center_cell.y + dy);
                        for &neighbor_id in grid.query_cell(cell) {
                            if let Some(food) = foods.get_mut(&neighbor_id) {
                                if !food.3 {
                                    let dist_sq = (pos.0 - food.1).length_squared();
                                    if dist_sq < search_sq {
                                        energy.0 += food.2;
                                        food.3 = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Diet::Carnivore => {
                let mut ate_this_tick = false;
                for dx in -search_range..=search_range {
                    for dy in -search_range..=search_range {
                        let cell = common::IVec2::new(center_cell.x + dx, center_cell.y + dy);
                        for &neighbor_id in grid.query_cell(cell) {
                            if neighbor_id.0 == org_entity_bits || ate_this_tick {
                                continue;
                            }
                            if let Some(prey) = preys.get_mut(&neighbor_id) {
                                if !prey.4 && prey.2 < 0.85 * genome.size {
                                    let dist_sq = (pos.0 - prey.1).length_squared();
                                    if dist_sq < search_sq {
                                        energy.0 += prey.3;
                                        prey.4 = true;
                                        deaths.push(prey.0);
                                        ate_this_tick = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Diet::Omnivore => {
                for dx in -search_range..=search_range {
                    for dy in -search_range..=search_range {
                        let cell = common::IVec2::new(center_cell.x + dx, center_cell.y + dy);
                        for &neighbor_id in grid.query_cell(cell) {
                            if let Some(food) = foods.get_mut(&neighbor_id) {
                                if !food.3 {
                                    let dist_sq = (pos.0 - food.1).length_squared();
                                    if dist_sq < search_sq {
                                        energy.0 += food.2;
                                        food.3 = true;
                                    }
                                }
                            }
                        }
                    }
                }

                // Passive Carbon absorption
                let half_w = grid_width as f32 / 2.0;
                let half_h = grid_height as f32 / 2.0;
                let gx = (pos.0.x + half_w).floor() as i32;
                let gy = (pos.0.y + half_h).floor() as i32;

                if gx >= 0 && gx < grid_width as i32 && gy >= 0 && gy < grid_height as i32 {
                    let idx = (gy as u32 * grid_width + gx as u32) as usize;
                    let carbon_level = field_grid[idx][1]; // Carbon is index 1

                    // Pellet energy is ~10.0, so this gives fraction of a pellet.
                    let scavenge_energy = carbon_level * 0.3 * 10.0;
                    energy.0 += scavenge_energy;
                }
            }
        }
    }

    // 4. Despawn consumed food
    for food in foods.into_values() {
        if food.3 {
            let _ = world.despawn(food.0);
        }
    }

    // 5. Despawn consumed prey and emit events
    for dead_entity in deaths {
        let _ = world.despawn(dead_entity);
        events.publish(PhylonEvent::DeathEvent {
            id: common::EntityId(dead_entity.to_bits().get()),
            reason: DeathCause::Predation,
        });
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

pub fn process_disease(world: &mut World, grid: &spatial::UniformGrid, rng_seed: u64, tick: u64) {
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(rng_seed.wrapping_add(tick));

    let mut to_cure = Vec::new();
    let mut to_infect = Vec::new();

    for (entity, (health, disease, pos)) in world.query_mut::<(
        &mut organisms::Health,
        &mut organisms::Disease,
        &physics::Position,
    )>() {
        health.0 -= disease.virulence; // Decay health based on virulence
        if disease.remaining_duration > 0 {
            disease.remaining_duration -= 1;
        } else {
            to_cure.push(entity);
            continue;
        }

        // Spread to nearby organisms
        let center_cell = grid.pos_to_cell(pos.0);
        let cell_size = grid.cell_size();
        let search_range = (10.0 / cell_size).ceil() as i32;

        for dx in -search_range..=search_range {
            for dy in -search_range..=search_range {
                let cell = common::IVec2::new(center_cell.x + dx, center_cell.y + dy);
                for &neighbor_id in grid.query_cell(cell) {
                    let nid = hecs::Entity::from_bits(neighbor_id.0).unwrap();
                    if nid == entity {
                        continue;
                    }

                    use rand::Rng;
                    if rng.gen_bool((disease.virulence * 0.05).clamp(0.0, 1.0) as f64) {
                        to_infect.push((nid, disease.clone()));
                    }
                }
            }
        }
    }

    for entity in to_cure {
        let _ = world.remove_one::<organisms::Disease>(entity);
    }

    for (entity, disease) in to_infect {
        let _ = world.insert_one(entity, disease);
    }
}
