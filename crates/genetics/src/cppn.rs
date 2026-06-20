use serde::{Deserialize, Serialize};

/// A node in the Compositional Pattern Producing Network (CPPN).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CppnNode {
    /// Activation function for this node.
    pub activation: brain::ActivationFn,
    /// Bias weight.
    pub bias: f32,
    /// Layer index (0 for inputs, 1 for hidden, 2 for outputs, etc) to ensure feedforward topological sort.
    pub layer: usize,
}

/// A directed connection (synapse) in the CPPN.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CppnConnection {
    /// Source node index.
    pub source: usize,
    /// Target node index.
    pub target: usize,
    /// Connection weight.
    pub weight: f32,
    /// Whether this connection is active.
    pub enabled: bool,
    /// Innovation number (for NEAT crossover).
    pub innovation: usize,
}
