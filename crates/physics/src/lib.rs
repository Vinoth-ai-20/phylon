//! # Phylon Physics
//!
//! Particle-spring soft-body dynamics, collision detection and response,
//! and force integration for the Phylon simulation.
//!
//! The physics model is node-and-edge based: organisms are represented as
//! networks of point masses connected by spring constraints. This natively
//! supports modular body plans, tissue deformation, and amputation mechanics.
//!
//! ## Integrator
//!
//! The default integrator is **Symplectic Euler** (semi-implicit), selected
//! for its energy-conserving properties in oscillatory spring networks.
//! Velocity Verlet is available as an alternative for experiments requiring
//! higher accuracy.
//!
//! ## Collision detection
//!
//! - **Broad phase**: Uniform Grid Spatial Hash (O(1) per entity in dense scenes).
//! - **Narrow phase**: Node-level circle intersection tests.
//!
//! ## Phase 0 scope
//!
//! Type signatures only. Implementation: Phase 2.

#![warn(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::prelude::{Component, Query, Res};
use common::Vec2;

/// Errors produced by the physics subsystem.
#[derive(Debug, thiserror::Error)]
pub enum PhysicsError {
    /// A spring constraint references a node that does not exist.
    #[error("spring references unknown node")]
    UnknownNode,
}

impl common::PhylonError for PhysicsError {}

/// A single point mass in the particle-spring network.
#[derive(Component, Debug, Clone, Default)]
pub struct ParticleNode {
    /// Current position in simulation space.
    pub position: Vec2,
    /// Current velocity.
    pub velocity: Vec2,
    /// Accumulated force for this tick (reset after integration).
    pub force: Vec2,
    /// Mass of this node in simulation mass units.
    pub mass: f32,
    /// Segment type (0=Head, 1=Torso, 2=Muscle, 3=Tail, 4=Fin)
    pub segment_type: u32,
    /// Whether the node is fixed in place.
    pub is_fixed: bool,
}

impl ParticleNode {
    /// Creates a new node at the given position.
    pub fn new(position: Vec2, mass: f32, segment_type: u32) -> Self {
        Self {
            position,
            velocity: Vec2::ZERO,
            force: Vec2::ZERO,
            mass,
            segment_type,
            is_fixed: false,
        }
    }
}

/// The physical behavior of a constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ConstraintType {
    /// Elastic muscle: actuates and acts like a damped spring.
    Elastic,
    /// Rigid bone: position-based dynamics enforce exact distance.
    Rigid,
    /// Passive tissue: acts as a standard soft spring.
    Passive,
    /// Rotational hinge/motor: dynamically alters target angle or flaps.
    Rotational,
}

/// A spring connecting two nodes.
#[derive(Component, Debug, Clone)]
pub struct Spring {
    /// The first node entity.
    pub node_a: bevy_ecs::entity::Entity,
    /// The second node entity.
    pub node_b: bevy_ecs::entity::Entity,
    /// The type of constraint (Muscle, Bone, Fat).
    pub constraint_type: ConstraintType,
    /// Current rest length of the spring (modified by muscle actuation).
    pub rest_length: f32,
    /// Base rest length (the original genome-encoded length).
    pub base_length: f32,
    /// Spring stiffness (k).
    pub stiffness: f32,
    /// Damping coefficient to prevent infinite oscillation.
    pub damping: f32,
    /// Amplitude of actuation (0.0 if not a muscle).
    pub actuation_amplitude: f32,
    /// Phase offset for actuation.
    pub actuation_phase: f32,
    /// Ratio of extension beyond rest_length before the spring breaks (e.g. 2.0 = breaks at 2x rest_length).
    pub breaking_strain: f32,
    /// Indicates if this segment is a lateral fin (1) or not (0), used for anisotropic drag.
    pub is_fin: u32,
}

