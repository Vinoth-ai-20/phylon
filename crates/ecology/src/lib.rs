//! Food chain, predation, disease spread, fungi networks, and decomposition.

#![warn(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::prelude::*;
use common::Vec2;

use rand::Rng;
use serde::{Deserialize, Serialize};

/// Subsystem for random and manual environmental catastrophes.
pub mod catastrophe;

/// Pathogen infection state, spread, and progression.
pub mod disease;
pub use disease::{
    disease_progression_system, disease_spread_system, DiseaseConfig, Infection, InfectionState,
    SegmentImmunity, SegmentInfection,
};

/// Fungal (Decomposer) nutrient-redistribution network.
pub mod fungi;
pub use fungi::{fungal_network_system, FungalNetworkConfig};

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

impl Diet {
    /// The one canonical skin color for this diet, used everywhere an
    /// organism is spawned (sandbox tool and simulation-start seeding) so
    /// the same diet always looks the same regardless of spawn path.
    ///
    /// Values are linear-space RGB, gamma-decoded from the sRGB hex swatch
    /// noted in each comment (matching the convention already used by
    /// existing color literals in this codebase, e.g. `x_linear = (x_srgb/255)^2.2`).
    pub fn standard_color(&self) -> [f32; 3] {
        match self {
            Diet::Producer => [0.070, 0.437, 0.078],  // #4CAF50 green
            Diet::Herbivore => [0.065, 0.591, 0.776], // #48CAE4 blue
            Diet::Carnivore => [0.871, 0.089, 0.089], // #F05454 red
            // Phase 6, Epic J: was #FFB703 amber ([1.0, 0.482, 0.0]) —
            // `docs/design/accessibility.md`'s own Deuteranopia simulation
            // found Carnivore and Omnivore converge to a near-identical
            // yellow-olive under red-green color blindness. Measured (not
            // guessed) via a throwaway Machado et al. (2009) deuteranopia
            // simulation (`crates/ecology/examples/deuteranopia_check.rs`,
            // deleted after use): shifting toward orange/brown made the
            // collision *worse* (converges harder with red); shifting to a
            // fully saturated bright yellow measurably improved separation
            // from Carnivore (simulated-color distance +43%), Producer
            // (+35%), and Decomposer (+8%), at the cost of a smaller
            // reduction vs. Herbivore (-7%, still an enormous margin).
            Diet::Omnivore => [1.0, 0.737972, 0.0], // #FFDE00 bright yellow
            Diet::Decomposer => [0.334, 0.109, 0.789], // #9B5DE5 purple
        }
    }
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

/// Grid cell size for the broad-phase indices built each tick. Only affects
/// bucket occupancy, not correctness — final eat/predation checks still do
/// an exact distance comparison.
const FORAGING_CELL_SIZE: f32 = 50.0;

/// Spatial index over environmental resource pellets (food/minerals/
/// corpses), rebuilt once per tick by [`build_resource_grids_system`] and
/// shared by `sensing::sensing_system` and [`foraging_system`] so neither
/// has to independently rebuild the same 3 grids from the same underlying
/// data every tick.
#[derive(Resource)]
pub struct ResourceSpatialGrids {
    /// Broad-phase index over `FoodPellet` positions.
    pub food: spatial::UniformGrid,
    /// Broad-phase index over `MineralPellet` positions.
    pub minerals: spatial::UniformGrid,
    /// Broad-phase index over `Corpse` positions.
    pub corpses: spatial::UniformGrid,
}

impl ResourceSpatialGrids {
    /// Creates empty grids with the given cell size.
    pub fn new(cell_size: f32) -> Self {
        Self {
            food: spatial::UniformGrid::new(cell_size).unwrap(),
            minerals: spatial::UniformGrid::new(cell_size).unwrap(),
            corpses: spatial::UniformGrid::new(cell_size).unwrap(),
        }
    }
}

/// Rebuilds [`ResourceSpatialGrids`] from this tick's pellet positions. Must
/// run before both `sensing::sensing_system` and [`foraging_system`].
pub fn build_resource_grids_system(
    mut grids: ResMut<ResourceSpatialGrids>,
    food_query: Query<(Entity, &FoodPellet)>,
    mineral_query: Query<(Entity, &MineralPellet)>,
    corpse_query: Query<(Entity, &Corpse)>,
) {
    grids.food.clear();
    for (entity, food) in food_query.iter() {
        let _ = grids.food.insert(entity, food.position);
    }
    grids.minerals.clear();
    for (entity, mineral) in mineral_query.iter() {
        let _ = grids.minerals.insert(entity, mineral.position);
    }
    grids.corpses.clear();
    for (entity, corpse) in corpse_query.iter() {
        let _ = grids.corpses.insert(entity, corpse.position);
    }
}

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
/// file. Deliberately **not** extended to Phase 2 (pellet/mineral/corpse
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
    let mut organism_nodes: std::collections::HashMap<u32, Vec<common::Vec2>> =
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

