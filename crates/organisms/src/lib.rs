//! # Phylon Organisms
//!
//! Organism archetype definitions, ECS component bundles, and lifecycle types.
//!
//! Every simulated organism is a set of ECS components. This crate defines
//! the canonical component bundles and the [`DietType`] enum that governs
//! ecological interactions.
//!
//! ## Phase 0 scope
//!
//! Component type declarations and DietType enum. ECS integration: Phase 3.

#![warn(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::prelude::{Component, Entity, Query};
use common::{EntityId, SimEnergy, SimLength, Vec2};
use serde::{Deserialize, Serialize};

/// The dietary strategy of an organism.
///
/// This is the primary axis of ecological role assignment. Diet determines
/// what an organism can consume and how it interacts with resource fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DietType {
    /// Consumes plant matter and nutrient fields.
    Herbivore,
    /// Hunts and consumes other organisms.
    Carnivore,
    /// Consumes both plant matter and other organisms.
    Omnivore,
    /// Consumes fungal networks and decomposing matter.
    Fungivore,
    /// Harvests energy directly from the sunlight field.
    Phototroph,
    /// Consumes dead organisms and carrion.
    Scavenger,
    /// Lives on a host organism, draining its energy.
    Parasite,
    /// Consumes detritus and waste products.
    Detritivore,
}

/// Spatial components of an organism: position, velocity, and collision radius.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialComponents {
    /// Current position in the simulation world.
    pub position: Vec2,
    /// Current velocity vector.
    pub velocity: Vec2,
    /// Collision and sensing radius.
    pub radius: SimLength,
}

/// Biological state components: energy, age, and diet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiologicalComponents {
    /// Current energy reserve. Organism dies when this reaches zero.
    pub energy: SimEnergy,
    /// Age in simulation ticks.
    pub age_ticks: u64,
    /// The organism's dietary strategy.
    pub diet: DietType,
    /// Parent entity ID (null if initial spawn).
    pub parent: EntityId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diet_type_is_copy() {
        let d = DietType::Herbivore;
        let _d2 = d; // copy semantics
    }

    #[test]
    fn all_diet_types_are_distinct() {
        let types = [
            DietType::Herbivore,
            DietType::Carnivore,
            DietType::Omnivore,
            DietType::Fungivore,
            DietType::Phototroph,
            DietType::Scavenger,
            DietType::Parasite,
            DietType::Detritivore,
        ];
        // All 8 variants present
        assert_eq!(types.len(), 8);
    }
}

/// Tracks the sequential growth of an organism from its genome.
#[derive(Component, Debug, Clone)]
pub struct GrowthState {
    /// The genome driving growth.
    pub genome: genetics::Genome,
    /// Next segment to build.
    pub next_segment_index: usize,
    /// Ticks remaining until the next segment buds.
    pub ticks_until_next_bud: u64,
    /// The interval between buds.
    pub base_bud_interval: u64,
    /// The nodes from the previously grown segment to attach to.
    pub parent_nodes: Vec<bevy_ecs::entity::Entity>,
    /// The position for the next segment.
    pub current_pos: Vec2,
    /// The length of each segment.
    pub segment_length: f32,
    /// The vertical distance between the two nodes in a segment.
    pub vertical_spread: f32,
    /// The list of effector muscles/fins created during growth.
    pub effectors: Vec<bevy_ecs::entity::Entity>,
}

