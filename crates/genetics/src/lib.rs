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

/// One gene in the Hox sequence — describes a single axial segment and whether
/// it should sprout a lateral appendage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HoxGene {
    /// The type of this axial segment.
    pub segment: SegmentType,
    /// Branching threshold in `[-1, 1]`.  
    /// A value **> 0.0** means this segment grows a bilateral fin/limb pair.
    /// Torso and Muscle segments are the only ones where branching makes
    /// biological sense; the growth system should ignore this for Head/Tail.
    pub branching_signal: f32,
    /// Actuation amplitude for muscle segments (0.0 for non-muscle).
    pub actuation_amplitude: f32,
    /// Actuation phase offset (radians).
    pub actuation_phase: f32,
}

impl HoxGene {
    /// A plain structural torso gene with no branching.
    pub fn torso() -> Self {
        Self {
            segment: SegmentType::Torso,
            branching_signal: -1.0,
            actuation_amplitude: 0.0,
            actuation_phase: 0.0,
        }
    }

    /// A torso gene that **will** branch into bilateral fins.
    pub fn branching_torso(actuation_amplitude: f32, actuation_phase: f32) -> Self {
        Self {
            segment: SegmentType::Torso,
            branching_signal: 0.5, // > 0 → branch
            actuation_amplitude,
            actuation_phase,
        }
    }

    /// A muscle gene with a given actuation amplitude and phase.
    pub fn muscle(amplitude: f32, phase: f32) -> Self {
        Self {
            segment: SegmentType::Muscle,
            branching_signal: -1.0,
            actuation_amplitude: amplitude,
            actuation_phase: phase,
        }
    }

    /// A tail gene.
    pub fn tail() -> Self {
        Self {
            segment: SegmentType::Tail,
            branching_signal: -1.0,
            actuation_amplitude: 0.0,
            actuation_phase: 0.0,
        }
    }

    /// A head gene.
    pub fn head() -> Self {
        Self {
            segment: SegmentType::Head,
            branching_signal: -1.0,
            actuation_amplitude: 0.0,
            actuation_phase: 0.0,
        }
    }
}

/// The complete axial Hox sequence for an organism's body plan.
///
/// Growth walks this list front-to-back: index 0 is the anteriormost segment
/// (Head), the last index is the posteriormost (Tail).  Each intermediate
/// segment is a Torso, Muscle, or Fin gene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HoxSequence {
    /// The ordered list of segment genes (Head → ... → Tail).
    pub genes: Vec<HoxGene>,
    /// Per-organism skin colour encoded as `[R, G, B]` in `[0, 1]`.
    pub color: [f32; 3],
}

impl HoxSequence {
    /// Construct a sequence from a slice of genes and a colour.
    pub fn new(genes: Vec<HoxGene>, color: [f32; 3]) -> Self {
        Self { genes, color }
    }

    /// A minimal worm-like organism: Head + N Muscle segments + Tail.
    /// No branching.
    pub fn worm(torso_count: usize, color: [f32; 3]) -> Self {
        let mut genes = vec![HoxGene::head()];
        for i in 0..torso_count {
            let phase = i as f32 * std::f32::consts::PI / 2.0;
            // Amplitude kept to ≤6% of segment_length (20 units) to stay in
            // the numerically stable regime for symplectic-Euler + PBD.
            genes.push(HoxGene::muscle(1.2, phase));
        }
        genes.push(HoxGene::tail());
        Self::new(genes, color)
    }

    /// A fish-like organism: Head + some rigid Torso + branching Torso
    /// (fins) + muscle Torso + Tail.
    pub fn fish(torso_count: usize, fin_at: usize, color: [f32; 3]) -> Self {
        let mut genes = vec![HoxGene::head()];
        for i in 0..torso_count {
            if i == fin_at {
                // Fin amplitude 2.5 units ≈ 17% of fin_spread (15 units) —
                // enough to produce visible flapping without physics blow-up.
                genes.push(HoxGene::branching_torso(2.5, 0.0));
            } else {
                let phase = i as f32 * std::f32::consts::PI / 3.0;
                genes.push(HoxGene::muscle(1.2, phase));
            }
        }
        genes.push(HoxGene::tail());
        Self::new(genes, color)
    }
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

    /// Creates a genome driven entirely by an explicit [`HoxSequence`].
    ///
    /// The CPPN fields are left empty; the growth system will use `hox`
    /// exclusively for body-plan decisions.  Neural wiring is still derived
    /// from the empty CPPN (identity mapping) as a placeholder.
    pub fn new_hox_driven(id: GenomeId, origin: EntityId, hox: HoxSequence) -> Self {
        Self {
            schema_version: 1,
            id,
            origin,
            ploidy: Ploidy::Haploid,
            nodes: Vec::new(),
            connections: Vec::new(),
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

        values
    }

    /// Performs a simple crossover with another genome.
    pub fn crossover<R: rand::Rng>(&self, _other: &Genome, new_id: GenomeId, _rng: &mut R) -> Self {
        // TODO(phase-5): Full NEAT crossover based on historical innovation numbers.
        // For Phase 4, we just clone self (asexual drift).
        Self {
            schema_version: self.schema_version,
            id: new_id,
            origin: self.origin, // Caller must update
            ploidy: self.ploidy,
            nodes: self.nodes.clone(),
            connections: self.connections.clone(),
            hox: self.hox.clone(),
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