/// Resource holding the fixed timestep physics configuration.
#[derive(bevy_ecs::prelude::Resource, Debug, Clone)]
pub struct PhysicsConfig {
    /// Time step delta t for integration.
    pub dt: f32,
    /// Number of substeps per tick.
    pub substep_count: u32,
    /// Global dampening factor applied to velocity.
    pub dampening: f32,
    /// Pull towards the origin.
    pub centering_force: f32,
    /// Downward gravity force.
    pub gravity: f32,
    /// Repulsion strength during collisions.
    pub collision_force: f32,
    /// Inter-particle repulsion strength (non-colliding).
    pub repel_force: f32,
    /// Spring link strength multiplier.
    pub links_force: f32,
    /// Repulsion force from world bounds.
    pub wall_force: f32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            dt: 0.016,
            substep_count: 1,
            dampening: 0.99,
            centering_force: 0.0,
            gravity: 0.0,
            collision_force: 1.0,
            repel_force: 1.0,
            links_force: 1.0,
            wall_force: 1.0,
        }
    }
}

/// Computes the spring forces between nodes and adds them to `ParticleNode.force`.
#[allow(clippy::type_complexity)]
pub fn spring_force_system(
    mut commands: bevy_ecs::prelude::Commands,
    mut queries: bevy_ecs::system::ParamSet<(
        Query<(bevy_ecs::prelude::Entity, &Spring)>,
        Query<&mut ParticleNode>,
    )>,
) {
    let mut forces_to_apply = Vec::new();
    let mut springs_to_break = Vec::new();

    let mut spring_clones = Vec::new();
    for (entity, spring) in queries.p0().iter() {
        spring_clones.push((entity, spring.clone()));
    }

    let nodes = queries.p1();
    for (entity, spring) in spring_clones {
        if let (Ok(node_a), Ok(node_b)) = (nodes.get(spring.node_a), nodes.get(spring.node_b)) {
            let diff = node_b.position - node_a.position;
            let dist = diff.length();

            // Check breaking strain
            if dist > spring.base_length * spring.breaking_strain {
                springs_to_break.push(entity);
                continue;
            }

            if dist > 0.0001 {
                let dir = diff / dist;
                let rel_vel = node_b.velocity - node_a.velocity;
                let spring_force = (dist - spring.rest_length) * spring.stiffness;
                let damping_force = rel_vel.dot(dir) * spring.damping;

                let total_force = dir * (spring_force + damping_force);
                forces_to_apply.push((spring.node_a, total_force));
                forces_to_apply.push((spring.node_b, -total_force));
            }
        }
    }

    for entity in springs_to_break {
        commands.entity(entity).despawn();
    }

    // Apply forces
    let mut nodes = queries.p1();
    for (entity, force) in forces_to_apply {
        if let Ok(mut node) = nodes.get_mut(entity) {
            node.force += force;
        }
    }
}

/// Integrates positions and velocities using Symplectic Euler.
pub fn physics_integration_system(config: Res<PhysicsConfig>, mut query: Query<&mut ParticleNode>) {
    let dt = config.dt;
    for mut node in query.iter_mut() {
        if node.mass > 0.0 && !node.is_fixed {
            let acceleration = node.force / node.mass;
            // Symplectic Euler: update velocity first, then position.
            node.velocity += acceleration * dt;
            let dv = node.velocity * dt;
            node.position += dv;
            // Reset forces for next tick
            node.force = Vec2::ZERO;

            // Add a slight global damping to prevent chaotic explosion
            node.velocity *= 0.99;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particle_node_initial_state() {
        let node = ParticleNode::new(Vec2::new(1.0, 2.0), 3.0, 1);
        assert_eq!(node.position, Vec2::new(1.0, 2.0));
        assert_eq!(node.velocity, Vec2::ZERO);
        assert_eq!(node.force, Vec2::ZERO);
        assert_eq!(node.mass, 3.0);
        assert_eq!(node.segment_type, 1);
        assert!(!node.is_fixed);
    }
}
