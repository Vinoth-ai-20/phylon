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

/// # Compositional Pattern Producing Network (NEAT Topology)
///
/// ## 1. What Happens
/// The `Cppn` is a specialized Artificial Neural Network architecture used to generate spatial
/// patterns and morphological traits (like brain weights or skin color) as a function of
/// geometry $(X, Y, \dots)$ rather than temporal inputs.
///
/// ## 2. Why It Happens
/// In natural biology, DNA doesn't store a 1:1 blueprint of the brain. It stores a "recipe"
/// that unfolds over space and time. A CPPN mathematically mimics this by taking spatial coordinates
/// and outputting traits, creating smooth gradients, symmetries, and repeating motifs—crucial
/// for generating complex, scalable biological structures with minimal genetic bytes.
///
/// ## 3. How It Happens
/// The network is a Directed Acyclic Graph (DAG) evaluated via topological feedforward.
/// For a node $N_i$ with activation function $f$, bias $b_i$, and incoming synapses $W_{j \to i}$:
///
/// $$ Output_i = f\left( b_i + \sum_{j} (Output_j \times W_{j \to i}) \right) $$
///
/// The structure evolves using NEAT (NeuroEvolution of Augmenting Topologies). Mutations
/// (`mutate_add_node`, `mutate_add_connection`) split edges and insert complexity over generations,
/// tracked via global `innovation` numbers for historical crossover.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Cppn {
    /// The nodes in the CPPN.
    pub nodes: Vec<CppnNode>,
    /// The connections in the CPPN.
    pub connections: Vec<CppnConnection>,
}

impl Cppn {
    /// Performs NEAT-style crossover with another CPPN.
    ///
    /// Nodes are only mixed when both parents share the same node count
    /// (guaranteeing connection indices stay valid); otherwise `self`'s nodes
    /// are kept. Matching connection genes (same `innovation` number) are
    /// inherited from a random parent; disjoint/excess genes always come from
    /// `self`. Any inherited connection referencing an out-of-range node
    /// index (possible when node counts differ) is dropped.
    pub fn crossover<R: rand::Rng>(&self, other: &Cppn, rng: &mut R) -> Cppn {
        let nodes = if self.nodes.len() == other.nodes.len() {
            self.nodes
                .iter()
                .zip(other.nodes.iter())
                .map(|(a, b)| {
                    if rng.gen_bool(0.5) {
                        a.clone()
                    } else {
                        b.clone()
                    }
                })
                .collect()
        } else {
            self.nodes.clone()
        };
        let node_count = nodes.len();

        let other_by_innovation: std::collections::HashMap<usize, &CppnConnection> = other
            .connections
            .iter()
            .map(|c| (c.innovation, c))
            .collect();

        let connections = self
            .connections
            .iter()
            .map(|c| match other_by_innovation.get(&c.innovation) {
                Some(other_c) if rng.gen_bool(0.5) => (*other_c).clone(),
                _ => c.clone(),
            })
            .filter(|c| c.source < node_count && c.target < node_count)
            .collect();

        Cppn { nodes, connections }
    }

    /// Creates a new empty CPPN.
    pub fn new() -> Self {
        Self::default()
    }

    /// Evaluates the CPPN for a given set of inputs.
    pub fn evaluate(&self, inputs: &[f32]) -> Vec<f32> {
        if self.nodes.is_empty() {
            return Vec::new();
        }

        let mut values = vec![0.0; self.nodes.len()];
        for (i, &val) in inputs.iter().enumerate() {
            if i < values.len() {
                values[i] = val;
            }
        }

        let start_idx = inputs.len();
        for target_idx in start_idx..self.nodes.len() {
            let mut sum = self.nodes[target_idx].bias;
            for conn in &self.connections {
                if conn.enabled && conn.target == target_idx {
                    sum += values[conn.source] * conn.weight;
                }
            }

            values[target_idx] = match self.nodes[target_idx].activation {
                brain::ActivationFn::Sigmoid => 1.0 / (1.0 + (-sum).exp()),
                brain::ActivationFn::Tanh => sum.tanh(),
                brain::ActivationFn::ReLU => sum.max(0.0),
                brain::ActivationFn::LeakyReLU => {
                    if sum > 0.0 {
                        sum
                    } else {
                        0.01 * sum
                    }
                }
                brain::ActivationFn::Sine => sum.sin(),
                brain::ActivationFn::Gaussian => (-sum * sum).exp(),
                brain::ActivationFn::Abs => sum.abs(),
                brain::ActivationFn::Linear => sum,
                brain::ActivationFn::Step => {
                    if sum > 0.0 {
                        1.0
                    } else {
                        0.0
                    }
                }
            };
        }

        let mut outputs = Vec::new();
        for (node_idx, node) in self.nodes.iter().enumerate() {
            if node.layer == 1 {
                outputs.push(values[node_idx]);
            }
        }
        outputs
    }