/// System that builds out the organism's body sequentially.
pub fn growth_system(
    mut commands: bevy_ecs::prelude::Commands,
    mut query: Query<(Entity, &mut GrowthState)>,
) {
    use genetics::SegmentType;
    use physics::{ParticleNode, Spring};

    for (entity, mut state) in query.iter_mut() {
        let mut is_finished = false;
        if state.next_segment_index >= 10 {
            is_finished = true;
        }

        if state.ticks_until_next_bud > 0 && !is_finished {
            state.ticks_until_next_bud -= 1;
            continue;
        }

        let i = state.next_segment_index;
        let inputs = [i as f32 * 0.2, 0.0];
        let outputs = state.genome.evaluate(&inputs);

        if outputs.len() > 4 && outputs[4] < -0.5 {
            is_finished = true; // Stop signal
        }

        if is_finished {
            let input_count = 3; // e.g., chemical, energy, age
            let output_count = state.effectors.len();

            // Simple direct mapping for now: create output nodes for each effector
            let mut nodes = Vec::new();
            let mut synapses = Vec::new();

            // Inputs
            for _ in 0..input_count {
                nodes.push(brain::CtrnnNode {
                    state: 0.0,
                    time_constant: 1.0,
                    bias: 0.0,
                    activation: 7, // Linear
                });
            }

            // Outputs
            for _ in 0..output_count {
                nodes.push(brain::CtrnnNode {
                    state: 0.0,
                    time_constant: 0.5,
                    bias: 0.0,
                    activation: 1, // Tanh [-1, 1]
                });
            }

            // Fully connect inputs to outputs with CPPN weight queries
            for i in 0..input_count {
                for j in 0..output_count {
                    // For a real HyperNEAT we'd evaluate (x1, y1, x2, y2)
                    let w_inputs = [
                        (i as f32) / (input_count as f32),
                        (j as f32) / (output_count as f32),
                    ];
                    let w_outputs = state.genome.evaluate(&w_inputs);
                    let weight = if !w_outputs.is_empty() {
                        w_outputs[0] * 5.0
                    } else {
                        0.5
                    };

                    synapses.push(brain::CtrnnSynapse {
                        source: i as u32,
                        target: (input_count + j) as u32,
                        weight,
                        _padding: 0,
                    });
                }
            }

            // Insert components on Head
            commands.entity(entity).insert((
                brain::Brain::new(
                    brain::BrainId(0),
                    nodes,
                    synapses,
                    input_count,
                    output_count,
                ),
                sensing::SensoryState::new(input_count),
                behavior::MotorSystem {
                    effectors: state.effectors.clone(),
                },
            ));

            commands.entity(entity).remove::<GrowthState>();
            continue;
        }

        let mut best_idx = 1; // Default Torso
        if outputs.len() >= 4 {
            let mut max_val = f32::NEG_INFINITY;
            for (idx, &val) in outputs.iter().enumerate().take(4) {
                if val > max_val {
                    max_val = val;
                    best_idx = idx;
                }
            }
        }

        let segment = match best_idx {
            0 => SegmentType::Head,
            1 => SegmentType::Torso,
            2 => SegmentType::Muscle,
            _ => SegmentType::Tail,
        };

        let stiffness = match segment {
            SegmentType::Head => 10.0,
            SegmentType::Torso => 15.0,
            SegmentType::Muscle => 8.0,
            SegmentType::Tail => 2.0,
            SegmentType::Fin => 5.0,
        };

        let actuation_amplitude = match segment {
            SegmentType::Muscle => 5.0,
            _ => 0.0,
        };

        let actuation_phase = i as f32 * std::f32::consts::PI / 4.0;

        let n1_pos = state.current_pos + Vec2::new(0.0, state.vertical_spread);
        let n2_pos = state.current_pos + Vec2::new(0.0, -state.vertical_spread);

        let segment_u32 = match segment {
            SegmentType::Head => 0,
            SegmentType::Torso => 1,
            SegmentType::Muscle => 2,
            SegmentType::Tail => 3,
            SegmentType::Fin => 4,
        };

        let n1 = commands
            .spawn(ParticleNode::new(n1_pos, 1.0, segment_u32))
            .id();
        let n2 = commands
            .spawn(ParticleNode::new(n2_pos, 1.0, segment_u32))
            .id();

        let constraint_type = match segment {
            SegmentType::Head => physics::ConstraintType::Rigid,
            SegmentType::Torso => physics::ConstraintType::Rigid,
            SegmentType::Muscle => physics::ConstraintType::Elastic,
            SegmentType::Tail => physics::ConstraintType::Passive,
            SegmentType::Fin => physics::ConstraintType::Rotational,
        };

        // Connect the two nodes with a vertical spring
        let s1 = commands
            .spawn(Spring {
                node_a: n1,
                node_b: n2,
                constraint_type,
                rest_length: state.vertical_spread * 2.0,
                base_length: state.vertical_spread * 2.0,
                stiffness,
                damping: 0.5,
                actuation_amplitude: 0.0,
                actuation_phase: 0.0,
                breaking_strain: 2.0,
                is_fin: 0,
            })
            .id();
        if constraint_type == physics::ConstraintType::Elastic
            || constraint_type == physics::ConstraintType::Rotational
        {
            state.effectors.push(s1);
        }

        // Connect to previous segment nodes (spine to spine)
        if state.parent_nodes.len() >= 2 {
            let p1 = state.parent_nodes[0];
            let p2 = state.parent_nodes[1];

            // Horizontal springs
            let s2 = commands
                .spawn(Spring {
                    node_a: p1,
                    node_b: n1,
                    constraint_type,
                    rest_length: state.segment_length,
                    base_length: state.segment_length,
                    stiffness,
                    damping: 0.5,
                    actuation_amplitude,
                    actuation_phase,
                    breaking_strain: 2.0,
                    is_fin: 0,
                })
                .id();
            if constraint_type == physics::ConstraintType::Elastic
                || constraint_type == physics::ConstraintType::Rotational
            {
                state.effectors.push(s2);
            }

            let s3 = commands
                .spawn(Spring {
                    node_a: p2,
                    node_b: n2,
                    constraint_type,
                    rest_length: state.segment_length,
                    base_length: state.segment_length,
                    stiffness,
                    damping: 0.5,
                    actuation_amplitude,
                    actuation_phase,
                    breaking_strain: 2.0,
                    is_fin: 0,
                })
                .id();
            if constraint_type == physics::ConstraintType::Elastic
                || constraint_type == physics::ConstraintType::Rotational
            {
                state.effectors.push(s3);
            }

            // Cross springs for structural stability
            let cross_length = (state.segment_length * state.segment_length
                + (state.vertical_spread * 2.0).powi(2))
            .sqrt();
            commands.spawn(Spring {
                node_a: p1,
                node_b: n2,
                constraint_type: physics::ConstraintType::Passive,
                rest_length: cross_length,
                base_length: cross_length,
                stiffness,
                damping: 0.5,
                actuation_amplitude,
                actuation_phase,
                breaking_strain: 2.0,
                is_fin: 0,
            });
            commands.spawn(Spring {
                node_a: p2,
                node_b: n1,
                constraint_type: physics::ConstraintType::Passive,
                rest_length: cross_length,
                base_length: cross_length,
                stiffness,
                damping: 0.5,
                actuation_amplitude,
                actuation_phase,
                breaking_strain: 2.0,
                is_fin: 0,
            });
        }

        // Evaluate branching signal
        let branching_signal = if outputs.len() > 5 { outputs[5] } else { -1.0 };
        if branching_signal > 0.0 {
            // Spawn lateral fins
            let f1_pos = state.current_pos + Vec2::new(0.0, state.vertical_spread * 2.0);
            let f2_pos = state.current_pos + Vec2::new(0.0, -state.vertical_spread * 2.0);

            let f1 = commands.spawn(ParticleNode::new(f1_pos, 0.5, 4)).id();
            let f2 = commands.spawn(ParticleNode::new(f2_pos, 0.5, 4)).id();

            // Connect fins to spine with Rotational constraints
            let sf1 = commands
                .spawn(Spring {
                    node_a: n1,
                    node_b: f1,
                    constraint_type: physics::ConstraintType::Rotational,
                    rest_length: state.vertical_spread,
                    base_length: state.vertical_spread,
                    stiffness: 5.0,
                    damping: 0.5,
                    actuation_amplitude: 0.0,
                    actuation_phase: 0.0,
                    breaking_strain: 2.0,
                    is_fin: 1,
                })
                .id();
            state.effectors.push(sf1);

            let sf2 = commands
                .spawn(Spring {
                    node_a: n2,
                    node_b: f2,
                    constraint_type: physics::ConstraintType::Rotational,
                    rest_length: state.vertical_spread,
                    base_length: state.vertical_spread,
                    stiffness: 5.0,
                    damping: 0.5,
                    actuation_amplitude: 0.0,
                    actuation_phase: 0.0,
                    breaking_strain: 2.0,
                    is_fin: 1,
                })
                .id();
            state.effectors.push(sf2);

            // Advance state, tracking fins as the new parent nodes
            state.parent_nodes = vec![n1, n2, f1, f2];
        } else {
            // Advance state with just the spine nodes
            state.parent_nodes = vec![n1, n2];
        }

        state.current_pos.x -= state.segment_length;
        state.next_segment_index += 1;
        state.ticks_until_next_bud = state.base_bud_interval;
    }
}

