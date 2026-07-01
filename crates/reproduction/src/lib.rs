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

/// # Ecological Reproduction Strategy
///
/// ## 1. What Happens
/// The `ReproductionStrategy` component manages the biological cost, threshold, and mode
/// (Asexual vs Sexual) of creating offspring. It stores the parent's `Genome` which will
/// be passed down or crossed over.
///
/// ## 2. Why It Happens
/// In ALife, infinite free reproduction causes exponential explosions that crash the engine.
/// Tying reproduction directly to the `ChemicalEconomy` ensures that populations are strictly
/// bottlenecked by the availability of environmental energy (Glucose/ATP).
///
/// ## 3. How It Happens
/// If $Glucose \ge \text{energy\_threshold}$ and $ATP \ge \text{energy\_threshold}$ and the
/// `current_cooldown` is $0$, the system deducts `energy_cost` and emits a `BirthRequest`.
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

/// # Offspring Spawning Event
///
/// ## 1. What Happens
/// `BirthRequest` is an asynchronous event requesting the engine to spawn a new organism.
///
/// ## 2. Why It Happens
/// Spawning a complex physics body with $N$ nodes and $M$ springs requires mutable access to
/// `Commands` and various structural components. Processing reproduction in the ECS update phase
/// but deferring the actual spawning to an Event Reader phase prevents system borrow conflicts.
///
/// ## 3. How It Happens
/// The `reproduction_system` writes the event. The `app` crate reads the event, increments the
/// `Generation` counter, and calls `spawn_organism` from the `organisms` crate.
#[derive(Message, Debug, Clone)]
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

/// # Population Replication System
///
/// ## 1. What Happens
/// The `reproduction_system` scans all organisms. If they meet their metabolic and cooldown
/// thresholds, it attempts to reproduce them. It supports both Asexual (clonal budding) and
/// Sexual (proximity-based crossover) mating.
///
/// ## 2. Why It Happens
/// This system drives the evolutionary loop. By enforcing proximity for Sexual mating, we
/// create spatial selection pressures (organisms must be good at finding mates). By tracking
/// global population caps, we prevent OOM crashes during periods of extreme resource abundance.
///
/// ## 3. How It Happens
/// The system runs in two passes:
/// 1. **Asexual Pass**: Checks thresholds. If Asexual, mutates the `Genome` structurally and
///    sends a `BirthRequest`. If Sexual, adds the organism to a `ready_sexuals` vector.
/// 2. **Sexual Pass**: Iterates over `ready_sexuals` looking for pairs where:
///    $$ \text{Distance}(A, B) < 50.0 $$
///    $$ A.BrainTopology \equiv B.BrainTopology $$
///    If matched, it performs NEAT crossover, mutates, and deducts the energy cost.
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
    mut birth_events: MessageWriter<BirthRequest>,
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

                birth_events.write(BirthRequest {
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

                    birth_events.write(BirthRequest {
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