    /// Mutates a random existing connection's weight.
    pub fn mutate_weight(&mut self) {
        if self.connections.is_empty() {
            return;
        }
        let idx = rand::random::<usize>() % self.connections.len();
        let delta = rand::random::<f32>() - 0.5;
        self.connections[idx].weight += delta;
    }

    /// Adds a random connection between two unconnected nodes.
    pub fn mutate_add_connection(&mut self, next_innovation: &mut usize) {
        if self.nodes.len() < 2 {
            return;
        }

        for _ in 0..10 {
            let src = rand::random::<usize>() % self.nodes.len();
            let tgt = rand::random::<usize>() % self.nodes.len();

            if self.nodes[src].layer >= self.nodes[tgt].layer {
                continue;
            }

            let exists = self
                .connections
                .iter()
                .any(|c| c.source == src && c.target == tgt);
            if !exists {
                self.connections.push(CppnConnection {
                    source: src,
                    target: tgt,
                    weight: (rand::random::<f32>() - 0.5) * 2.0,
                    enabled: true,
                    innovation: *next_innovation,
                });
                *next_innovation += 1;
                break;
            }
        }
    }

    /// Splits a connection and inserts a new hidden node.
    pub fn mutate_add_node(&mut self, next_innovation: &mut usize) {
        if self.connections.is_empty() {
            return;
        }

        let mut enabled_indices = Vec::new();
        for (i, c) in self.connections.iter().enumerate() {
            if c.enabled {
                enabled_indices.push(i);
            }
        }

        if enabled_indices.is_empty() {
            return;
        }

        let idx = enabled_indices[rand::random::<usize>() % enabled_indices.len()];
        let conn = self.connections[idx].clone();

        self.connections[idx].enabled = false;

        let src_layer = self.nodes[conn.source].layer;
        let tgt_layer = self.nodes[conn.target].layer;

        let new_layer = if tgt_layer > src_layer + 1 {
            src_layer + 1
        } else {
            for n in &mut self.nodes {
                if n.layer >= tgt_layer {
                    n.layer += 1;
                }
            }
            src_layer + 1
        };

        let new_node_idx = self.nodes.len();
        self.nodes.push(CppnNode {
            activation: brain::ActivationFn::Tanh,
            bias: 0.0,
            layer: new_layer,
        });

        self.connections.push(CppnConnection {
            source: conn.source,
            target: new_node_idx,
            weight: 1.0,
            enabled: true,
            innovation: *next_innovation,
        });
        *next_innovation += 1;

        self.connections.push(CppnConnection {
            source: new_node_idx,
            target: conn.target,
            weight: conn.weight,
            enabled: true,
            innovation: *next_innovation,
        });
        *next_innovation += 1;
    }
}

/// Global tracking of NEAT innovation numbers.
#[derive(bevy_ecs::prelude::Resource, Debug, Clone, Serialize, Deserialize)]
pub struct GlobalInnovationTracker {
    /// The next available innovation number for a new structural mutation.
    pub next_innovation: usize,
}

impl Default for GlobalInnovationTracker {
    fn default() -> Self {
        Self {
            next_innovation: 100, // Reserve 0-99 for initial topologies
        }
    }
}
