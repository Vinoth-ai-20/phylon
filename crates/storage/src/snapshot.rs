#![allow(missing_docs)]

use serde::{Deserialize, Serialize};

/// Screen/UI-resolved 2D values that are genuinely 2D at the point they're
/// recorded — a hazard's world-space center (ADR-P8-05: the hazard field
/// stays a flat plane) or a replay-recorded spawn-click position
/// ([`crate::replay::ReplayAction`]) — never a live entity's world
/// position (see [`SerializedVec3`] for that).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SerializedVec2 {
    pub x: f32,
    pub y: f32,
}

impl From<common::Vec2> for SerializedVec2 {
    fn from(v: common::Vec2) -> Self {
        Self { x: v.x, y: v.y }
    }
}

impl From<SerializedVec2> for common::Vec2 {
    fn from(val: SerializedVec2) -> Self {
        common::Vec2::new(val.x, val.y)
    }
}

/// A live entity's world-space position/velocity (Phase 8, Epic 8.13,
/// ADR-P8-08) — replaces `SerializedVec2` for every field that used to
/// truncate a real `Vec3` down to 2D on save and re-extend it with `z =
/// 0.0` on restore. `SchemaVersion::CURRENT` bumped from 4 to 5 for this
/// change — see that constant's own doc comment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SerializedVec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<common::Vec3> for SerializedVec3 {
    fn from(v: common::Vec3) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

impl From<SerializedVec3> for common::Vec3 {
    fn from(val: SerializedVec3) -> Self {
        common::Vec3::new(val.x, val.y, val.z)
    }
}

