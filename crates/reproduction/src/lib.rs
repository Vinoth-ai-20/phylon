use events::EventBus;
use events::PhylonEvent;
use genetics::{Genome, ReproductionMode};
use hecs::World;
use organisms::{Energy, Health};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use spatial::UniformGrid;

#[derive(Debug, Clone, PartialEq)]
pub struct ReproductionCooldown(pub u32);

pub fn process_reproduction(
    world: &mut World,
    grid: &UniformGrid,
    event_bus: &EventBus,
    rng_seed: u64,
    tick: u64,
) {
    let mut rng = ChaCha8Rng::seed_from_u64(rng_seed.wrapping_add(tick));

    let mut to_spawn = Vec::new();

    // Collect all capable of reproducing
    let mut candidates = Vec::new();
    for (entity, (energy, health, genome, pos, cooldown)) in world.query_mut::<(
        &mut Energy,
        &Health,
        &Genome,
        &physics::Position,
        &mut ReproductionCooldown,
    )>() {
        if cooldown.0 > 0 {
            cooldown.0 -= 1;
            continue;
        }

        // Must have sufficient energy and health
        if energy.0 >= 100.0 && health.0 > 50.0 {
            candidates.push((entity, genome.clone(), pos.0, energy.0));
        }
    }

    use rand::Rng;

    for (entity, genome, pos, _current_energy) in candidates {
        let is_sexual = match genome.reproduction_mode {
            ReproductionMode::Asexual => false,
            ReproductionMode::Sexual => true,
            ReproductionMode::Facultative { sexual_threshold } => {
                let local_density = grid.query_cell(grid.pos_to_cell(pos)).count() as f32;
                // High density -> sexual, low density -> asexual
                local_density > (sexual_threshold * 10.0)
            }
        };

        if is_sexual {
            let center_cell = grid.pos_to_cell(pos);
            let mut mate_genome = None;
            'outer: for dx in -1..=1 {
                for dy in -1..=1 {
                    let cell = common::IVec2::new(center_cell.x + dx, center_cell.y + dy);
                    for &nid in grid.query_cell(cell) {
                        let neighbor = hecs::Entity::from_bits(nid.0).unwrap();
                        if neighbor == entity {
                            continue;
                        }

                        if let Ok((n_energy, n_cooldown, n_genome)) =
                            world.query_one_mut::<(&Energy, &ReproductionCooldown, &Genome)>(
                                neighbor,
                            )
                        {
                            if n_cooldown.0 == 0 && n_energy.0 >= 100.0 {
                                mate_genome = Some(n_genome.clone());
                                break 'outer;
                            }
                        }
                    }
                }
            }

            if let Some(mate) = mate_genome {
                if let Ok(energy) = world.query_one_mut::<&mut Energy>(entity) {
                    energy.0 -= 80.0;
                }
                if let Ok(cooldown) = world.query_one_mut::<&mut ReproductionCooldown>(entity) {
                    cooldown.0 = 200;
                }

                let child_genome = genome.crossover(&mate, &mut rng, 0.05);
                to_spawn.push((
                    Some(common::EntityId(entity.to_bits().get())),
                    child_genome,
                    pos,
                    40.0,
                ));
            }
        } else {
            // Asexual
            if let Ok(energy) = world.query_one_mut::<&mut Energy>(entity) {
                energy.0 -= 60.0;
            }
            if let Ok(cooldown) = world.query_one_mut::<&mut ReproductionCooldown>(entity) {
                cooldown.0 = 150;
            }

            let child_genome = genome.mutate(&mut rng, 0.05);
            to_spawn.push((
                Some(common::EntityId(entity.to_bits().get())),
                child_genome,
                pos,
                30.0,
            ));
        }
    }

    for (parent, genome, pos, child_energy) in to_spawn {
        let jitter_x = (rng.gen::<f32>() - 0.5) * 5.0;
        let jitter_y = (rng.gen::<f32>() - 0.5) * 5.0;
        let spawn_pos = common::Vec2::new(pos.x + jitter_x, pos.y + jitter_y);

        event_bus.publish(PhylonEvent::BirthEvent {
            parent,
            genome,
            initial_energy: child_energy,
            position: spawn_pos,
        });
    }
}
