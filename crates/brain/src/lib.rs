//! # Phylon Brain
//!
//! Neural substrate for organisms: NEAT topology evolution, CTRNN dynamics,
//! Hebbian plasticity, and neuromodulator channels.
//!
//! The brain crate defines the data structures and evaluation interfaces for
//! neural networks. It is deliberately independent of `burn` in Phase 0 to
//! keep compilation fast. GPU-accelerated inference via `burn` is added in
//! Phase 6.
//!
//! ## Phase 0 scope
//!
//! BrainId and NeuralActivation placeholder types. Implementation: Phase 6.

#![warn(missing_docs)]
#![warn(clippy::all)]

use serde::{Deserialize, Serialize};

/// A unique identifier for a neural brain instance.
///
/// Distinct from [`common::EntityId`] because brains persist in the lineage
/// record for cross-generation comparison studies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BrainId(pub u64);

/// Activation function types available to neural nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivationFn {
    /// Standard sigmoid: `1 / (1 + exp(-x))`.
    Sigmoid,
    /// Hyperbolic tangent.
    Tanh,
    /// Rectified linear unit: `max(0, x)`.
    ReLU,
    /// Leaky ReLU with slope 0.01 for negative inputs.
    LeakyReLU,
    /// Sinusoidal activation (useful for rhythmic/oscillatory behaviours).
    Sine,
    /// Step function: `0` if `x < 0`, else `1`.
    Step,
}

/// Placeholder for a neural brain.
///
/// TODO(phase-6): Implement full NEAT topology, CTRNN dynamics, and
/// Hebbian plasticity.
#[allow(dead_code)]
pub struct Brain {
    /// Unique identifier for this brain.
    id: BrainId,
    /// Number of input nodes (set by the genome's sensor configuration).
    input_count: usize,
    /// Number of output nodes (one per motor action dimension).
    output_count: usize,
}

impl Brain {
    /// Creates a minimal placeholder brain.
    pub fn placeholder(id: BrainId, input_count: usize, output_count: usize) -> Self {
        Self {
            id,
            input_count,
            output_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brain_id_equality() {
        assert_eq!(BrainId(1), BrainId(1));
    }

    #[test]
    fn activation_fn_is_copy() {
        let a = ActivationFn::Sigmoid;
        let _a2 = a;
    }
}
