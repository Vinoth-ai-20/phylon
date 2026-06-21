//! Reproduction strategies, birth events, offspring dispersal, and malformed offspring handling.

#![warn(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::prelude::*;
use common::Vec2;
use genetics::Genome;
use metabolism::ChemicalEconomy;

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
    /// The parent entity, if any.
    pub parent_id: Option<bevy_ecs::entity::Entity>,
    /// The genome for the new child.
    pub genome: Genome,
    /// The position to spawn the child.
    pub position: Vec2,
    /// The diet inherited from the parent.
    pub diet: ecology::Diet,
    /// The ecological category inherited from the parent.
    pub category: ecology::EcologicalCategory,
}

/// System that handles reproduction (Asexual cloning and Sexual mating).
pub fn reproduction_system(
    mut query: Query<(
        Entity,
        &mut ChemicalEconomy,
        &mut ReproductionStrategy,
        &physics::ParticleNode,
        &ecology::Diet,
        &ecology::EcologicalCategory,
    )>,
    config: Res<ecology::EcologyConfig>,
    mut tracker: ResMut<genetics::GlobalInnovationTracker>,
    all_organisms: Query<(), With<ReproductionStrategy>>,
    mut birth_events: EventWriter<BirthRequest>,
) {
    let current_population = all_organisms.iter().count();
    let mut pending_births = 0;

    let mut ready_sexuals = Vec::new();

    // First pass: asexual + gather sexuals
    for (entity, mut chem, mut strategy, node, diet, category) in query.iter_mut() {
        if strategy.current_cooldown > 0 {
            strategy.current_cooldown -= 1;
        }

        // Apply Invasive species reproduction buff (e.g. 50% faster cooldown or cheaper cost)
        // Since cooldown is applied via tick reduction, we can reduce cost or threshold.
        // Let's just reduce the energy cost by 50% if Invasive.
        let mut actual_cost = strategy.energy_cost;
        let mut actual_threshold = strategy.energy_threshold;
        if *category == ecology::EcologicalCategory::Invasive {
            actual_cost *= 0.5;
            actual_threshold *= 0.5; // Also reproduce sooner
        }

        if chem.glucose >= actual_threshold
            && chem.atp >= actual_threshold
            && strategy.current_cooldown == 0
        {
            // Asexual clone
            if strategy.mode == ReproductionMode::Asexual {
                if current_population + pending_births >= config.max_organisms {
                    continue;
                }
                chem.glucose -= actual_cost;
                chem.atp -= actual_cost;
                strategy.current_cooldown = strategy.cooldown_ticks;

                let mut child_genome = strategy.genome.clone();
                // Introduce structural mutation
                child_genome.mutate(0.2, &mut rand::thread_rng(), &mut tracker);

                // Spawn child slightly offset
                let offset = Vec2::new(
                    (fastrand::f32() - 0.5) * 50.0,
                    (fastrand::f32() - 0.5) * 50.0,
                );

                birth_events.send(BirthRequest {
                    parent_id: Some(entity),
                    genome: child_genome,
                    position: node.position + offset,
                    diet: diet.clone(),
                    category: category.clone(),
                });

                pending_births += 1;
            } else if strategy.mode == ReproductionMode::Sexual {
                ready_sexuals.push((
                    entity,
                    node.position,
                    strategy.genome.clone(),
                    diet.clone(),
                    category.clone(),
                ));
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

            let (e1, p1, g1, d1, c1) = &ready_sexuals[i];
            let (e2, p2, g2, _d2, _c2) = &ready_sexuals[j];

            // Distance check (collision radius approx 50.0)
            if p1.distance(*p2) < 50.0 {
                // Compatibility check: roughly same node count
                if g1.brain_cppn.nodes.len() == g2.brain_cppn.nodes.len() {
                    if current_population + pending_births >= config.max_organisms {
                        break;
                    }

                    mated.insert(*e1);
                    mated.insert(*e2);

                    let mut rng = rand::thread_rng();
                    use rand::Rng;
                    let mut child_genome =
                        g1.crossover(g2, genetics::GenomeId(rng.gen()), &mut rng);
                    child_genome.mutate(0.1, &mut rng, &mut tracker);

                    let mut offset_pos = *p1;
                    offset_pos.x += (rng.gen::<f32>() - 0.5) * 50.0;
                    offset_pos.y += (rng.gen::<f32>() - 0.5) * 50.0;

                    birth_events.send(BirthRequest {
                        parent_id: Some(*e1),
                        genome: child_genome,
                        position: offset_pos,
                        diet: d1.clone(),
                        category: c1.clone(),
                    });
                    pending_births += 1;
                    break; // e1 has mated, move to next i
                }
            }
        }
    }

    // Deduct energy for those who mated sexually
    for (entity, mut chem, mut strategy, _, _, _) in query.iter_mut() {
        if mated.contains(&entity) {
            chem.glucose -= strategy.energy_cost;
            chem.atp -= strategy.energy_cost;
            strategy.current_cooldown = strategy.cooldown_ticks;
        }
    }
}