/// A [`organisms::DevelopmentalNode`], with its `entity` field remapped to
/// the same stable `u64` id scheme [`SnapshotSpring`] already uses (raw
/// `Entity` references can't be serialized directly, and wouldn't be valid
/// across a restore anyway — Bevy assigns fresh `Entity` ids on every
/// respawn).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDevelopmentalNode {
    pub role: genetics::SegmentType,
    pub outputs: genetics::DevelopmentalOutputs,
    pub parent: Option<usize>,
    pub is_branch: bool,
    pub position: usize,
    pub entity_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotNode {
    pub id: u64, // Internal mapping ID
    pub position: SerializedVec3,
    pub velocity: SerializedVec3,
    pub mass: f32,
    pub segment_type: u32,
    pub is_fixed: bool,
    pub organism_id: u32,

    // Optional attributes per node
    pub color: Option<[f32; 3]>,
    pub diet: Option<ecology::Diet>,
    pub category: Option<ecology::EcologicalCategory>,

    // Only one node per organism needs to store the genome/brain
    pub genome: Option<genetics::Genome>,
    pub brain: Option<brain::Brain>,

    // Phase 6, Epic E — physiology/graph state that previously silently
    // vanished on save/load (see this module's `SimulationSnapshot` doc
    // comment's Epic E note). All per-segment; `Option` because most only
    // exist on the head (`chemical_economy`/`morphogen_level` are the
    // exception, carried by every segment).
    pub chemical_economy: Option<metabolism::ChemicalEconomy>,
    pub age: Option<metabolism::Age>,
    pub metabolism: Option<metabolism::Metabolism>,
    pub health: Option<metabolism::Health>,
    pub hydration: Option<metabolism::Hydration>,
    pub body_temperature: Option<metabolism::BodyTemperature>,
    pub generation: Option<organisms::Generation>,
    pub spawn_tick: Option<organisms::SpawnTick>,
    pub life_stage: Option<organisms::LifeStage>,
    pub morphogen_level: Option<organisms::MorphogenLevel>,
    pub hormone_level: Option<brain::HormoneLevel>,
    /// [`brain::Neuromodulators`]'s 3 public channels (dopamine, serotonin,
    /// noradrenaline), in that order. `Neuromodulators` itself isn't
    /// serialized directly — its private `last_atp` bookkeeping field has
    /// no public accessor — so `restore_world` reconstructs it via
    /// `Neuromodulators::new(restored_chemical_economy.atp)` and then
    /// overwrites these 3 channels, at the documented cost of `last_atp`
    /// itself being reseeded from the restored ATP rather than the exact
    /// pre-save value (a one-tick precision loss in the next dopamine-delta
    /// computation, not a correctness break).
    pub neuromodulator_channels: Option<[f32; 3]>,
    pub infection: Option<ecology::disease::Infection>,
    pub segment_infection: Option<ecology::disease::SegmentInfection>,
    pub segment_immunity: Option<ecology::disease::SegmentImmunity>,

    /// Present only on the organism's head entity — the persistent Body
    /// Graph (`organisms::DevelopmentalGraph`), with every node's `entity`
    /// remapped to a stable id (see [`SnapshotDevelopmentalNode`]).
    pub developmental_graph: Option<Vec<SnapshotDevelopmentalNode>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotSpring {
    pub node_a_id: u64,
    pub node_b_id: u64,
    pub constraint_type: physics::ConstraintType,
    pub rest_length: f32,
    pub base_length: f32,
    pub stiffness: f32,
    pub damping: f32,
    pub actuation_amplitude: f32,
    pub actuation_phase: f32,
    pub breaking_strain: f32,
    pub is_fin: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotFood {
    pub position: SerializedVec3,
    pub energy_value: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMineral {
    pub position: SerializedVec3,
    pub energy_value: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotCorpse {
    pub position: SerializedVec3,
    pub energy_value: f32,
    pub decay_timer: u32,
    pub max_decay: u32,
}

/// # Deterministic State Snapshot
///
/// ## 1. What Happens
/// `SimulationSnapshot` is a serializable representation of the entire ECS world state
/// at a specific tick.
///
/// ## 2. Why It Happens
/// Bevy's ECS does not natively support perfect serialization out of the box, especially
/// with pointers, Entity IDs, and GPU buffers. To guarantee deterministic restore, we must
/// extract the exact float positions, neural weights, and genomes, and map local Entity IDs
/// to a stable 64-bit format.
///
/// ## 3. How It Happens
/// `from_world` executes bulk queries over the ECS, packing `ParticleNode`, `Spring`, and
/// `FoodPellet` data into flat `Vec`s. `restore_world` clears the ECS completely and respawns
/// every entity, carefully rewriting `node_a` and `node_b` entity references in the `Spring`s
/// to match the new Bevy Entity IDs.
///
/// ## 4. Phase 6, Epic E note — physiology/graph coverage
/// Until this milestone, `from_world`/`restore_world` only ever round-tripped position/
/// velocity/mass/segment_type/color/diet/category/genome/brain — every other per-segment or
/// per-organism physiological component (`metabolism::ChemicalEconomy`/`Age`/`Metabolism`/
/// `Health`/`Hydration`/`BodyTemperature`, `brain::HormoneLevel`/`Neuromodulators`,
/// `ecology::disease::{Infection,SegmentInfection,SegmentImmunity}`,
/// `organisms::{Generation,SpawnTick,LifeStage,MorphogenLevel,DevelopmentalGraph}`) was silently
/// discarded on save/load — confirmed by none of those types deriving `Serialize` at all before
/// this milestone. In practice this meant a reloaded organism's `metabolism_system` query (which
/// requires `ChemicalEconomy`+`Age`+`Metabolism`) simply never matched it again: a "loaded"
/// organism looked identical visually but had silently stopped metabolizing entirely. This
/// milestone closes that gap by adding `Serialize`/`Deserialize` directly to each of those
/// (small, plain-data) component types in their own home crates — the same pattern this crate
/// already used for `organisms::{Generation,SpawnTick,LifeStage}`/`OrganismColor` — rather than
/// duplicating their shape into bespoke wrapper structs here, keeping this file's job to
/// entity-id remapping and optional-field plumbing, not field-by-field re-declaration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationSnapshot {
    pub schema_version: u32,
    pub seed: u64,
    pub total_sim_time: f32,

    pub nodes: Vec<SnapshotNode>,
    pub springs: Vec<SnapshotSpring>,

    pub food_pellets: Vec<SnapshotFood>,
    pub mineral_pellets: Vec<SnapshotMineral>,
    pub corpses: Vec<SnapshotCorpse>,

    pub diffusion_data: Vec<f32>,
}

impl SimulationSnapshot {
    pub fn from_world(world: &mut bevy_ecs::world::World, seed: u64, total_sim_time: f32) -> Self {
        let mut nodes = Vec::new();
        let mut springs = Vec::new();
        let mut food_pellets = Vec::new();
        let mut mineral_pellets = Vec::new();
        let mut corpses = Vec::new();
        let diffusion_data = Vec::new();

        // Query nodes
        let mut node_query = world.query::<(
            bevy_ecs::entity::Entity,
            &physics::ParticleNode,
            Option<&organisms::OrganismColor>,
            Option<&ecology::Diet>,
            Option<&ecology::EcologicalCategory>,
            Option<&reproduction::ReproductionStrategy>,
            Option<&brain::Brain>,
        )>();

        let mut entity_map = std::collections::HashMap::new();
        // Parallel index map (Phase 6, Epic E) so the many small
        // second-pass queries below can fill in a given entity's already-
        // pushed `SnapshotNode` in place, without needing one giant query
        // tuple — Bevy's `WorldQuery` tuple impls have a practical arity
        // ceiling, and this crate's own physiology/graph state alone is
        // already ~15 additional optional components.
        let mut entity_to_index = std::collections::HashMap::new();

        for (e, node, color, diet, category, repro, brain) in node_query.iter(world) {
            let id = e.to_bits();
            entity_map.insert(e, id);
            entity_to_index.insert(e, nodes.len());

            nodes.push(SnapshotNode {
                id,
                // Phase 8, Epic 8.13 (ADR-P8-08): full `Vec3` fidelity —
                // no truncation, unlike the pre-8.13 `SerializedVec2`.
                position: node.position.into(),
                velocity: node.velocity.into(),
                mass: node.mass,
                segment_type: node.segment_type,
                is_fixed: node.is_fixed,
                organism_id: node.organism_id,
                color: color.map(|c| c.0),
                diet: diet.cloned(),
                category: category.cloned(),
                genome: repro.map(|r| r.genome.clone()),
                brain: brain.cloned(),
                chemical_economy: None,
                age: None,
                metabolism: None,
                health: None,
                hydration: None,
                body_temperature: None,
                generation: None,
                spawn_tick: None,
                life_stage: None,
                morphogen_level: None,
                hormone_level: None,
                neuromodulator_channels: None,
                infection: None,
                segment_infection: None,
                segment_immunity: None,
                developmental_graph: None,
            });
        }

        // Second pass: one small query per physiology/graph component,
        // filling in the already-pushed `SnapshotNode` by index. Each of
        // these mirrors the exact same "query, then look up by entity"
        // shape, kept separate rather than folded into a helper macro so
        // each line stays independently greppable against the component
        // type it restores.
        let mut chem_query =
            world.query::<(bevy_ecs::entity::Entity, &metabolism::ChemicalEconomy)>();
        for (e, c) in chem_query.iter(world) {
            if let Some(&i) = entity_to_index.get(&e) {
                nodes[i].chemical_economy = Some(c.clone());
            }
        }

        let mut age_query = world.query::<(bevy_ecs::entity::Entity, &metabolism::Age)>();
        for (e, a) in age_query.iter(world) {
            if let Some(&i) = entity_to_index.get(&e) {
                nodes[i].age = Some(a.clone());
            }
        }

        let mut metabolism_query =
            world.query::<(bevy_ecs::entity::Entity, &metabolism::Metabolism)>();
        for (e, m) in metabolism_query.iter(world) {
            if let Some(&i) = entity_to_index.get(&e) {
                nodes[i].metabolism = Some(m.clone());
            }
        }

        let mut health_query = world.query::<(bevy_ecs::entity::Entity, &metabolism::Health)>();
        for (e, h) in health_query.iter(world) {
            if let Some(&i) = entity_to_index.get(&e) {
                nodes[i].health = Some(h.clone());
            }
        }

        let mut hydration_query =
            world.query::<(bevy_ecs::entity::Entity, &metabolism::Hydration)>();
        for (e, h) in hydration_query.iter(world) {
            if let Some(&i) = entity_to_index.get(&e) {
                nodes[i].hydration = Some(h.clone());
            }
        }

        let mut body_temp_query =
            world.query::<(bevy_ecs::entity::Entity, &metabolism::BodyTemperature)>();
        for (e, b) in body_temp_query.iter(world) {
            if let Some(&i) = entity_to_index.get(&e) {
                nodes[i].body_temperature = Some(b.clone());
            }
        }

        let mut generation_query =
            world.query::<(bevy_ecs::entity::Entity, &organisms::Generation)>();
        for (e, g) in generation_query.iter(world) {
            if let Some(&i) = entity_to_index.get(&e) {
                nodes[i].generation = Some(*g);
            }
        }

        let mut spawn_tick_query =
            world.query::<(bevy_ecs::entity::Entity, &organisms::SpawnTick)>();
        for (e, s) in spawn_tick_query.iter(world) {
            if let Some(&i) = entity_to_index.get(&e) {
                nodes[i].spawn_tick = Some(*s);
            }
        }

        let mut life_stage_query =
            world.query::<(bevy_ecs::entity::Entity, &organisms::LifeStage)>();
        for (e, l) in life_stage_query.iter(world) {
            if let Some(&i) = entity_to_index.get(&e) {
                nodes[i].life_stage = Some(*l);
            }
        }

        let mut morphogen_query =
            world.query::<(bevy_ecs::entity::Entity, &organisms::MorphogenLevel)>();
        for (e, m) in morphogen_query.iter(world) {
            if let Some(&i) = entity_to_index.get(&e) {
                nodes[i].morphogen_level = Some(*m);
            }
        }

        let mut hormone_query = world.query::<(bevy_ecs::entity::Entity, &brain::HormoneLevel)>();
        for (e, h) in hormone_query.iter(world) {
            if let Some(&i) = entity_to_index.get(&e) {
                nodes[i].hormone_level = Some(*h);
            }
        }

        let mut neuro_query = world.query::<(bevy_ecs::entity::Entity, &brain::Neuromodulators)>();
        for (e, n) in neuro_query.iter(world) {
            if let Some(&i) = entity_to_index.get(&e) {
                nodes[i].neuromodulator_channels = Some([n.dopamine, n.serotonin, n.noradrenaline]);
            }
        }

        let mut infection_query =
            world.query::<(bevy_ecs::entity::Entity, &ecology::disease::Infection)>();
        for (e, i) in infection_query.iter(world) {
            if let Some(&idx) = entity_to_index.get(&e) {
                nodes[idx].infection = Some(i.clone());
            }
        }

        let mut segment_infection_query = world.query::<(
            bevy_ecs::entity::Entity,
            &ecology::disease::SegmentInfection,
        )>();
        for (e, s) in segment_infection_query.iter(world) {
            if let Some(&i) = entity_to_index.get(&e) {
                nodes[i].segment_infection = Some(*s);
            }
        }

        let mut segment_immunity_query =
            world.query::<(bevy_ecs::entity::Entity, &ecology::disease::SegmentImmunity)>();
        for (e, s) in segment_immunity_query.iter(world) {
            if let Some(&i) = entity_to_index.get(&e) {
                nodes[i].segment_immunity = Some(*s);
            }
        }

        // The persistent Body Graph — present only on an organism's head
        // entity. Its nodes' `entity` fields are remapped through the same
        // `entity_map` every other cross-reference in this file uses.
        let mut graph_query =
            world.query::<(bevy_ecs::entity::Entity, &organisms::DevelopmentalGraph)>();
        for (e, graph) in graph_query.iter(world) {
            if let Some(&i) = entity_to_index.get(&e) {
                let snapshot_nodes = graph
                    .nodes
                    .iter()
                    .map(|n| SnapshotDevelopmentalNode {
                        role: n.role,
                        outputs: n.outputs,
                        parent: n.parent,
                        is_branch: n.is_branch,
                        position: n.position,
                        entity_id: n.entity.and_then(|ent| entity_map.get(&ent).copied()),
                    })
                    .collect();
                nodes[i].developmental_graph = Some(snapshot_nodes);
            }
        }

        // Query springs
        let mut spring_query = world.query::<&physics::Spring>();
        for spring in spring_query.iter(world) {
            if let (Some(&node_a_id), Some(&node_b_id)) = (
                entity_map.get(&spring.node_a),
                entity_map.get(&spring.node_b),
            ) {
                springs.push(SnapshotSpring {
                    node_a_id,
                    node_b_id,
                    constraint_type: spring.constraint_type,
                    rest_length: spring.rest_length,
                    base_length: spring.base_length,
                    stiffness: spring.stiffness,
                    damping: spring.damping,
                    actuation_amplitude: spring.actuation_amplitude,
                    actuation_phase: spring.actuation_phase,
                    breaking_strain: spring.breaking_strain,
                    is_fin: spring.is_fin,
                });
            }
        }

        // Query food
        let mut food_query = world.query::<&ecology::FoodPellet>();
        for food in food_query.iter(world) {
            food_pellets.push(SnapshotFood {
                position: food.position.into(),
                energy_value: food.energy_value,
            });
        }

        let mut mineral_query = world.query::<&ecology::MineralPellet>();
        for min in mineral_query.iter(world) {
            mineral_pellets.push(SnapshotMineral {
                position: min.position.into(),
                energy_value: min.energy_value,
            });
        }

        let mut corpse_query = world.query::<&ecology::Corpse>();
        for corpse in corpse_query.iter(world) {
            corpses.push(SnapshotCorpse {
                position: corpse.position.into(),
                energy_value: corpse.energy_value,
                decay_timer: corpse.decay_timer,
                max_decay: corpse.max_decay,
            });
        }

        Self {
            schema_version: crate::SchemaVersion::CURRENT.0,
            seed,
            total_sim_time,
            nodes,
            springs,
            food_pellets,
            mineral_pellets,
            corpses,
            diffusion_data,
        }
    }

    pub fn restore_world(&self, world: &mut bevy_ecs::world::World) {
        world.clear_entities();

        let mut id_map = std::collections::HashMap::new();

        for node in &self.nodes {
            let restored_position: common::Vec3 = node.position.clone().into();
            let restored_velocity: common::Vec3 = node.velocity.clone().into();
            let mut entity_cmds = world.spawn(physics::ParticleNode {
                position: restored_position,
                velocity: restored_velocity,
                force: common::Vec3::ZERO,
                mass: node.mass,
                segment_type: node.segment_type,
                is_fixed: node.is_fixed,
                organism_id: node.organism_id,
            });

            if let Some(color) = node.color {
                entity_cmds.insert(organisms::OrganismColor(color));
            }
            if let Some(diet) = &node.diet {
                entity_cmds.insert(diet.clone());
            }
            if let Some(category) = &node.category {
                entity_cmds.insert(category.clone());
            }
            if let Some(genome) = &node.genome {
                entity_cmds.insert(reproduction::ReproductionStrategy {
                    energy_threshold: 180.0,
                    energy_cost: 100.0,
                    cooldown_ticks: 300,
                    current_cooldown: 0,
                    mode: reproduction::ReproductionMode::Asexual,
                    genome: genome.clone(),
                });
            }
            if let Some(brain) = &node.brain {
                entity_cmds.insert(brain.clone());
            }

            // Phase 6, Epic E: restore every previously-lost physiology
            // component. `Neuromodulators` is reconstructed via `::new`
            // (seeded from this same node's restored ATP, or `0.0` if this
            // node never had a `ChemicalEconomy`) and then its 3 public
            // channels are overwritten from the snapshot — see
            // `SnapshotNode::neuromodulator_channels`'s doc comment for why
            // it isn't a direct `Neuromodulators` round-trip.
            if let Some(chem) = &node.chemical_economy {
                entity_cmds.insert(chem.clone());
            }
            if let Some(age) = &node.age {
                entity_cmds.insert(age.clone());
            }
            if let Some(metabolism) = &node.metabolism {
                entity_cmds.insert(metabolism.clone());
            }
            if let Some(health) = &node.health {
                entity_cmds.insert(health.clone());
            }
            if let Some(hydration) = &node.hydration {
                entity_cmds.insert(hydration.clone());
            }
            if let Some(body_temperature) = &node.body_temperature {
                entity_cmds.insert(body_temperature.clone());
            }
            if let Some(generation) = node.generation {
                entity_cmds.insert(generation);
            }
            if let Some(spawn_tick) = node.spawn_tick {
                entity_cmds.insert(spawn_tick);
            }
            if let Some(life_stage) = node.life_stage {
                entity_cmds.insert(life_stage);
            }
            if let Some(morphogen_level) = node.morphogen_level {
                entity_cmds.insert(morphogen_level);
            }
            if let Some(hormone_level) = node.hormone_level {
                entity_cmds.insert(hormone_level);
            }
            if let Some(channels) = node.neuromodulator_channels {
                let seed_atp = node.chemical_economy.as_ref().map(|c| c.atp).unwrap_or(0.0);
                let mut neuro = brain::Neuromodulators::new(seed_atp);
                neuro.dopamine = channels[0];
                neuro.serotonin = channels[1];
                neuro.noradrenaline = channels[2];
                entity_cmds.insert(neuro);
            }
            if let Some(infection) = &node.infection {
                entity_cmds.insert(infection.clone());
            }
            if let Some(segment_infection) = node.segment_infection {
                entity_cmds.insert(segment_infection);
            }
            if let Some(segment_immunity) = node.segment_immunity {
                entity_cmds.insert(segment_immunity);
            }

            id_map.insert(node.id, entity_cmds.id());
        }

        // Second pass for `DevelopmentalGraph`: every entity now exists, so
        // `entity_id`s can finally be remapped through `id_map` (a graph
        // node may reference a sibling segment that hadn't been spawned yet
        // in the first pass, since `self.nodes` isn't guaranteed
        // head-first).
        for node in &self.nodes {
            let Some(snapshot_nodes) = &node.developmental_graph else {
                continue;
            };
            let Some(&head_entity) = id_map.get(&node.id) else {
                continue;
            };
            let restored_nodes = snapshot_nodes
                .iter()
                .map(|n| organisms::DevelopmentalNode {
                    role: n.role,
                    outputs: n.outputs,
                    parent: n.parent,
                    is_branch: n.is_branch,
                    position: n.position,
                    entity: n.entity_id.and_then(|id| id_map.get(&id).copied()),
                })
                .collect();
            world
                .entity_mut(head_entity)
                .insert(organisms::DevelopmentalGraph {
                    nodes: restored_nodes,
                });
        }

        for spring in &self.springs {
            if let (Some(&node_a), Some(&node_b)) =
                (id_map.get(&spring.node_a_id), id_map.get(&spring.node_b_id))
            {
                world.spawn(physics::Spring {
                    node_a,
                    node_b,
                    constraint_type: spring.constraint_type,
                    rest_length: spring.rest_length,
                    base_length: spring.base_length,
                    stiffness: spring.stiffness,
                    damping: spring.damping,
                    actuation_amplitude: spring.actuation_amplitude,
                    actuation_phase: spring.actuation_phase,
                    breaking_strain: spring.breaking_strain,
                    is_fin: spring.is_fin,
                });
            }
        }

        for food in &self.food_pellets {
            world.spawn(ecology::FoodPellet {
                position: food.position.clone().into(),
                energy_value: food.energy_value,
            });
        }

        for min in &self.mineral_pellets {
            world.spawn(ecology::MineralPellet {
                position: min.position.clone().into(),
                energy_value: min.energy_value,
            });
        }

        for corpse in &self.corpses {
            world.spawn(ecology::Corpse {
                position: corpse.position.clone().into(),
                energy_value: corpse.energy_value,
                decay_timer: corpse.decay_timer,
                max_decay: corpse.max_decay,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;

    /// Phase 6, Epic E's own named verification requirement: a save-then-
    /// load round-trip test asserting the previously-lost fields survive.
    /// Builds a small organism (head + one body segment) carrying every
    /// component this milestone newly covers, snapshots it, restores it
    /// into a fresh `World`, and confirms each value survived — not just
    /// that the snapshot *contains* the data (that would only prove
    /// `from_world`, not the full round-trip `restore_world` completes).
    #[test]
    fn save_then_load_round_trip_preserves_previously_lost_physiology_and_graph_state() {
        let mut world = World::new();

        let head = world
            .spawn((
                physics::ParticleNode {
                    // Non-zero `z` (Phase 8, Epic 8.13) proves the
                    // round trip preserves full 3D fidelity, not just
                    // the pre-8.13 truncate-to-2D/re-extend-with-0.0
                    // behavior.
                    position: common::Vec3::new(1.0, 2.0, 5.0),
                    velocity: common::Vec3::ZERO,
                    force: common::Vec3::ZERO,
                    mass: 1.0,
                    segment_type: 0,
                    is_fixed: false,
                    organism_id: 1,
                },
                metabolism::ChemicalEconomy {
                    glucose: 111.0,
                    o2: 222.0,
                    co2: 3.0,
                    atp: 444.0,
                    max_glucose: 1000.0,
                    max_o2: 1000.0,
                    max_co2: 1000.0,
                    max_atp: 1000.0,
                },
                metabolism::Age {
                    ticks: 777,
                    max_lifespan: 10_000,
                },
                metabolism::Metabolism {
                    mass: 12.5,
                    base_rate: 0.02,
                    is_plant: false,
                },
                metabolism::Health {
                    current: 55.0,
                    max: 100.0,
                },
                metabolism::Hydration {
                    level: 0.75,
                    loss_rate: 0.001,
                },
                metabolism::BodyTemperature {
                    current: 21.0,
                    ideal: 22.0,
                },
                organisms::Generation(4),
                organisms::SpawnTick(123),
                organisms::LifeStage::Adult,
                organisms::MorphogenLevel {
                    concentration: 0.42,
                },
                ecology::disease::Infection {
                    state: ecology::disease::InfectionState::Infectious,
                    ticks_in_state: 9,
                    virulence: 0.3,
                    transmissibility: 0.1,
                },
            ))
            .id();
        let mut neuro = brain::Neuromodulators::new(444.0);
        neuro.dopamine = 0.6;
        neuro.serotonin = 0.7;
        neuro.noradrenaline = 0.3;
        world.entity_mut(head).insert(neuro);

        let segment = world
            .spawn((
                physics::ParticleNode {
                    position: common::Vec3::new(3.0, 4.0, 0.0),
                    velocity: common::Vec3::ZERO,
                    force: common::Vec3::ZERO,
                    mass: 1.0,
                    segment_type: 1,
                    is_fixed: false,
                    organism_id: 1,
                },
                brain::HormoneLevel {
                    dopamine: 0.1,
                    serotonin: 0.2,
                    noradrenaline: 0.3,
                },
                ecology::disease::SegmentInfection { severity: 0.5 },
                ecology::disease::SegmentImmunity { resistance: 0.05 },
                organisms::MorphogenLevel { concentration: 0.9 },
            ))
            .id();

        let mut graph = organisms::DevelopmentalGraph::new();
        graph.nodes.push(organisms::DevelopmentalNode {
            role: genetics::SegmentType::Head,
            outputs: genetics::DevelopmentalOutputs {
                segment_type: genetics::SegmentType::Head,
                branches: false,
                actuation_amplitude: 0.0,
                actuation_phase: 0.0,
                pigment: [0.1, 0.2, 0.3],
                apoptosis: false,
            },
            parent: None,
            is_branch: false,
            position: 0,
            entity: Some(head),
        });
        graph.nodes.push(organisms::DevelopmentalNode {
            role: genetics::SegmentType::Torso,
            outputs: genetics::DevelopmentalOutputs {
                segment_type: genetics::SegmentType::Torso,
                branches: false,
                actuation_amplitude: 0.5,
                actuation_phase: 1.0,
                pigment: [0.4, 0.5, 0.6],
                apoptosis: false,
            },
            parent: Some(0),
            is_branch: false,
            position: 1,
            entity: Some(segment),
        });
        world.entity_mut(head).insert(graph);

        let snapshot = SimulationSnapshot::from_world(&mut world, 42, 0.0);
        assert_eq!(snapshot.schema_version, crate::SchemaVersion::CURRENT.0);

        let mut restored = World::new();
        snapshot.restore_world(&mut restored);

        // Find the restored head by its distinguishing Age/Generation
        // combination, and the restored segment by its HormoneLevel — the
        // point of this test is that both entities exist post-restore with
        // their previously-lost components intact, not that entity ids
        // matched (they never do — Bevy assigns fresh ones).
        let mut head_query = restored.query::<(
            &physics::ParticleNode,
            &metabolism::ChemicalEconomy,
            &metabolism::Age,
            &metabolism::Metabolism,
            &metabolism::Health,
            &metabolism::Hydration,
            &metabolism::BodyTemperature,
            &organisms::Generation,
            &organisms::SpawnTick,
            &organisms::LifeStage,
            &organisms::MorphogenLevel,
            &ecology::disease::Infection,
            &brain::Neuromodulators,
            &organisms::DevelopmentalGraph,
        )>();
        let (
            node,
            chem,
            age,
            metabolism,
            health,
            hydration,
            body_temp,
            generation,
            spawn_tick,
            life_stage,
            morphogen,
            infection,
            neuromodulators,
            graph,
        ) = head_query
            .iter(&restored)
            .next()
            .expect("restored head entity should carry every previously-lost component");

        // Phase 8, Epic 8.13's own named verification requirement: real 3D
        // data survives the round trip, not just the pre-8.13 flat 2D case.
        assert_eq!(node.position, common::Vec3::new(1.0, 2.0, 5.0));

        assert_eq!(chem.glucose, 111.0);
        assert_eq!(chem.atp, 444.0);
        assert_eq!(age.ticks, 777);
        assert_eq!(metabolism.mass, 12.5);
        assert_eq!(health.current, 55.0);
        assert_eq!(hydration.level, 0.75);
        assert_eq!(body_temp.current, 21.0);
        assert_eq!(generation.0, 4);
        assert_eq!(spawn_tick.0, 123);
        assert_eq!(*life_stage, organisms::LifeStage::Adult);
        assert_eq!(morphogen.concentration, 0.42);
        assert_eq!(infection.ticks_in_state, 9);
        assert_eq!(neuromodulators.dopamine, 0.6);
        assert_eq!(neuromodulators.serotonin, 0.7);
        assert_eq!(neuromodulators.noradrenaline, 0.3);

        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.nodes[1].parent, Some(0));
        assert_eq!(graph.nodes[1].role, genetics::SegmentType::Torso);
        // The graph's own `entity` references must have been remapped to
        // real (fresh) restored entities, not left as stale/`None`.
        let restored_head_entity = graph.nodes[0]
            .entity
            .expect("head node's entity reference must survive the round trip");
        let restored_segment_entity = graph.nodes[1]
            .entity
            .expect("segment node's entity reference must survive the round trip");
        assert_ne!(restored_head_entity, restored_segment_entity);

        let mut segment_query = restored.query::<(
            &brain::HormoneLevel,
            &ecology::disease::SegmentInfection,
            &ecology::disease::SegmentImmunity,
            &organisms::MorphogenLevel,
        )>();
        let (hormone, seg_infection, seg_immunity, seg_morphogen) = segment_query
            .get(&restored, restored_segment_entity)
            .expect(
                "the graph's remapped segment entity should carry the segment's own components",
            );
        assert_eq!(hormone.dopamine, 0.1);
        assert_eq!(seg_infection.severity, 0.5);
        assert_eq!(seg_immunity.resistance, 0.05);
        assert_eq!(seg_morphogen.concentration, 0.9);
    }
}
