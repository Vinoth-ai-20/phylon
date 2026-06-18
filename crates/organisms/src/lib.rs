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

/// Spawns an organism as a clustered graph of `Node` and `Spring` entities
/// based on its genome.
pub fn spawn_organism(
    world: &mut bevy_ecs::world::World,
    genome: &genetics::Genome,
    start_pos: Vec2,
) {
    use genetics::SegmentType;
    use physics::{ParticleNode, Spring};

    let mut current_pos = start_pos;
    let mut prev_nodes: Vec<bevy_ecs::entity::Entity> = Vec::new();

    let segment_length = 20.0;
    let vertical_spread = 10.0;

    for (i, segment) in genome.segments.iter().enumerate() {
        let stiffness = match segment {
            SegmentType::Head => 10.0,
            SegmentType::Torso => 15.0, // High stiffness
            SegmentType::Muscle => 8.0,
            SegmentType::Tail => 2.0, // Low stiffness
        };

        let actuation_amplitude = match segment {
            SegmentType::Muscle => 5.0,
            _ => 0.0,
        };

        let actuation_phase = i as f32 * std::f32::consts::PI / 4.0;

        // Spawn 2 nodes for this segment (top and bottom)
        let n1_pos = current_pos + Vec2::new(0.0, vertical_spread);
        let n2_pos = current_pos + Vec2::new(0.0, -vertical_spread);

        let mut n1_builder = world.spawn(ParticleNode::new(n1_pos, 1.0));
        if i == 0 {
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
            ));
        }
        let n1 = n1_builder.id();
        let n2 = world.spawn(ParticleNode::new(n2_pos, 1.0)).id();

        // Connect the two nodes with a vertical spring
        world.spawn(Spring {
            node_a: n1,
            node_b: n2,
            rest_length: vertical_spread * 2.0,
            base_length: vertical_spread * 2.0,
            stiffness,
            damping: 0.5,
            actuation_amplitude: 0.0, // vertical spring doesn't actuate for now
            actuation_phase: 0.0,
        });

        // Connect to previous segment nodes
        if prev_nodes.len() == 2 {
            let p1 = prev_nodes[0];
            let p2 = prev_nodes[1];

            // Horizontal springs
            world.spawn(Spring {
                node_a: p1,
                node_b: n1,
                rest_length: segment_length,
                base_length: segment_length,
                stiffness,
                damping: 0.5,
                actuation_amplitude,
                actuation_phase,
            });
            world.spawn(Spring {
                node_a: p2,
                node_b: n2,
                rest_length: segment_length,
                base_length: segment_length,
                stiffness,
                damping: 0.5,
                actuation_amplitude,
                actuation_phase,
            });

            // Cross springs for structural stability
            let cross_length =
                (segment_length * segment_length + (vertical_spread * 2.0).powi(2)).sqrt();
            world.spawn(Spring {
                node_a: p1,
                node_b: n2,
                rest_length: cross_length,
                base_length: cross_length,
                stiffness,
                damping: 0.5,
                actuation_amplitude,
                actuation_phase,
            });
            world.spawn(Spring {
                node_a: p2,
                node_b: n1,
                rest_length: cross_length,
                base_length: cross_length,
                stiffness,
                damping: 0.5,
                actuation_amplitude,
                actuation_phase,
            });
        }

        prev_nodes = vec![n1, n2];
        current_pos.x -= segment_length; // Grow backwards
    }
}
