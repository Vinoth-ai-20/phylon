//! # Phylon Genetics
//!
//! Genome representation, mutation operators, crossover, and epigenetic markers.
//!
//! The genome is the heritable blueprint of an organism. It encodes body plan,
//! neural topology seeds, diet preferences, metabolic rates, and sensory
//! parameters via a base-4 bitstring representation.
//!
//! All stochastic operations (mutation, crossover) use `ChaCha8Rng` seeded
//! from the experiment manifest to ensure CPU-authoritative reproducibility.
//!
//! ## Phase 0 scope
//!
//! Genome type declaration and GenomeId. Full mutation and crossover: Phase 5.

#![warn(missing_docs)]
#![warn(clippy::all)]

use common::EntityId;
use serde::{Deserialize, Serialize};

/// A unique identifier for a genome sequence.
///
/// Distinct from [`EntityId`] because multiple organisms can share the same
/// genome (e.g., clones, twins) and a genome persists in the lineage record
/// after the organism dies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GenomeId(pub u64);

/// The ploidy level of a genome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ploidy {
    /// Single chromosome set — typical for microbes.
    Haploid,
    /// Two chromosome sets — typical for complex organisms.
    Diploid,
}

/// Represents a distinct morphological segment in the procedural soft-body growth phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SegmentType {
    /// Front sensory segment.
    Head,
    /// Structural central segment (high stiffness).
    Torso,
    /// Actuated segment that dynamically changes rest length (GPU computed).
    Muscle,
    /// Loose rear segment (low stiffness).
    Tail,
    /// Lateral proto-limb or fin for branched swimmers.
    Fin,
}

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

/// The genome of an organism, represented as a CPPN.
///
/// The CPPN is evaluated over spatial coordinates during procedural growth
/// to dictate morphology (Segment types, Symmetries) and wire synaptic weights.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Genome {
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
}

impl Genome {
    /// Creates a minimal genome (e.g. just inputs wired to outputs).
    pub fn new_minimal(id: GenomeId, origin: EntityId) -> Self {
        Self {
            id,
            origin,
            ploidy: Ploidy::Haploid,
            nodes: Vec::new(),
            connections: Vec::new(),
        }
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

        values
    }

    /// Performs a simple crossover with another genome.
    pub fn crossover<R: rand::Rng>(&self, _other: &Genome, new_id: GenomeId, _rng: &mut R) -> Self {
        // TODO(phase-5): Full NEAT crossover based on historical innovation numbers.
        // For Phase 4, we just clone self (asexual drift).
        Self {
            id: new_id,
            origin: self.origin, // Caller must update
            ploidy: self.ploidy,
            nodes: self.nodes.clone(),
            connections: self.connections.clone(),
        }
    }

    /// Mutates the genome in place.
    pub fn mutate<R: rand::Rng>(&mut self, mutation_rate: f32, rng: &mut R) {
        // TODO(phase-5): Add/Remove nodes and connections, perturb weights.
        if rng.gen::<f32>() < mutation_rate {
            for conn in &mut self.connections {
                if rng.gen::<f32>() < 0.1 {
                    conn.weight += rng.gen_range(-0.5..0.5);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_genome_is_empty() {
        let g = Genome::new_minimal(GenomeId(1), EntityId(0));
        assert_eq!(g.nodes.len(), 0);
        assert_eq!(g.connections.len(), 0);
    }

    #[test]
    fn genome_id_equality() {
        assert_eq!(GenomeId(1), GenomeId(1));
        assert_ne!(GenomeId(1), GenomeId(2));
    }
}
