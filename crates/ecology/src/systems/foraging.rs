use crate::components::{Corpse, Diet, Eaten, FoodPellet, MineralPellet, ResourceSpatialGrids};
use bevy_ecs::prelude::*;

/// Grid cell size for the broad-phase indices built each tick. Only affects
/// bucket occupancy, not correctness — final eat/predation checks still do
/// an exact distance comparison.
const FORAGING_CELL_SIZE: f32 = 50.0;

/// # Predation and Biomass Transfer System
///
/// ## 1. What Happens
/// The `foraging_system` handles all collision-based consumption in the ecosystem. It evaluates
/// interactions between organisms based on their `Diet` components (Carnivore, Herbivore, Decomposer)
/// and transfers `Glucose` from the prey (or food pellet/corpse) to the predator upon spatial overlap.
///
/// ## 2. Why It Happens
/// An ecosystem requires a flow of energy. Without predation, primary producers (plants) would simply
/// replicate until the `max_organisms` limit was reached, creating a static, dead simulation.
/// Predation introduces Lotka-Volterra population dynamics and creates evolutionary selection pressures
/// for speed, armor, and vision.
///
/// ## 3. How It Happens
/// Broad-phase candidates come from a per-tick spatial grid keyed on each organism's core-entity
/// position (replacing the previous full $O(N^2)$ pairwise scan); the exact minimum inter-segment
/// distance check between a predator node $P_1$ and prey node $P_2$ still gates the interaction:
///
/// $$ | \vec{P_1} - \vec{P_2} | \le R $$
///
/// The prey is marked `Eaten` and its total caloric value is added to the predator's glucose pool,
/// clamped to the predator's maximum stomach capacity:
///
/// $$ G_{predator} = \min(G_{predator} + G_{prey} + ATP_{prey}, G_{max}) $$
///
/// **Phase 5, SX-2c:** the moment of a successful organism-vs-organism meal
/// (predation or herbivory — Phase 1 below) spawns a `TimedEffects`
/// floating-text burst at the *eater's* position, the same trigger pattern
/// `corpse_decay_system`'s "Decomposed" burst already establishes in this
/// crate. Deliberately **not** extended to Phase 2 (pellet/mineral/corpse
/// grazing) — that happens routinely, every tick, for a large fraction of
/// the population, and would flood the viewport the same way logging every
/// `BehaviorState` change would flood `NarrationLog` (an existing, deliberate
/// restraint this milestone extends rather than overrides). Organism-vs-
/// organism consumption is comparatively rare and narratively significant,
/// matching "attack," specifically — this is a real distinction, not an
/// arbitrary cut.
#[allow(clippy::too_many_arguments)]
pub fn foraging_system(
    mut commands: Commands,
    mut timed_effects: ResMut<events::TimedEffects>,
    atmosphere: Res<metabolism::GlobalAtmosphere>,
    mut organism_query: Query<(
        Entity,
        &mut metabolism::ChemicalEconomy,
        &Diet,
        &physics::ParticleNode,
    )>,
    node_query: Query<&physics::ParticleNode>,
    food_query: Query<(Entity, &FoodPellet)>,
    mineral_query: Query<(Entity, &MineralPellet)>,
    corpse_query: Query<(Entity, &Corpse)>,
    resource_grids: Res<ResourceSpatialGrids>,
) {
    // Collect all nodes per organism to allow eating any segment
    let mut organism_nodes: std::collections::HashMap<u32, Vec<common::Vec3>> =
        std::collections::HashMap::new();
    for node in node_query.iter() {
        organism_nodes
            .entry(node.organism_id)
            .or_default()
            .push(node.position);
    }

    // Phase 1: Organism vs Organism predation.
    // Broad-phase via spatial grid (keyed on each organism's core-entity
    // position) replaces the previous O(N^2) `iter_combinations_mut` scan;
    // the exact minimum inter-segment distance check below is unchanged.
    let organism_eat_radius = 40.0;
    // Generous margin over the eat radius to account for body extent beyond
    // the core node before the exact per-segment distance check narrows it.
    let broadphase_radius = organism_eat_radius + 150.0;

    let mut core_entities: Vec<(Entity, common::Vec3)> = Vec::new();
    let mut organism_grid = spatial::UniformGrid::new(FORAGING_CELL_SIZE).unwrap();
    for (entity, _chem, _diet, node) in organism_query.iter() {
        core_entities.push((entity, node.position));
        let _ = organism_grid.insert(entity, node.position);
    }

    let mut processed_pairs: std::collections::HashSet<(Entity, Entity)> =
        std::collections::HashSet::new();

    for (e1, p1) in &core_entities {
        for e2 in organism_grid.query_radius(*p1, broadphase_radius) {
            if e2 == *e1 {
                continue;
            }
            let pair_key = if e1.index() < e2.index() {
                (*e1, e2)
            } else {
                (e2, *e1)
            };
            if !processed_pairs.insert(pair_key) {
                continue;
            }

            let Ok([(_, mut chem1, diet1, node1), (_, mut chem2, diet2, node2)]) =
                organism_query.get_many_mut([*e1, e2])
            else {
                continue;
            };

            if chem1.atp <= 0.0 || chem2.atp <= 0.0 {
                continue;
            }

            let mut dist = node1.position.distance(node2.position);

            if let Some(nodes2) = organism_nodes.get(&e2.index()) {
                for pos in nodes2 {
                    dist = dist.min(node1.position.distance(*pos));
                }
            }
            if let Some(nodes1) = organism_nodes.get(&e1.index()) {
                for pos in nodes1 {
                    dist = dist.min(node2.position.distance(*pos));
                }
            }

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

                // Phase 5, SX-2c: brief text (predation vs. herbivory read
                // differently) at a fixed duration, colored by the eater's
                // own `Diet::standard_color()` — never a new literal.
                const FEEDING_EFFECT_DURATION_TICKS: u64 = 45;
                let feeding_text = |eater: &Diet| -> &'static str {
                    if *eater == Diet::Carnivore {
                        "Hunted!"
                    } else {
                        "Grazed!"
                    }
                };

                if one_eats_two {
                    chem1.glucose =
                        (chem1.glucose + chem2.max_glucose + chem2.max_atp).min(chem1.max_glucose);
                    chem2.glucose = 0.0;
                    chem2.atp = 0.0;
                    if let Some(mut entity_cmds) = commands.get_entity(e2) {
                        entity_cmds.insert(Eaten);
                    }
                    timed_effects.spawn(
                        node1.position,
                        events::TimedEffectKind::FloatingText {
                            text: feeding_text(diet1).to_string(),
                            color: diet1.standard_color(),
                        },
                        atmosphere.ticks,
                        FEEDING_EFFECT_DURATION_TICKS,
                    );
                } else if two_eats_one {
                    chem2.glucose =
                        (chem2.glucose + chem1.max_glucose + chem1.max_atp).min(chem2.max_glucose);
                    chem1.glucose = 0.0;
                    chem1.atp = 0.0;
                    if let Some(mut entity_cmds) = commands.get_entity(*e1) {
                        entity_cmds.insert(Eaten);
                    }
                    timed_effects.spawn(
                        node2.position,
                        events::TimedEffectKind::FloatingText {
                            text: feeding_text(diet2).to_string(),
                            color: diet2.standard_color(),
                        },
                        atmosphere.ticks,
                        FEEDING_EFFECT_DURATION_TICKS,
                    );
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
                for mineral_entity in resource_grids
                    .minerals
                    .query_radius(node.position, eat_radius)
                {
                    if let Ok((_, mineral)) = mineral_query.get(mineral_entity) {
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
                for food_entity in resource_grids.food.query_radius(node.position, eat_radius) {
                    if let Ok((_, food)) = food_query.get(food_entity) {
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
                for corpse_entity in resource_grids
                    .corpses
                    .query_radius(node.position, eat_radius)
                {
                    if let Ok((_, corpse)) = corpse_query.get(corpse_entity) {
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
