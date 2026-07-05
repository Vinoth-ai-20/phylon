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
//! ## Current scope
//!
//! Interface trait declarations only ([`ObservationVector`], [`ActionVector`],
//! [`PolicyProvider`]) — no RL algorithm, training loop, or `burn`/`pyo3`
//! backend is implemented here yet. See the implementation roadmap's
//! "Learning Framework" epic.

#![warn(missing_docs)]
#![warn(clippy::all)]

/// # Sensory Observation Vector
///
/// ## 1. What Happens
/// The `ObservationVector` is a flat, continuous $1D$ tensor representing the instantaneous
/// sensory state of an organism (e.g., raycasts, olfaction gradients, proprioception).
///
/// ## 2. Why It Happens
/// Machine Learning models (like PPO or SAC in RL) expect fixed-length, normalized vector inputs.
/// Rather than passing complex ECS structs (which vary by species) into a neural network,
/// the `sensing` crate flattens biological state into this standardized format.
///
/// ## 3. How It Happens
/// The vector $O_t \in \mathbb{R}^N$ is constructed per-tick by iterating over all attached
/// sensor components, concatenating their outputs, and normalizing them to $[-1, 1]$.
pub type ObservationVector = Vec<f32>;

/// # Motor Action Vector
///
/// ## 1. What Happens
/// The `ActionVector` is a flat, continuous $1D$ tensor output by the `PolicyProvider` representing
/// the intended muscle actuations or chemical emissions for the current tick.
///
/// ## 2. Why It Happens
/// The physics and ecology systems need concrete scalar values to drive spring contraction ($k$)
/// or pheromone droplet mass. The neural network outputs generic floats; this type acts as the
/// translation contract back into the biological domain.
///
/// ## 3. How It Happens
/// The vector $A_t \in \mathbb{R}^M$ is consumed by the `behavior` crate. The values are typically
/// squashed via Tanh to $[-1, 1]$ and then scaled by the specific organ's mechanical limits:
///
/// $$ Actuation = A_t\[i\] \times MaxAmplitude $$
pub type ActionVector = Vec<f32>;

/// # Reinforcement Learning Policy Interface
///
/// ## 1. What Happens
/// `PolicyProvider` is the bridging trait between the Phylon Rust engine and external Machine
/// Learning inference backends (like PyTorch via PyO3, or local Burn models).
///
/// ## 2. Why It Happens
/// To keep the engine fast and modular, Phylon does not hardcode the intelligence algorithm.
/// By programming to an interface, researchers can hot-swap a hardcoded heuristic, a neat-evolved
/// CTRNN, or an external Python PPO agent without changing the core loop.
///
/// ## 3. How It Happens
/// During the cognitive phase of the tick, the engine maps the function $f: O_t \to A_t$:
///
/// $$ A_t = \text{PolicyProvider.act}(O_t) $$
///
/// The resulting $A_t$ is injected into the organism's `MotorSystem` component for the physics step.
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