/// Spawns an organism's zygote based on its genome.
pub fn spawn_organism(
    world: &mut bevy_ecs::world::World,
    genome: &genetics::Genome,
    start_pos: Vec2,
) {
    use genetics::SegmentType;
    use physics::{ParticleNode, Spring};

    let segment_length = 20.0;
    let vertical_spread = 10.0;

    let inputs = [0.0, 0.0];
    let outputs = genome.evaluate(&inputs);
    let mut best_idx = 1; // Torso
    if outputs.len() >= 4 {
        let mut max_val = f32::NEG_INFINITY;
        for (idx, &val) in outputs.iter().enumerate().take(4) {
            if val > max_val {
                max_val = val;
                best_idx = idx;
            }
        }
    }

    let segment = match best_idx {
        0 => SegmentType::Head,
        1 => SegmentType::Torso,
        2 => SegmentType::Muscle,
        _ => SegmentType::Tail,
    };

    let stiffness = match segment {
        SegmentType::Head => 10.0,
        SegmentType::Torso => 15.0,
        SegmentType::Muscle => 8.0,
        SegmentType::Tail => 2.0,
        SegmentType::Fin => 5.0,
    };

    let n1_pos = start_pos + Vec2::new(0.0, vertical_spread);
    let n2_pos = start_pos + Vec2::new(0.0, -vertical_spread);

    // Initial spine nodes are always Torso (1) by default or whatever we choose
    let segment_u32 = match segment {
        SegmentType::Head => 0,
        SegmentType::Torso => 1,
        SegmentType::Muscle => 2,
        SegmentType::Tail => 3,
        SegmentType::Fin => 4,
    };

    let n1 = world
        .spawn(ParticleNode::new(n1_pos, 1.0, segment_u32))
        .id();
    let n2 = world
        .spawn(ParticleNode::new(n2_pos, 1.0, segment_u32))
        .id();

    // First node (Head) holds the biological state
    world.entity_mut(n1).insert((
        metabolism::Energy {
            current: 100.0,
            max: 200.0,
        },
        metabolism::Age {
            ticks: 0,
            max_lifespan: 10000,
        },
        metabolism::Metabolism {
            mass: 10.0, // Initial mass
            base_rate: 0.05,
        },
        ecology::Diet::Herbivore,
        reproduction::ReproductionStrategy {
            energy_threshold: 180.0,
            energy_cost: 100.0,
            cooldown_ticks: 300,
            current_cooldown: 0,
            mode: reproduction::ReproductionMode::Asexual,
            genome: genome.clone(),
        },
        GrowthState {
            genome: genome.clone(),
            next_segment_index: 1,    // Already spawned segment 0
            ticks_until_next_bud: 60, // 1 second per segment bud
            base_bud_interval: 60,
            parent_nodes: vec![n1, n2],
            current_pos: start_pos - Vec2::new(segment_length, 0.0),
            segment_length,
            vertical_spread,
            effectors: Vec::new(),
        },
    ));

    let constraint_type = match segment {
        SegmentType::Head => physics::ConstraintType::Rigid,
        SegmentType::Torso => physics::ConstraintType::Rigid,
        SegmentType::Muscle => physics::ConstraintType::Elastic,
        SegmentType::Tail => physics::ConstraintType::Passive,
        SegmentType::Fin => physics::ConstraintType::Rotational,
    };

    world.spawn(Spring {
        node_a: n1,
        node_b: n2,
        constraint_type,
        rest_length: vertical_spread * 2.0,
        base_length: vertical_spread * 2.0,
        stiffness,
        damping: 0.5,
        actuation_amplitude: 0.0,
        actuation_phase: 0.0,
        breaking_strain: 2.0,
        is_fin: 0,
    });
}
