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
}

/// System that builds out the organism's body sequentially.
pub fn growth_system(
    mut commands: bevy_ecs::prelude::Commands,
    mut query: Query<(Entity, &mut GrowthState)>,
) {
    use genetics::SegmentType;
    use physics::{ParticleNode, Spring};

    for (entity, mut state) in query.iter_mut() {
        if state.next_segment_index >= state.genome.segments.len() {
            // Finished growing
            commands.entity(entity).remove::<GrowthState>();
            continue;
        }

        if state.ticks_until_next_bud > 0 {
            state.ticks_until_next_bud -= 1;
            continue;
        }

        let i = state.next_segment_index;
        let segment = &state.genome.segments[i];

        let stiffness = match segment {
            SegmentType::Head => 10.0,
            SegmentType::Torso => 15.0,
            SegmentType::Muscle => 8.0,
            SegmentType::Tail => 2.0,
        };

        let actuation_amplitude = match segment {
            SegmentType::Muscle => 5.0,
            _ => 0.0,
        };

        let actuation_phase = i as f32 * std::f32::consts::PI / 4.0;

        let n1_pos = state.current_pos + Vec2::new(0.0, state.vertical_spread);
        let n2_pos = state.current_pos + Vec2::new(0.0, -state.vertical_spread);

        let n1 = commands.spawn(ParticleNode::new(n1_pos, 1.0)).id();
        let n2 = commands.spawn(ParticleNode::new(n2_pos, 1.0)).id();

        // Connect the two nodes with a vertical spring
        commands.spawn(Spring {
            node_a: n1,
            node_b: n2,
            rest_length: state.vertical_spread * 2.0,
            base_length: state.vertical_spread * 2.0,
            stiffness,
            damping: 0.5,
            actuation_amplitude: 0.0,
            actuation_phase: 0.0,
            breaking_strain: 2.0,
        });

        // Connect to previous segment nodes
        if state.parent_nodes.len() == 2 {
            let p1 = state.parent_nodes[0];
            let p2 = state.parent_nodes[1];

            // Horizontal springs
            commands.spawn(Spring {
                node_a: p1,
                node_b: n1,
                rest_length: state.segment_length,
                base_length: state.segment_length,
                stiffness,
                damping: 0.5,
                actuation_amplitude,
                actuation_phase,
                breaking_strain: 2.0,
            });
            commands.spawn(Spring {
                node_a: p2,
                node_b: n2,
                rest_length: state.segment_length,
                base_length: state.segment_length,
                stiffness,
                damping: 0.5,
                actuation_amplitude,
                actuation_phase,
                breaking_strain: 2.0,
            });

            // Cross springs for structural stability
            let cross_length = (state.segment_length * state.segment_length
                + (state.vertical_spread * 2.0).powi(2))
            .sqrt();
            commands.spawn(Spring {
                node_a: p1,
                node_b: n2,
                rest_length: cross_length,
                base_length: cross_length,
                stiffness,
                damping: 0.5,
                actuation_amplitude,
                actuation_phase,
                breaking_strain: 2.0,
            });
            commands.spawn(Spring {
                node_a: p2,
                node_b: n1,
                rest_length: cross_length,
                base_length: cross_length,
                stiffness,
                damping: 0.5,
                actuation_amplitude,
                actuation_phase,
                breaking_strain: 2.0,
            });
        }

        // Advance state
        state.parent_nodes = vec![n1, n2];
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

    if genome.segments.is_empty() {
        return;
    }

    let segment = &genome.segments[0];
    let stiffness = match segment {
        SegmentType::Head => 10.0,
        SegmentType::Torso => 15.0,
        SegmentType::Muscle => 8.0,
        SegmentType::Tail => 2.0,
    };

    let n1_pos = start_pos + Vec2::new(0.0, vertical_spread);
    let n2_pos = start_pos + Vec2::new(0.0, -vertical_spread);

    let mut n1_builder = world.spawn(ParticleNode::new(n1_pos, 1.0));
    // First node (Head) holds the biological state
    n1_builder.insert((
        metabolism::Energy {
            current: 100.0,
            max: 200.0,
        },
        metabolism::Age {
            ticks: 0,
            max_lifespan: 10000,
        },
        metabolism::Metabolism {
            mass: genome.segments.len() as f32 * 2.0,
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
            parent_nodes: Vec::new(),
            current_pos: start_pos - Vec2::new(segment_length, 0.0),
            segment_length,
            vertical_spread,
        },
    ));

    let n1 = n1_builder.id();
    let n2 = world.spawn(ParticleNode::new(n2_pos, 1.0)).id();

    // Now that n2 is spawned, update n1's GrowthState to reference them
    if let Some(mut state) = world.get_mut::<GrowthState>(n1) {
        state.parent_nodes = vec![n1, n2];
    }

    world.spawn(Spring {
        node_a: n1,
        node_b: n2,
        rest_length: vertical_spread * 2.0,
        base_length: vertical_spread * 2.0,
        stiffness,
        damping: 0.5,
        actuation_amplitude: 0.0,
        actuation_phase: 0.0,
        breaking_strain: 2.0,
    });
}
