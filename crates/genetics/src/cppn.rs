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
    /// Per-locus probability this connection's weight jitters during a
    /// mutation pass (see `Genome::mutate`) — evolvable, not a fixed
    /// global constant: it drifts slightly on every mutation pass and is
    /// inherited (like the weight itself) through crossover, so different
    /// connections in the same genome — and the same connection across
    /// generations — can settle on different volatility. New connections
    /// (from `mutate_add_connection`/`mutate_add_node`) start at
    /// [`DEFAULT_MUTATION_RATE`], matching the rate every connection used
    /// before this field existed.
    pub mutation_rate: f32,
}

/// Initial per-connection mutation rate for newly-created connections —
/// equal to the single global rate every connection used before per-locus
/// rates existed, so a brand-new genome's mutation behavior is unchanged
/// until rates start drifting via evolution.
pub const DEFAULT_MUTATION_RATE: f32 = 0.2;

/// Standard NEAT coefficient weighting excess genes in
/// [`Cppn::compatibility_distance`].
pub const EXCESS_COEFFICIENT: f32 = 1.0;
/// Standard NEAT coefficient weighting disjoint genes in
/// [`Cppn::compatibility_distance`].
pub const DISJOINT_COEFFICIENT: f32 = 1.0;
/// Standard NEAT coefficient weighting average matching-gene weight
/// difference in [`Cppn::compatibility_distance`].
pub const WEIGHT_DIFF_COEFFICIENT: f32 = 0.4;

