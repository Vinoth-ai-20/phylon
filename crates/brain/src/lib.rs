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
    /// Gaussian activation `exp(-x^2)` (useful for bilateral symmetry in CPPNs).
    Gaussian,
    /// Absolute value `|x|`.
    Abs,
    /// Linear / Identity `x`.
    Linear,
    /// Step function: `0` if `x < 0`, else `1`.
    Step,
}

/// A single node in the CTRNN, designed for GPU/Pod compatibility.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Serialize, Deserialize)]
pub struct CtrnnNode {
    /// The current state/activation potential of the node.
    pub state: f32,
    /// The time constant (tau) dictating how fast the state updates.
    pub time_constant: f32,
    /// Bias added before activation.
    pub bias: f32,
    /// Activation function index (mapped from ActivationFn enum).
    pub activation: u32,
}

/// A synapse connecting two nodes in the CTRNN, designed for GPU/Pod compatibility.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Serialize, Deserialize)]
pub struct CtrnnSynapse {
    /// Source node index.
    pub source: u32,
    /// Target node index.
    pub target: u32,
    /// Connection weight.
    pub weight: f32,
    /// Padding for 16-byte alignment.
    pub _padding: u32,
}

/// A Continuous-Time Recurrent Neural Network (CTRNN).
#[derive(bevy_ecs::prelude::Component, Debug, Clone, Serialize, Deserialize)]
pub struct Brain {
    /// Unique identifier for this brain.
    pub id: BrainId,
    /// Nodes in the network.
    pub nodes: Vec<CtrnnNode>,
    /// Synapses connecting the nodes.
    pub synapses: Vec<CtrnnSynapse>,
    /// Number of input nodes.
    pub input_count: usize,
    /// Number of output nodes.
    pub output_count: usize,
}

impl Brain {
    /// Steps the CTRNN forward by `dt` given an array of inputs.
    /// Returns the outputs of the network.
    pub fn step(&mut self, inputs: &[f32], dt: f32) -> Vec<f32> {
        if self.nodes.is_empty() {
            return Vec::new();
        }

        // Apply external inputs directly to the state of input nodes
        for (i, &input_val) in inputs.iter().enumerate() {
            if i < self.input_count && i < self.nodes.len() {
                self.nodes[i].state = input_val;
            }
        }

        // Compute pre-activation inputs for all nodes
        let mut network_inputs = vec![0.0; self.nodes.len()];
        for synapse in &self.synapses {
            let src_idx = synapse.source as usize;
            let tgt_idx = synapse.target as usize;

            if src_idx < self.nodes.len() && tgt_idx < self.nodes.len() {
                let src_activation = Self::apply_activation(
                    self.nodes[src_idx].state + self.nodes[src_idx].bias,
                    self.nodes[src_idx].activation,
                );
                network_inputs[tgt_idx] += src_activation * synapse.weight;
            }
        }

        // Update states using Euler integration
        for (i, node) in self.nodes.iter_mut().enumerate() {
            // Input nodes don't integrate via time constant, they just reflect the environment.
            if i >= self.input_count {
                let dy_dt = (1.0 / node.time_constant) * (-node.state + network_inputs[i]);
                node.state += dy_dt * dt;
            }
        }

        // Extract outputs
        let mut outputs = Vec::with_capacity(self.output_count);
        let start_idx = self.nodes.len().saturating_sub(self.output_count);
        for i in start_idx..self.nodes.len() {
            outputs.push(Self::apply_activation(
                self.nodes[i].state + self.nodes[i].bias,
                self.nodes[i].activation,
            ));
        }

        outputs
    }

    fn apply_activation(x: f32, act_id: u32) -> f32 {
        match act_id {
            0 => 1.0 / (1.0 + (-x).exp()), // Sigmoid
            1 => x.tanh(),                 // Tanh
            2 => x.max(0.0),               // ReLU
            3 => {
                if x > 0.0 {
                    x
                } else {
                    0.01 * x
                }
            } // LeakyReLU
            4 => x.sin(),                  // Sine
            5 => (-x * x).exp(),           // Gaussian
            6 => x.abs(),                  // Abs
            7 => x,                        // Linear
            8 => {
                if x > 0.0 {
                    1.0
                } else {
                    0.0
                }
            } // Step
            _ => x,
        }
    }

    /// Creates a new functional CTRNN brain.
    pub fn new(
        id: BrainId,
        nodes: Vec<CtrnnNode>,
        synapses: Vec<CtrnnSynapse>,
        input_count: usize,
        output_count: usize,
    ) -> Self {
        Self {
            id,
            nodes,
            synapses,
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
