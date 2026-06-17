//! # Phylon Learning
//!
//! Reinforcement learning interfaces, observation and action space definitions,
//! and policy API contracts.
//!
//! This crate defines the *interfaces* that the simulation exposes to external
//! RL trainers. It is intentionally lean and independent of any ML framework
//! so that multiple backends (`burn`, external Python via `pyo3`, etc.) can
//! implement the policy trait without coupling the rest of the simulation.
//!
//! ## Phase 0 scope
//!
//! Interface trait declarations only. Implementation: Phase 11.

#![warn(missing_docs)]
#![warn(clippy::all)]

/// A flattened observation vector produced by the sensing layer.
///
/// Observers in an RL context receive this vector as input to a policy network.
/// The length is fixed per-organism and defined by the genome's sensor
/// configuration.
///
/// TODO(phase-11): Implement observation space bounds and normalization.
pub type ObservationVector = Vec<f32>;

/// A flattened action vector produced by a policy network.
///
/// Consumed by the behavior layer to drive motor actions. Length is fixed
/// by the action space definition in the organism's genome.
///
/// TODO(phase-11): Implement action space bounds and clamping.
pub type ActionVector = Vec<f32>;

/// The interface between the simulation and an RL policy provider.
///
/// Implement this trait to connect any policy backend (local inference,
/// remote server, scripted rule-set) to the simulation loop.
///
/// TODO(phase-11): Add async variant for remote policy servers.
pub trait PolicyProvider: Send + Sync {
    /// Given an observation, returns the action vector.
    fn act(&self, observation: &ObservationVector) -> ActionVector;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trivial zero-output policy for testing.
    struct ZeroPolicy;
    impl PolicyProvider for ZeroPolicy {
        fn act(&self, obs: &ObservationVector) -> ActionVector {
            vec![0.0; obs.len()]
        }
    }

    #[test]
    fn zero_policy_returns_correct_length() {
        let obs = vec![1.0, 2.0, 3.0];
        let policy = ZeroPolicy;
        let action = policy.act(&obs);
        assert_eq!(action.len(), obs.len());
    }
}