/// # Compositional Pattern Producing Network (CPPN)
///
/// A CPPN is a small feedforward neural network evaluated not over time, but
/// as a pure function of position or index — e.g. "where along the body is
/// this segment" or "which pair of genes is this". Everywhere this crate
/// needs a smoothly-varying, evolvable value keyed by position (brain
/// synapse weights, body morphology, regulatory-gene biases and edge
/// weights), it queries a `Cppn` instead of consulting a lookup table.
///
/// ## Why a network instead of a table
///
/// A lookup table (one entry per position/gene-pair) has no structure: every
/// entry is independent, so mutating one entry can't smoothly reshape a
/// whole gradient, symmetry, or repeating motif — and the table's size grows
/// with the number of positions, not with the complexity of the pattern.
/// A CPPN instead encodes the *rule* that produces the pattern: its topology
/// and weights are the heritable, evolvable "genome"; the input position is
/// a query into that rule, not a storage key. Small mutations to a CPPN's
/// weights or topology therefore tend to produce small, smooth changes to
/// the resulting body plan or brain — an evolutionarily useful property
/// no plain lookup table gives you. This is directly modeled on real DNA,
/// which doesn't store a 1:1 blueprint of the body either — it stores a
/// compact "recipe" that unfolds over space and time.
///
/// ## Evaluation
///
/// The network is a directed acyclic graph (DAG), evaluated by processing
/// nodes in topological (layer) order so every node's inputs are already
/// computed by the time it's evaluated. For a node $N_i$ with activation
/// function $f$, bias $b_i$, and incoming connections $W_{j \to i}$:
///
/// $$ Output_i = f\left( b_i + \sum_{j} (Output_j \times W_{j \to i}) \right) $$
///
/// ## Topology evolution (NEAT)
///
/// Structure evolves using NEAT (NeuroEvolution of Augmenting Topologies):
/// [`Cppn::mutate_add_node`] splits an existing connection and inserts a new
/// hidden node in its place, and [`Cppn::mutate_add_connection`] adds a new
/// connection between two previously-unconnected nodes. Every structural
/// mutation is tagged with a global, ever-increasing `innovation` number
/// (see [`GlobalInnovationTracker`]), which is what lets
/// [`Cppn::crossover`]/[`Cppn::compatibility_distance`] match up
/// corresponding genes between two independently-evolved networks instead of
/// comparing them positionally.
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

    /// NEAT-style compatibility distance to another CPPN, for genetic-distance
    /// speciation — matches connection genes by `innovation` number exactly
    /// like [`Cppn::crossover`] does, but measures divergence instead of
    /// blending.
    ///
    /// Follows Stanley & Miikkulainen's original formula: genes beyond the
    /// smaller genome's highest innovation number are "excess", genes within
    /// range but present in only one parent are "disjoint", and genes present
    /// in both contribute their average `|weight|` difference. `c1`/`c2`/`c3`
    /// weight each term; [`EXCESS_COEFFICIENT`]/[`DISJOINT_COEFFICIENT`]/
    /// [`WEIGHT_DIFF_COEFFICIENT`] are the standard NEAT defaults.
    pub fn compatibility_distance(&self, other: &Cppn, c1: f32, c2: f32, c3: f32) -> f32 {
        if self.connections.is_empty() && other.connections.is_empty() {
            return 0.0;
        }

        let self_by_innovation: std::collections::HashMap<usize, &CppnConnection> =
            self.connections.iter().map(|c| (c.innovation, c)).collect();
        let other_by_innovation: std::collections::HashMap<usize, &CppnConnection> = other
            .connections
            .iter()
            .map(|c| (c.innovation, c))
            .collect();

        let self_max = self.connections.iter().map(|c| c.innovation).max();
        let other_max = other.connections.iter().map(|c| c.innovation).max();
        let lower_max = match (self_max, other_max) {
            (Some(a), Some(b)) => a.min(b),
            _ => 0,
        };

        let mut excess = 0u32;
        let mut disjoint = 0u32;
        let mut matching = 0u32;
        let mut weight_diff_sum = 0.0f32;

        let all_innovations: std::collections::HashSet<usize> = self_by_innovation
            .keys()
            .chain(other_by_innovation.keys())
            .copied()
            .collect();
        for innov in all_innovations {
            match (
                self_by_innovation.get(&innov),
                other_by_innovation.get(&innov),
            ) {
                (Some(a), Some(b)) => {
                    matching += 1;
                    weight_diff_sum += (a.weight - b.weight).abs();
                }
                (Some(_), None) | (None, Some(_)) => {
                    if innov > lower_max {
                        excess += 1;
                    } else {
                        disjoint += 1;
                    }
                }
                (None, None) => unreachable!("innovation drawn from the union of both keysets"),
            }
        }

        let n = (self.connections.len().max(other.connections.len())).max(1) as f32;
        let avg_weight_diff = if matching > 0 {
            weight_diff_sum / matching as f32
        } else {
            0.0
        };

        c1 * excess as f32 / n + c2 * disjoint as f32 / n + c3 * avg_weight_diff
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
    ///
    /// Draws from the caller-supplied `rng` rather than a global source, so
    /// the same seed always produces the same mutation (see `common::SimRng`
    /// for the determinism policy this supports).
    pub fn mutate_weight<R: rand::Rng>(&mut self, rng: &mut R) {
        if self.connections.is_empty() {
            return;
        }
        let idx = rng.gen_range(0..self.connections.len());
        let delta = rng.gen::<f32>() - 0.5;
        self.connections[idx].weight += delta;
    }

    /// Adds a random connection between two unconnected nodes.
    ///
    /// Draws from the caller-supplied `rng` rather than a global source —
    /// see [`mutate_weight`](Self::mutate_weight)'s doc comment.
    pub fn mutate_add_connection<R: rand::Rng>(
        &mut self,
        next_innovation: &mut usize,
        rng: &mut R,
    ) {
        if self.nodes.len() < 2 {
            return;
        }

        for _ in 0..10 {
            let src = rng.gen_range(0..self.nodes.len());
            let tgt = rng.gen_range(0..self.nodes.len());

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
                    weight: (rng.gen::<f32>() - 0.5) * 2.0,
                    enabled: true,
                    innovation: *next_innovation,
                    mutation_rate: DEFAULT_MUTATION_RATE,
                });
                *next_innovation += 1;
                break;
            }
        }
    }

    /// Splits a connection and inserts a new hidden node.
    ///
    /// Draws from the caller-supplied `rng` rather than a global source —
    /// see [`mutate_weight`](Self::mutate_weight)'s doc comment.
    pub fn mutate_add_node<R: rand::Rng>(&mut self, next_innovation: &mut usize, rng: &mut R) {
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

        let idx = enabled_indices[rng.gen_range(0..enabled_indices.len())];
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

        // Both halves of a split connection inherit the original locus's
        // mutation rate — a gene duplication carries its parent locus's
        // volatility with it, rather than resetting to the default.
        self.connections.push(CppnConnection {
            source: conn.source,
            target: new_node_idx,
            weight: 1.0,
            enabled: true,
            innovation: *next_innovation,
            mutation_rate: conn.mutation_rate,
        });
        *next_innovation += 1;

        self.connections.push(CppnConnection {
            source: new_node_idx,
            target: conn.target,
            weight: conn.weight,
            enabled: true,
            innovation: *next_innovation,
            mutation_rate: conn.mutation_rate,
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

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    /// A minimal two-node, one-connection CPPN — just enough structure for
    /// `mutate_add_connection` (needs >=2 nodes) and `mutate_add_node`
    /// (needs >=1 connection) to actually do something, rather than hitting
    /// their early-return guards.
    fn sample_cppn() -> Cppn {
        Cppn {
            nodes: vec![
                CppnNode {
                    activation: brain::ActivationFn::Linear,
                    bias: 0.0,
                    layer: 0,
                },
                CppnNode {
                    activation: brain::ActivationFn::Tanh,
                    bias: 0.0,
                    layer: 1,
                },
            ],
            connections: vec![CppnConnection {
                source: 0,
                target: 1,
                weight: 0.5,
                enabled: true,
                innovation: 0,
                mutation_rate: DEFAULT_MUTATION_RATE,
            }],
        }
    }

    #[test]
    fn mutate_weight_is_deterministic_for_same_seed() {
        let mut a = sample_cppn();
        let mut b = sample_cppn();
        a.mutate_weight(&mut ChaCha8Rng::seed_from_u64(7));
        b.mutate_weight(&mut ChaCha8Rng::seed_from_u64(7));
        assert_eq!(a, b);
    }

    #[test]
    fn mutate_add_connection_is_deterministic_for_same_seed() {
        let mut a = sample_cppn();
        let mut b = sample_cppn();
        let mut next_innovation_a = 10;
        let mut next_innovation_b = 10;
        a.mutate_add_connection(&mut next_innovation_a, &mut ChaCha8Rng::seed_from_u64(3));
        b.mutate_add_connection(&mut next_innovation_b, &mut ChaCha8Rng::seed_from_u64(3));
        assert_eq!(a, b);
        assert_eq!(next_innovation_a, next_innovation_b);
    }

    #[test]
    fn mutate_add_node_is_deterministic_for_same_seed() {
        let mut a = sample_cppn();
        let mut b = sample_cppn();
        let mut next_innovation_a = 10;
        let mut next_innovation_b = 10;
        a.mutate_add_node(&mut next_innovation_a, &mut ChaCha8Rng::seed_from_u64(11));
        b.mutate_add_node(&mut next_innovation_b, &mut ChaCha8Rng::seed_from_u64(11));
        assert_eq!(a, b);
        assert_eq!(next_innovation_a, next_innovation_b);
    }

    #[test]
    fn mutate_weight_diverges_across_different_seeds() {
        let mut a = sample_cppn();
        let mut b = sample_cppn();
        a.mutate_weight(&mut ChaCha8Rng::seed_from_u64(1));
        b.mutate_weight(&mut ChaCha8Rng::seed_from_u64(2));
        assert_ne!(a, b);
    }

    #[test]
    fn compatibility_distance_is_zero_for_identical_cppns() {
        let a = sample_cppn();
        let b = sample_cppn();
        assert_eq!(
            a.compatibility_distance(
                &b,
                EXCESS_COEFFICIENT,
                DISJOINT_COEFFICIENT,
                WEIGHT_DIFF_COEFFICIENT
            ),
            0.0
        );
    }

    #[test]
    fn compatibility_distance_grows_with_weight_difference() {
        let mut a = sample_cppn();
        let mut b = sample_cppn();
        a.connections[0].weight = 0.0;
        b.connections[0].weight = 5.0;
        let d = a.compatibility_distance(
            &b,
            EXCESS_COEFFICIENT,
            DISJOINT_COEFFICIENT,
            WEIGHT_DIFF_COEFFICIENT,
        );
        assert!((d - WEIGHT_DIFF_COEFFICIENT * 5.0).abs() < 1e-6);
    }

    #[test]
    fn compatibility_distance_counts_excess_genes() {
        let a = sample_cppn();
        let mut b = sample_cppn();
        // b has an extra connection beyond a's highest innovation number —
        // this is an excess gene, not disjoint (nothing in a exceeds it).
        b.connections.push(CppnConnection {
            source: 0,
            target: 1,
            weight: 1.0,
            enabled: true,
            innovation: 99,
            mutation_rate: DEFAULT_MUTATION_RATE,
        });
        let d = a.compatibility_distance(
            &b,
            EXCESS_COEFFICIENT,
            DISJOINT_COEFFICIENT,
            WEIGHT_DIFF_COEFFICIENT,
        );
        assert!(d > 0.0);
    }

    #[test]
    fn compatibility_distance_is_symmetric() {
        let mut a = sample_cppn();
        let mut b = sample_cppn();
        a.connections[0].weight = -1.0;
        b.connections.push(CppnConnection {
            source: 0,
            target: 1,
            weight: 2.0,
            enabled: true,
            innovation: 7,
            mutation_rate: DEFAULT_MUTATION_RATE,
        });
        let ab = a.compatibility_distance(
            &b,
            EXCESS_COEFFICIENT,
            DISJOINT_COEFFICIENT,
            WEIGHT_DIFF_COEFFICIENT,
        );
        let ba = b.compatibility_distance(
            &a,
            EXCESS_COEFFICIENT,
            DISJOINT_COEFFICIENT,
            WEIGHT_DIFF_COEFFICIENT,
        );
        assert!((ab - ba).abs() < 1e-6);
    }
}
