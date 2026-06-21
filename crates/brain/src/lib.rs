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
    /// Start index of synapses targeting this node.
    pub first_synapse: u32,
    /// Number of synapses targeting this node.
    pub synapse_count: u32,
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
    /// Extracts the output values from the current node states.
    /// In the new architecture, the integration happens on the GPU,
    /// so this simply reads the post-activation output states.
    pub fn get_outputs(&self) -> Vec<f32> {
        if self.nodes.is_empty() {
            return Vec::new();
        }

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

    /// Sets the input node states from sensor values.
    /// This happens on CPU before uploading the nodes to the GPU.
    pub fn set_inputs(&mut self, inputs: &[f32]) {
        for (i, &input_val) in inputs.iter().enumerate() {
            if i < self.input_count && i < self.nodes.len() {
                self.nodes[i].state = input_val;
            }
        }
    }

    /// Applies the mathematical activation function mapped to the given activation ID.
    pub fn apply_activation(x: f32, act_id: u32) -> f32 {
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

    /// Creates a new functional CTRNN brain and properly sorts synapses for GPU compatibility.
    pub fn new(
        id: BrainId,
        mut nodes: Vec<CtrnnNode>,
        mut synapses: Vec<CtrnnSynapse>,
        input_count: usize,
        output_count: usize,
    ) -> Self {
        // Sort synapses by target node to allow efficient GPU gather operations
        synapses.sort_by_key(|s| s.target);

        // Reset all synapse counts
        for node in &mut nodes {
            node.first_synapse = 0;
            node.synapse_count = 0;
        }

        // Compute offsets
        if !synapses.is_empty() {
            let mut current_target = synapses[0].target as usize;
            let mut current_start = 0;
            let mut current_count = 0;

            for (i, syn) in synapses.iter().enumerate() {
                if syn.target as usize != current_target {
                    if current_target < nodes.len() {
                        nodes[current_target].first_synapse = current_start;
                        nodes[current_target].synapse_count = current_count;
                    }
                    current_target = syn.target as usize;
                    current_start = i as u32;
                    current_count = 1;
                } else {
                    current_count += 1;
                }
            }
            // Tail
            if current_target < nodes.len() {
                nodes[current_target].first_synapse = current_start;
                nodes[current_target].synapse_count = current_count;
            }
        }

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