    let mut core_entities: Vec<(Entity, common::Vec2)> = Vec::new();
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

/// # Autotrophic Energy Generation System
///
/// ## 1. What Happens
/// The `photosynthesis_system` allows organisms with the `Diet::Producer` trait to passively
/// convert ambient `GlobalAtmosphere.sunlight` and `GlobalAtmosphere.co2` directly into
/// structural `Glucose` and respired $O_2$.
///
/// ## 2. Why It Happens
/// The food web must have a foundational energy source. In Earth's biosphere, this is solar
/// irradiance. This system injects new biomass into the economy. However, to prevent runaway
/// infinite growth, the conversion is strictly bottlenecked by the availability of atmospheric $CO_2$.
///
/// ## 3. How It Happens
/// Every tick, a Producer requests a carbon volume proportional to its mass ($M$) and the
/// available sunlight ($S$):
///
/// $$ CO_{2_{req}} = 4.0 \times M \times S $$
///
/// To prevent a "Carbon Leak" where plants delete carbon by over-eating when full, the requested
/// $CO_2$ is clamped to the available space in the organism's glucose tank:
///
/// $$ \Delta CO_2 = \min(CO_{2_{req}}, G_{max} - G_{current}, CO_{2_{atmosphere}}) $$
///
/// The $\Delta CO_2$ is subtracted from the atmosphere, and the organism's glucose and $O_2$
/// are incremented by the same amount (a 1:1 simplified stoichiometric ratio).
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
            let mut co2_needed = 4.0 * metabolism.mass * sunlight;

            // Phase 3: Stop the Carbon Leak
            // Do not absorb CO2 if the Glucose tank is full, otherwise the carbon is deleted.
            let glucose_room = (chem.max_glucose - chem.glucose).max(0.0);
            co2_needed = co2_needed.min(glucose_room);

            let actual_co2 = atmosphere.co2.min(co2_needed);
            atmosphere.co2 -= actual_co2;

            // 1 CO2 -> 1 Glucose + 1 O2 (simplified). O2 output feeds back
            // into the shared atmosphere pool as well as the organism's own
            // tank, closing the loop with metabolism_system's O2 draw.
            chem.glucose = (chem.glucose + actual_co2).min(chem.max_glucose);
            chem.o2 = (chem.o2 + actual_co2).min(chem.max_o2);
            atmosphere.o2 += actual_co2;
        }
    }
}

