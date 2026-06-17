//! # Phylon Behavior
//!
//! Movement decisions, action selection, and locomotion output systems.
//!
//! Organisms receive sensory inputs, process them through their neural brain,
//! and emit motor commands. This crate converts neural output into physical
//! forces applied to the organism's particle nodes.
//!
//! ## Phase 0 scope
//!
//! Action type declaration. Implementation: Phase 3.

#![warn(missing_docs)]
#![warn(clippy::all)]

use common::Vec2;

/// A motor action emitted by an organism's brain for one tick.
#[derive(Debug, Clone, Copy)]
pub struct MotorAction {
    /// Desired force vector to apply to the organism's center of mass.
    pub thrust: Vec2,
    /// Rotational torque (positive = counter-clockwise).
    pub torque: f32,
}

impl MotorAction {
    /// The zero action — no movement.
    pub const IDLE: Self = Self {
        thrust: Vec2::ZERO,
        torque: 0.0,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_action_is_zero() {
        assert_eq!(MotorAction::IDLE.thrust, Vec2::ZERO);
        assert_eq!(MotorAction::IDLE.torque, 0.0);
    }
}
