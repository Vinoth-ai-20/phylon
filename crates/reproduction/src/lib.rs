//! Reproduction strategies, birth events, offspring dispersal, and malformed offspring handling.

#![warn(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::prelude::*;
use common::Vec2;
use genetics::Genome;
use metabolism::Energy;

/// Reproduction mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReproductionMode {
    /// Clones itself.
    Asexual,
    /// Requires a mate.
    Sexual,
}

/// Component defining an organism's reproduction logic.
#[derive(Component, Debug, Clone)]
pub struct ReproductionStrategy {
    /// Energy required to trigger reproduction.
    pub energy_threshold: f32,
    /// Energy deducted from parent when reproducing.
    pub energy_cost: f32,
    /// Minimum ticks between reproduction events.
    pub cooldown_ticks: u64,
    /// Ticks since last reproduction.
    pub current_cooldown: u64,
    /// Mode of reproduction.
    pub mode: ReproductionMode,
    /// The genome of this organism (to pass to offspring).
    pub genome: Genome,
}

/// Event triggered when an organism successfully reproduces.
#[derive(Event, Debug, Clone)]
pub struct BirthRequest {
    /// The genome for the new child.
    pub genome: Genome,
    /// The position to spawn the child.
    pub position: Vec2,
}

/// System that handles reproduction (Asexual cloning and Sexual mating).
pub fn reproduction_system(
    mut query: Query<(
        Entity,
        &mut Energy,
        &mut ReproductionStrategy,
        &physics::ParticleNode,
    )>,
    config: Res<ecology::EcologyConfig>,
    all_organisms: Query<(), With<ReproductionStrategy>>,
    mut birth_events: EventWriter<BirthRequest>,
) {
    let current_population = all_organisms.iter().count();
    let mut pending_births = 0;

    let mut ready_sexuals = Vec::new();

    // First pass: asexual + gather sexuals
    for (entity, mut energy, mut strategy, node) in query.iter_mut() {
        if strategy.current_cooldown > 0 {
            strategy.current_cooldown -= 1;
            continue;
        }

        if energy.current >= strategy.energy_threshold {
            if strategy.mode == ReproductionMode::Asexual {
                if current_population + pending_births >= config.max_organisms {
                    continue;
                }
                energy.current -= strategy.energy_cost;
                strategy.current_cooldown = strategy.cooldown_ticks;

                let mut offset_pos = node.position;
                offset_pos.x += (fastrand::f32() - 0.5) * 100.0;
                offset_pos.y += (fastrand::f32() - 0.5) * 100.0;

                let mut child_genome = strategy.genome.clone();
                child_genome.mutate(0.05, &mut rand::thread_rng());

                birth_events.send(BirthRequest {
                    genome: child_genome,
                    position: offset_pos,
                });

                pending_births += 1;
            } else if strategy.mode == ReproductionMode::Sexual {
                ready_sexuals.push((entity, node.position, strategy.genome.clone()));
            }
        }
    }

    // Second pass: sexual mating (requires another organism)
    let mut mated = std::collections::HashSet::new();

    for i in 0..ready_sexuals.len() {
        if mated.contains(&ready_sexuals[i].0) {
            continue;
        }
        for j in (i + 1)..ready_sexuals.len() {
            if mated.contains(&ready_sexuals[j].0) {
                continue;
            }

            let (e1, p1, g1) = &ready_sexuals[i];
            let (e2, p2, g2) = &ready_sexuals[j];

            // Distance check (collision radius approx 50.0)
            if p1.distance(*p2) < 50.0 {
                // Compatibility check: exact segments match
                if g1.segments == g2.segments {
                    if current_population + pending_births >= config.max_organisms {
                        break;
                    }

                    mated.insert(*e1);
                    mated.insert(*e2);

                    let mut rng = rand::thread_rng();
                    use rand::Rng;
                    let mut child_genome =
                        g1.crossover(g2, genetics::GenomeId(rng.gen()), &mut rng);
                    child_genome.mutate(0.1, &mut rng);

                    let mut offset_pos = *p1;
                    offset_pos.x += (rng.gen::<f32>() - 0.5) * 50.0;
                    offset_pos.y += (rng.gen::<f32>() - 0.5) * 50.0;

                    birth_events.send(BirthRequest {
                        genome: child_genome,
                        position: offset_pos,
                    });
                    pending_births += 1;
                    break; // e1 has mated, move to next i
                }
            }
        }
    }

    // Deduct energy for those who mated sexually
    for (entity, mut energy, mut strategy, _) in query.iter_mut() {
        if mated.contains(&entity) {
            energy.current -= strategy.energy_cost;
            strategy.current_cooldown = strategy.cooldown_ticks;
        }
    }
}