/// # Corpse Decomposition & Outgassing System
///
/// ## 1. What Happens
/// The `corpse_decay_system` manages the biological decay of organisms that have died. When a
/// `Corpse` decays, it steadily outgasses $CO_2$ back into the `GlobalAtmosphere` over a set
/// duration. Once fully decayed, it despawns and leaves behind a `MineralPellet`.
///
/// ## 2. Why It Happens
/// This resolves the "Carbon Leak" tragedy-of-the-commons crisis. If organisms consume $CO_2$
/// to grow but delete that mass from the simulation upon death, the atmosphere would eventually
/// run out of carbon, halting all photosynthetic life. The outgassing models the gradual
/// respiration of invisible decomposer microbes breaking down structural carbon.
///
/// ## 3. How It Happens
/// Each tick, the system iterates over all entities with a `Corpse` component. The decay timer
/// is decremented, and the atmospheric outgassing accumulation is calculated per tick as:
///
/// $$ \Delta CO_{2} = \text{corpse.energy\_value} \times 0.0001 $$
///
/// Upon timer exhaustion ($t = 0$), the corpse undergoes complete mineralization. 50% of
/// the remaining energy is spawned as a `MineralPellet`, a 10% $\Delta CO_2$ burst is released,
/// and the `Corpse` entity is safely despawned.
pub fn corpse_decay_system(
    mut commands: Commands,
    mut atmosphere: ResMut<metabolism::GlobalAtmosphere>,
    mut corpse_query: Query<(Entity, &mut Corpse)>,
    mut timed_effects: ResMut<events::TimedEffects>,
) {
    // Phase 4, P4-V1: not biologically tuned, same placeholder status as
    // every other Phase 4 effect-duration constant.
    const DECOMPOSITION_EFFECT_DURATION_TICKS: u64 = 90;

    for (entity, mut corpse) in corpse_query.iter_mut() {
        if corpse.decay_timer > 0 {
            corpse.decay_timer -= 1;
            // Phase 3: Corpse Outgassing
            // Slowly release CO2 back into the atmosphere as the corpse decays.
            atmosphere.co2 += corpse.energy_value * 0.0001;
        } else {
            // Decay into mineral
            commands.spawn(MineralPellet {
                position: corpse.position,
                energy_value: corpse.energy_value * 0.5, // 50% energy lost to environment if not eaten directly
            });
            // Final burst of CO2 upon complete decay
            atmosphere.co2 += corpse.energy_value * 0.1;

            timed_effects.spawn(
                corpse.position,
                events::TimedEffectKind::FloatingText {
                    text: "Decomposed".to_string(),
                    color: [0.5, 0.4, 0.3],
                },
                atmosphere.ticks,
                DECOMPOSITION_EFFECT_DURATION_TICKS,
            );

            if let Some(mut e) = commands.get_entity(entity) {
                e.despawn();
            }
        }
    }
}

