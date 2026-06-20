#![allow(missing_docs)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotNode {
    pub id: u64, // Internal mapping ID
    pub position: SerializedVec2,
    pub velocity: SerializedVec2,
    pub mass: f32,
    pub segment_type: u32,
    pub is_fixed: bool,

    // Optional attributes per node
    pub color: Option<[f32; 3]>,
    pub diet: Option<ecology::Diet>,
    pub category: Option<ecology::EcologicalCategory>,

    // Only one node per organism needs to store the genome/brain
    pub genome: Option<genetics::Genome>,
    pub brain: Option<brain::Brain>,
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
    pub position: SerializedVec2,
    pub energy_value: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMineral {
    pub position: SerializedVec2,
    pub energy_value: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotCorpse {
    pub position: SerializedVec2,
    pub energy_value: f32,
    pub decay_timer: u32,
    pub max_decay: u32,
}

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

        for (e, node, color, diet, category, repro, brain) in node_query.iter(world) {
            let id = e.to_bits();
            entity_map.insert(e, id);

            nodes.push(SnapshotNode {
                id,
                position: node.position.into(),
                velocity: node.velocity.into(),
                mass: node.mass,
                segment_type: node.segment_type,
                is_fixed: node.is_fixed,
                color: color.map(|c| c.0),
                diet: diet.cloned(),
                category: category.cloned(),
                genome: repro.map(|r| r.genome.clone()),
                brain: brain.cloned(),
            });
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
            let mut entity_cmds = world.spawn(physics::ParticleNode {
                position: node.position.clone().into(),
                velocity: node.velocity.clone().into(),
                force: common::Vec2::ZERO,
                mass: node.mass,
                segment_type: node.segment_type,
                is_fixed: node.is_fixed,
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

            id_map.insert(node.id, entity_cmds.id());
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
