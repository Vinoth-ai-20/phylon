use crate::cppn::{CppnConnection, CppnNode};
use crate::hox::HoxSequence;
use crate::types::{GenomeId, Ploidy};
use common::EntityId;
use serde::{Deserialize, Serialize};

/// The genome of an organism, represented as a CPPN.
///
/// The CPPN is evaluated over spatial coordinates during procedural growth
/// to dictate morphology (Segment types, Symmetries) and wire synaptic weights.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Genome {
    /// Schema version for serialization compatibility (currently 1).
    pub schema_version: u32,
    /// Unique identifier for this genome sequence.
    pub id: GenomeId,
    /// The ID of the organism that created this genome (for lineage tracking).
    pub origin: EntityId,
    /// Ploidy level (haploid or diploid).
    pub ploidy: Ploidy,
    /// CPPN Nodes.
    pub nodes: Vec<CppnNode>,
    /// CPPN Connections.
    pub connections: Vec<CppnConnection>,
    /// Optional explicit Hox body-plan sequence.
    ///
    /// When `Some`, the growth system reads the body plan directly from this
    /// sequence rather than querying the CPPN.  This is the primary morphology
    /// driver for Phase 3.
    pub hox: Option<HoxSequence>,
}

impl Genome {
    /// Creates a minimal genome (e.g. just inputs wired to outputs).
    pub fn new_minimal(id: GenomeId, origin: EntityId) -> Self {
        Self {
            schema_version: 1,
            id,
            origin,
            ploidy: Ploidy::Haploid,
            nodes: Vec::new(),
            connections: Vec::new(),
            hox: None,
        }
    }

    /// Creates a deterministic genome with a pre-defined Hox sequence.
    /// The CPPN is initialized with 3 disconnected nodes (2 inputs, 1 output)
    /// so it can be mutated later.
    pub fn new_hox_driven(id: GenomeId, origin: EntityId, hox: HoxSequence) -> Self {
        Self {
            schema_version: 1,
            id,
            origin,
            ploidy: Ploidy::Haploid,
            nodes: vec![
                CppnNode {
                    activation: brain::ActivationFn::Linear,
                    bias: 0.0,
                    layer: 0,
                }, // Input: Source Node Coord
                CppnNode {
                    activation: brain::ActivationFn::Linear,
                    bias: 0.0,
                    layer: 0,
                }, // Input: Target Node Coord
                CppnNode {
                    activation: brain::ActivationFn::Tanh,
                    bias: 0.0,
                    layer: 1,
                }, // Output: Connection Weight
            ],
            connections: vec![],
            hox: Some(hox),
        }
    }

    /// Mutates a random existing connection's weight.
    pub fn mutate_weight(&mut self) {
        if self.connections.is_empty() {
            return;
        }
        let idx = rand::random::<usize>() % self.connections.len();
        // Perturb by [-0.5, 0.5]
        let delta = rand::random::<f32>() - 0.5;
        self.connections[idx].weight += delta;
    }

    /// Adds a random connection between two unconnected nodes.
    pub fn mutate_add_connection(&mut self, next_innovation: &mut usize) {
        if self.nodes.len() < 2 {
            return;
        }

        // Try a few times to find an unconnected pair
        for _ in 0..10 {
            let src = rand::random::<usize>() % self.nodes.len();
            let tgt = rand::random::<usize>() % self.nodes.len();

            // Enforce feedforward: source layer must be < target layer
            if self.nodes[src].layer >= self.nodes[tgt].layer {
                continue;
            }

            // Check if connection already exists
            let exists = self
                .connections
                .iter()
                .any(|c| c.source == src && c.target == tgt);
            if !exists {
                self.connections.push(CppnConnection {
                    source: src,
                    target: tgt,
                    weight: (rand::random::<f32>() - 0.5) * 2.0, // Initial random weight [-1.0, 1.0]
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

        // Pick a random enabled connection
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

        // Disable old connection
        self.connections[idx].enabled = false;

        // Create new node in a layer between source and target
        let src_layer = self.nodes[conn.source].layer;
        let tgt_layer = self.nodes[conn.target].layer;

        let new_layer = if tgt_layer > src_layer + 1 {
            src_layer + 1
        } else {
            // Need to push all downstream nodes forward to make room for a new layer
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

        // Connection from source to new node (weight 1.0)
        self.connections.push(CppnConnection {
            source: conn.source,
            target: new_node_idx,
            weight: 1.0,
            enabled: true,
            innovation: *next_innovation,
        });
        *next_innovation += 1;

        // Connection from new node to target (original weight)
        self.connections.push(CppnConnection {
            source: new_node_idx,
            target: conn.target,
            weight: conn.weight,
            enabled: true,
            innovation: *next_innovation,
        });
        *next_innovation += 1;
    }

    /// Evaluates the CPPN for a given set of inputs.
    pub fn evaluate(&self, inputs: &[f32]) -> Vec<f32> {
        // A simple feedforward evaluation. We assume nodes are sorted by layer!
        if self.nodes.is_empty() {
            return Vec::new();
        }

        let mut values = vec![0.0; self.nodes.len()];

        // Feed inputs
        for (i, &val) in inputs.iter().enumerate() {
            if i < values.len() {
                values[i] = val;
            }
        }

        // We assume nodes are topologically sorted by layer index.
        for target_idx in 0..self.nodes.len() {
            let mut sum = self.nodes[target_idx].bias;
            for conn in &self.connections {
                if conn.enabled && conn.target == target_idx {
                    sum += values[conn.source] * conn.weight;
                }
            }

            // Apply activation
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

        // Return the last node's value as the output
        vec![values.last().copied().unwrap_or(0.0)]
    }

    /// Performs a simple crossover with another genome.
    pub fn crossover<R: rand::Rng>(&self, _other: &Genome, new_id: GenomeId, _rng: &mut R) -> Self {
        Self {
            schema_version: self.schema_version,
            id: new_id,
            origin: self.origin,
            ploidy: self.ploidy,
            nodes: self.nodes.clone(),
            connections: self.connections.clone(),
            hox: self.hox.clone(),
        }
    }

    /// Mutates the genome in place.
    pub fn mutate<R: rand::Rng>(&mut self, mutation_rate: f32, rng: &mut R) {
        if rng.gen::<f32>() < mutation_rate {
            // Structural mutation: Add Node
            if rng.gen::<f32>() < 0.05 {
                let mut dummy_innov = self
                    .connections
                    .iter()
                    .map(|c| c.innovation)
                    .max()
                    .unwrap_or(0)
                    + 1;
                self.mutate_add_node(&mut dummy_innov);
            }

            // Structural mutation: Add Connection
            if rng.gen::<f32>() < 0.10 {
                let mut dummy_innov = self
                    .connections
                    .iter()
                    .map(|c| c.innovation)
                    .max()
                    .unwrap_or(0)
                    + 1;
                self.mutate_add_connection(&mut dummy_innov);
            }

            // Weight perturbation
            for conn in &mut self.connections {
                if rng.gen::<f32>() < 0.2 {
                    conn.weight += rng.gen_range(-1.0..1.0);
                }
            }
        }
    }
}
