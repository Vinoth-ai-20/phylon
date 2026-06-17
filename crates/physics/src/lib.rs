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
///
/// TODO(phase-2): Add full constraint solve and integrator.
#[allow(dead_code)]
pub struct ParticleNode {
    /// Current position in simulation space.
    pub position: Vec2,
    /// Current velocity.
    pub velocity: Vec2,
    /// Accumulated force for this tick (reset after integration).
    pub force: Vec2,
    /// Mass of this node in simulation mass units.
    pub mass: f32,
}

impl ParticleNode {
    /// Creates a new particle node at `position` with `mass` and zero velocity.
    pub fn new(position: Vec2, mass: f32) -> Self {
        Self {
            position,
            velocity: Vec2::ZERO,
            force: Vec2::ZERO,
            mass,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particle_node_initial_state() {
        let node = ParticleNode::new(Vec2::new(1.0, 2.0), 3.0);
        assert_eq!(node.position, Vec2::new(1.0, 2.0));
        assert_eq!(node.velocity, Vec2::ZERO);
        assert_eq!(node.force, Vec2::ZERO);
        assert_eq!(node.mass, 3.0);
    }
}