/// System that manages catastrophes, updates the hazard field, and drains energy from organisms in active hazards.
///
/// Phase 6, Epic A (re-audit finding, folded into this milestone rather than
/// deferred): this system previously used `Local<u64>` for `local_tick`, the
/// same anti-pattern SX-1a's diagnostic already named and fixed elsewhere —
/// the live app drives every system via `run_system_once` (a fresh
/// `SystemState` per call), so a `Local<u64>` silently reset to `0` on every
/// single tick, meaning `tick` here was **always `Tick(1)`**. Since hazard
/// lifecycle transitions are computed as `elapsed = tick - start_tick`, and
/// both sides of that subtraction were always `Tick(1)`, `elapsed` was
/// always `0` — hazards spawned into `Impending` state and then **never
/// transitioned to `Active` and never expired**, regardless of
/// `impending_duration`/`active_duration`. Fixed by reading
/// `metabolism::GlobalAtmosphere::ticks` (already the canonical live tick
/// counter this exact bug class was fixed with at SX-7a), which
/// `metabolism::day_night_cycle_system` increments earlier in the same
/// tick's system order (confirmed via `crates/app/src/simulation.rs`) — so
/// no new resource was introduced, just reuse of the one that already
/// exists.
#[allow(clippy::too_many_arguments)]
pub fn catastrophe_system(
    mut manager: ResMut<catastrophe::CatastropheManager>,
    config: Res<catastrophe::CatastropheConfig>,
    mut hazard_field: ResMut<diffusion::CpuHazardFieldState>,
    env: Res<environment::EnvironmentManager>,
    atmosphere: Res<metabolism::GlobalAtmosphere>,
    mut rng: ResMut<common::SimRng>,
    mut hazard_events: EventWriter<catastrophe::HazardSpawned>,
    mut organisms: Query<(
        &mut metabolism::ChemicalEconomy,
        &physics::ParticleNode,
        Option<&mut Corpse>,
    )>,
) {
    let tick = common::Tick(atmosphere.ticks);

    // Spawn random hazards
    if rng.gen::<f32>() < config.spawn_probability {
        let x = (rng.gen::<f32>() - 0.5) * env.width();
        let y = (rng.gen::<f32>() - 0.5) * env.height();
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

#[cfg(test)]
mod foraging_feeding_effect_tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;

    fn sample_chem(atp: f32, glucose: f32) -> metabolism::ChemicalEconomy {
        metabolism::ChemicalEconomy {
            glucose,
            o2: 0.0,
            co2: 0.0,
            atp,
            max_glucose: 1000.0,
            max_o2: 100.0,
            max_co2: 100.0,
            max_atp: 100.0,
        }
    }

    fn base_world() -> World {
        let mut world = World::new();
        world.insert_resource(events::TimedEffects::default());
        world.insert_resource(metabolism::GlobalAtmosphere::default());
        world.insert_resource(ResourceSpatialGrids::new(50.0));
        world
    }

    /// Phase 5, SX-2c: a successful organism-vs-organism predation should
    /// spawn a real `TimedEffects` burst at the predator's position — the
    /// gap this milestone closes (previously nothing marked the moment of
    /// the attack itself, only the prey's eventual death).
    #[test]
    fn predation_spawns_a_feeding_effect_at_the_predator_position() {
        let mut world = base_world();
        let predator_pos = common::Vec2::new(100.0, 100.0);
        world.spawn((
            physics::ParticleNode::new(predator_pos, 1.0, 0, 0),
            sample_chem(50.0, 0.0),
            Diet::Carnivore,
        ));
        world.spawn((
            physics::ParticleNode::new(predator_pos, 1.0, 0, 1),
            sample_chem(50.0, 10.0),
            Diet::Herbivore,
        ));

        world.run_system_once(foraging_system);

        let effects = &world.resource::<events::TimedEffects>().active;
        assert_eq!(effects.len(), 1);
        let events::TimedEffectKind::FloatingText { text, color } = &effects[0].kind;
        assert_eq!(text, "Hunted!");
        assert_eq!(*color, Diet::Carnivore.standard_color());
        assert_eq!(effects[0].position, predator_pos);
    }

    /// Herbivory (Herbivore-eats-Producer) is a distinct case with its own
    /// text, per `feeding_text`'s exhaustive-enough match.
    #[test]
    fn herbivory_spawns_a_grazed_effect_at_the_herbivore_position() {
        let mut world = base_world();
        let herbivore_pos = common::Vec2::new(-40.0, 20.0);
        world.spawn((
            physics::ParticleNode::new(herbivore_pos, 1.0, 0, 0),
            sample_chem(50.0, 0.0),
            Diet::Herbivore,
        ));
        world.spawn((
            physics::ParticleNode::new(herbivore_pos, 1.0, 0, 1),
            sample_chem(50.0, 10.0),
            Diet::Producer,
        ));

        world.run_system_once(foraging_system);

        let effects = &world.resource::<events::TimedEffects>().active;
        assert_eq!(effects.len(), 1);
        let events::TimedEffectKind::FloatingText { text, color } = &effects[0].kind;
        assert_eq!(text, "Grazed!");
        assert_eq!(*color, Diet::Herbivore.standard_color());
    }

    /// Two organisms too far apart to interact must not spawn any effect.
    #[test]
    fn no_effect_when_out_of_range() {
        let mut world = base_world();
        world.spawn((
            physics::ParticleNode::new(common::Vec2::new(0.0, 0.0), 1.0, 0, 0),
            sample_chem(50.0, 0.0),
            Diet::Carnivore,
        ));
        world.spawn((
            physics::ParticleNode::new(common::Vec2::new(1000.0, 1000.0), 1.0, 0, 1),
            sample_chem(50.0, 10.0),
            Diet::Herbivore,
        ));

        world.run_system_once(foraging_system);

        assert!(world.resource::<events::TimedEffects>().active.is_empty());
    }

    /// Phase 6, Epic A: `catastrophe_system` used to read a per-call
    /// `Local<u64>` tick counter that reset to `0` on every `run_system_once`
    /// invocation, so `elapsed = tick - start_tick` was always `0` regardless
    /// of how many real ticks had passed — a hazard could never reach
    /// `impending_duration` and would stay `Impending` forever. This proves
    /// the fix: a hazard whose `start_tick` is far enough in the past
    /// (measured via the real `GlobalAtmosphere::ticks` counter) must
    /// transition to `Active` the moment `catastrophe_system` runs.
    #[test]
    fn hazard_transitions_to_active_once_impending_duration_has_really_elapsed() {
        let mut world = World::new();
        world.insert_resource(common::SimRng::from_seed(1));
        world.insert_resource(metabolism::GlobalAtmosphere {
            ticks: 1000,
            ..Default::default()
        });
        world.insert_resource(environment::EnvironmentManager::new(1, false, 500.0, 500.0));
        world.insert_resource(diffusion::CpuHazardFieldState::default());
        world.insert_resource(bevy_ecs::event::Events::<catastrophe::HazardSpawned>::default());
        let config = catastrophe::CatastropheConfig {
            spawn_probability: 0.0, // don't let a second hazard spawn mid-test
            ..Default::default()
        };
        let impending_duration = config.impending_duration;
        world.insert_resource(config);
        let mut manager = catastrophe::CatastropheManager::default();
        manager.hazards.push(catastrophe::LocalHazard {
            center: common::Vec2::new(0.0, 0.0),
            state: catastrophe::HazardState::Impending {
                start_tick: common::Tick(1000 - impending_duration as u64),
            },
        });
        world.insert_resource(manager);

        world.run_system_once(catastrophe_system);

        let manager = world.resource::<catastrophe::CatastropheManager>();
        assert_eq!(manager.hazards.len(), 1);
        assert!(matches!(
            manager.hazards[0].state,
            catastrophe::HazardState::Active { .. }
        ));
    }

    /// Same fixed seed must produce the same hazard-spawn decision and
    /// position across two independent `World`s — proving the `fastrand`→
    /// `SimRng` migration preserved (rather than broke) this system's
    /// determinism guarantee.
    #[test]
    fn catastrophe_system_is_deterministic_for_a_given_seed() {
        fn run_once() -> Vec<common::Vec2> {
            let mut world = World::new();
            world.insert_resource(common::SimRng::from_seed(42));
            world.insert_resource(metabolism::GlobalAtmosphere::default());
            world.insert_resource(environment::EnvironmentManager::new(1, false, 500.0, 500.0));
            world.insert_resource(diffusion::CpuHazardFieldState::default());
            world.insert_resource(bevy_ecs::event::Events::<catastrophe::HazardSpawned>::default());
            world.insert_resource(catastrophe::CatastropheConfig {
                spawn_probability: 1.0, // always spawn, isolating the position draw
                ..Default::default()
            });
            world.insert_resource(catastrophe::CatastropheManager::default());

            world.run_system_once(catastrophe_system);

            world
                .resource::<catastrophe::CatastropheManager>()
                .hazards
                .iter()
                .map(|h| h.center)
                .collect()
        }

        assert_eq!(run_once(), run_once());
    }

    /// Same fixed seed must produce the same food-spawn decision (position,
    /// or consistent absence of one) across two independent `World`s —
    /// proving `food_spawner_system`'s `fastrand`→`SimRng` migration
    /// preserved determinism.
    #[test]
    fn food_spawner_system_is_deterministic_for_a_given_seed() {
        fn run_once() -> Vec<common::Vec2> {
            let mut world = World::new();
            world.insert_resource(common::SimRng::from_seed(7));
            world.insert_resource(metabolism::GlobalAtmosphere::default());
            world.insert_resource(environment::EnvironmentManager::new(1, false, 500.0, 500.0));
            world.insert_resource(EcologyConfig::default());

            world.run_system_once(food_spawner_system);

            let mut query = world.query::<&FoodPellet>();
            query.iter(&world).map(|p| p.position).collect()
        }

        assert_eq!(run_once(), run_once());
    }

    /// Phase 6, Epic J (Milestone J5): `Diet::Omnivore`'s color was changed
    /// specifically to increase separation from `Diet::Carnivore` under a
    /// Deuteranopia simulation (see `docs/design/accessibility.md`). This
    /// doesn't re-run the full colorblindness simulation (that measurement
    /// tool was a throwaway example, deleted after use, per this project's
    /// convention) — it's a cheap, permanent guard against silently
    /// reverting to the old amber value or picking a new one that's
    /// trivially identical to Carnivore in plain sRGB terms, which would
    /// undo this milestone's fix without any test catching it.
    #[test]
    fn omnivore_color_is_not_the_old_amber_and_stays_visibly_distinct_from_carnivore() {
        let omnivore = Diet::Omnivore.standard_color();
        let carnivore = Diet::Carnivore.standard_color();

        let old_amber = [1.0, 0.482, 0.0];
        assert_ne!(
            omnivore, old_amber,
            "Omnivore must not silently revert to the pre-Phase-6 amber that collided with Carnivore under deuteranopia"
        );

        let distance = ((omnivore[0] - carnivore[0]).powi(2)
            + (omnivore[1] - carnivore[1]).powi(2)
            + (omnivore[2] - carnivore[2]).powi(2))
        .sqrt();
        assert!(
            distance > 0.3,
            "Omnivore and Carnivore should read as clearly distinct colors in linear RGB; got distance {distance}"
        );
    }
}
