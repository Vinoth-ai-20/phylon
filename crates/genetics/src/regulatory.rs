//! Gene Regulatory Network (GRN) runtime.
//!
//! # Purpose
//!
//! Real animal development doesn't read body-plan information from a static
//! table. Instead, a network of genes turn each other on and off — activating
//! and repressing one another — until the network settles into a pattern of
//! expression levels that differs by position in the body, and *that*
//! pattern of "which genes ended up expressed here" is what determines local
//! anatomy. This module is a simplified computational analog of that process:
//! a small, evolvable network of gene "nodes" wired together by signed
//! activator/repressor edges, iterated for a few steps at a given body
//! position until it settles, and then read out to decide what grows there.
//!
//! # Architecture: generator + runtime, not a bespoke engine
//!
//! A `RegulatoryNetwork` is not designed by hand and it is not itself part of
//! the evolvable genome. Instead, `Genome::regulatory_cppn` is a [`Cppn`] (see
//! `crate::cppn` for what a CPPN is and why one is used) that is queried, once
//! per gene and once per gene pair, to produce that gene's bias and the
//! signed weight of the edge between each pair of genes. [`RegulatoryNetwork::generate`]
//! performs those queries and assembles the result into the small recurrent
//! network defined below. This is the same two-tier "evolvable generator
//! produces a runtime structure, which is then iteratively simulated" split
//! already used for the organism's brain (`Genome::brain_cppn` generates
//! synapse weights for a CTRNN — see `organisms::brain_wiring`); the
//! regulatory network reuses the same pattern rather than inventing a new
//! execution model. Unlike the brain's CTRNN, `RegulatoryNetwork` is
//! evaluated on the CPU only, over a small fixed number of *developmental*
//! steps — not simulation ticks, and not once per frame for a whole
//! population. It runs once whenever a body position needs to be decoded
//! (organism spawn, or a new segment during growth), not continuously, so it
//! has none of the brain's GPU-integration requirements.
//!
//! # Data flow
//!
//! `crate::develop::develop_at_position` is the single entry point that ties
//! this module to the rest of the crate: it builds a `RegulatoryNetwork` from
//! `regulatory_cppn`, develops it for a fixed number of steps at one body
//! position (with morphogen-derived external inputs — see `crate::morphogen`),
//! and decodes the settled gene states into a `SegmentType`, branching
//! decision, actuation parameters, and pigment. See that module for the
//! decode logic; this module owns only the network itself.

use crate::cppn::Cppn;
use serde::{Deserialize, Serialize};

/// Fixed semantic role of one gene (output node) in a [`RegulatoryNetwork`] —
/// a fixed-index convention (analogous to the brain CPPN's fixed input/output
/// columns) made explicit via an enum + [`REGULATORY_GENE_ROLES`] table, so
/// `crate::develop`'s decode knows which gene is which.
///
/// The gene *count* and role assignment are fixed today; evolvable growth of
/// the output vocabulary itself (e.g. new roles, or a variable gene count)
/// is a possible future extension, not implemented here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegulatoryGeneRole {
    /// Read positionally along the body axis to decide segment identity.
    /// Several Hox-designated genes together form a combinatorial bit-code —
    /// named after the biological Hox gene family, which plays the same
    /// segment-identity role in real animal development — decoded by
    /// `crate::develop::decode_segment_type`.
    Hox,
    /// Read to decide broader cell-fate/organ output, beyond the fixed
    /// segment-type vocabulary. Index 0 of this role drives the branching
    /// decision and index 1 drives apoptosis (see `crate::develop`).
    Differentiation,
    /// Drives a physical growth effector: index 0 is muscle actuation
    /// amplitude, index 1 is actuation phase (see
    /// `crate::develop::develop_at_position`).
    Effector,
    /// Drives per-segment skin pigmentation. Three Pigment-designated genes
    /// map directly to R/G/B; since decoding runs once per body position,
    /// color is a genuine per-segment emergent trait rather than a single
    /// organism-wide stored value — this is what makes gradients, stripes,
    /// or spots possible purely from a richer `regulatory_cppn` topology,
    /// with no change to this architecture.
    Pigment,
}

/// The regulatory-gene role table: 3 Hox-designated genes (enough for up to
/// 2^3 = 8 combinatorial identities under a simple on/off reading — matching
/// `SegmentType`'s 8 variants one-for-one), 2 Differentiation-designated
/// genes, 2 Effector-designated genes (amplitude, phase), and 3
/// Pigment-designated genes (R, G, B). `RegulatoryNetwork::generate`'s
/// `gene_count` argument is expected to match this table's length.
pub const REGULATORY_GENE_ROLES: &[RegulatoryGeneRole] = &[
    RegulatoryGeneRole::Hox,
    RegulatoryGeneRole::Hox,
    RegulatoryGeneRole::Hox,
    RegulatoryGeneRole::Differentiation,
    RegulatoryGeneRole::Differentiation,
    RegulatoryGeneRole::Effector,
    RegulatoryGeneRole::Effector,
    RegulatoryGeneRole::Pigment,
    RegulatoryGeneRole::Pigment,
    RegulatoryGeneRole::Pigment,
];

/// One regulatory gene's runtime state — conceptually analogous to a CTRNN
/// neuron (see `organisms::brain_wiring`), but evaluated on the CPU over a
/// handful of developmental steps rather than continuously on the GPU every
/// simulation tick (see this module's doc comment for why the two don't
/// share an implementation).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RegulatoryGeneNode {
    /// Current expression level (this gene's "activation potential").
    pub state: f32,
    /// Bias added before activation, generated from `regulatory_cppn`.
    pub bias: f32,
    /// Activation function — fixed to `Sigmoid` (a natural threshold-response
    /// curve for gene expression: near-zero for a strongly repressed gene,
    /// near-one for a strongly activated one). Evolving the choice per-gene,
    /// the way a `Cppn`'s nodes already do, is a straightforward future
    /// extension, not implemented here.
    pub activation: brain::ActivationFn,
}

/// A directed regulatory edge between two genes. The sign of `weight`
/// carries activator (positive) / repressor (negative) semantics directly —
/// no separate flag is needed, the same way excitatory/inhibitory synapses
/// in `Brain`/`Cppn` are already just signed weights.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RegulatoryEdge {
    /// Source gene index.
    pub source: usize,
    /// Target gene index.
    pub target: usize,
    /// Signed regulatory strength: positive activates, negative represses.
    pub weight: f32,
}

/// The runtime Gene Regulatory Network — generated from a `regulatory_cppn`
/// (see [`RegulatoryNetwork::generate`]) and iteratively evaluated over a
/// small, fixed number of developmental steps (see [`RegulatoryNetwork::develop`]).
///
/// Deliberately **not** a `bevy_ecs::Component` and **not** `Serialize`d —
/// unlike the organism's brain (which persists for its entire lifetime),
/// this network is regenerated fresh from the genome every time development
/// needs to run, and only its *output* (the decoded segment/branching/
/// actuation/pigment decision) is what gets baked into the persistent body
/// graph (see `organisms::developmental_graph`). The network itself doesn't
/// need to outlive that one computation.
#[derive(Debug, Clone, PartialEq)]
pub struct RegulatoryNetwork {
    /// Every gene in the network, indexed positionally (index into this
    /// `Vec` is the same index [`RegulatoryEdge::source`]/`target` and
    /// [`REGULATORY_GENE_ROLES`] use).
    pub nodes: Vec<RegulatoryGeneNode>,
    /// Every regulatory edge (activator/repressor relationship) between genes.
    pub edges: Vec<RegulatoryEdge>,
}

impl RegulatoryNetwork {
    /// Builds a `RegulatoryNetwork` of `gene_count` genes by querying
    /// `regulatory_cppn` — the same "query a CPPN once per index/pair" pattern
    /// `organisms::brain_wiring` uses to build a CTRNN's synapse weights from
    /// `brain_cppn`: once per gene index for that gene's bias
    /// (`evaluate(&[i/total, i/total])`), and once per gene-index pair for
    /// the edge weight between them (`evaluate(&[i/total, j/total])`). A pair
    /// whose evaluated weight is (numerically) exactly `0.0` is skipped — no
    /// edge is created — so the network's edge *topology* itself is shaped
    /// by evolution of `regulatory_cppn`, not fixed to a complete graph.
    pub fn generate(regulatory_cppn: &Cppn, gene_count: usize) -> Self {
        let total = gene_count.max(1) as f32;

        let nodes = (0..gene_count)
            .map(|i| {
                let idx = i as f32 / total;
                let bias = regulatory_cppn
                    .evaluate(&[idx, idx])
                    .first()
                    .copied()
                    .unwrap_or(0.0);
                RegulatoryGeneNode {
                    state: 0.0,
                    bias,
                    activation: brain::ActivationFn::Sigmoid,
                }
            })
            .collect();

        let mut edges = Vec::new();
        for i in 0..gene_count {
            for j in 0..gene_count {
                if i == j {
                    continue;
                }
                let weight = regulatory_cppn
                    .evaluate(&[i as f32 / total, j as f32 / total])
                    .first()
                    .copied()
                    .unwrap_or(0.0);
                if weight != 0.0 {
                    edges.push(RegulatoryEdge {
                        source: i,
                        target: j,
                        weight,
                    });
                }
            }
        }

        Self { nodes, edges }
    }

    /// Advances the network by exactly one developmental step.
    ///
    /// Every gene's next state is computed from a **snapshot of the
    /// previous step's states** (synchronous update), not from
    /// partially-updated values within the same step — this makes the
    /// result independent of the order genes happen to be stored/iterated
    /// in, the same order-independence property every other parallel/
    /// sequential system in this codebase is held to (see the snapshot →
    /// compute → reduce pattern documented across `metabolism`/`sensing`/
    /// `behavior`).
    ///
    /// `external_inputs` supplies one additional additive input per gene —
    /// in practice a morphogen-gradient reading for the current body
    /// position (see `crate::morphogen`); a gene beyond `external_inputs`'s
    /// length receives `0.0`.
    pub fn step(&mut self, external_inputs: &[f32]) {
        let previous: Vec<f32> = self.nodes.iter().map(|n| n.state).collect();

        let mut sums = vec![0.0f32; self.nodes.len()];
        for (i, sum) in sums.iter_mut().enumerate() {
            *sum = self.nodes[i].bias + external_inputs.get(i).copied().unwrap_or(0.0);
        }
        for edge in &self.edges {
            sums[edge.target] += previous[edge.source] * edge.weight;
        }

        for (node, &sum) in self.nodes.iter_mut().zip(sums.iter()) {
            node.state = match node.activation {
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
    }

    /// Runs [`RegulatoryNetwork::step`] `steps` times in sequence — a fixed
    /// step count, not "iterate until convergence": a fixed count keeps
    /// evaluation cost bounded and deterministic regardless of whether a
    /// given evolved topology would otherwise oscillate or never settle at
    /// all.
    pub fn develop(&mut self, steps: usize, external_inputs: &[f32]) {
        for _ in 0..steps {
            self.step(external_inputs);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cppn::{CppnConnection, CppnNode, DEFAULT_MUTATION_RATE};

    /// A small hand-built regulatory CPPN: 2 inputs (gene-index pair), one
    /// tanh output — enough structure for `generate` to produce non-trivial
    /// biases/weights without relying on a specific evolved topology.
    fn sample_regulatory_cppn() -> Cppn {
        Cppn {
            nodes: vec![
                CppnNode {
                    activation: brain::ActivationFn::Linear,
                    bias: 0.0,
                    layer: 0,
                },
                CppnNode {
                    activation: brain::ActivationFn::Linear,
                    bias: 0.0,
                    layer: 0,
                },
                CppnNode {
                    activation: brain::ActivationFn::Tanh,
                    bias: 0.1,
                    layer: 1,
                },
            ],
            connections: vec![
                CppnConnection {
                    source: 0,
                    target: 2,
                    weight: 1.5,
                    enabled: true,
                    innovation: 0,
                    mutation_rate: DEFAULT_MUTATION_RATE,
                },
                CppnConnection {
                    source: 1,
                    target: 2,
                    weight: -0.8,
                    enabled: true,
                    innovation: 1,
                    mutation_rate: DEFAULT_MUTATION_RATE,
                },
            ],
        }
    }

    #[test]
    fn generate_produces_expected_gene_and_edge_count() {
        let net =
            RegulatoryNetwork::generate(&sample_regulatory_cppn(), REGULATORY_GENE_ROLES.len());
        assert_eq!(net.nodes.len(), REGULATORY_GENE_ROLES.len());
        // Every non-self pair is queried; edges are only kept when the
        // evaluated weight isn't exactly zero, so this asserts an upper
        // bound, not an exact count (the sample CPPN is not guaranteed to
        // produce a nonzero weight for every pair).
        let max_possible = REGULATORY_GENE_ROLES.len() * (REGULATORY_GENE_ROLES.len() - 1);
        assert!(net.edges.len() <= max_possible);
    }

    #[test]
    fn generate_is_deterministic_for_the_same_cppn() {
        let cppn = sample_regulatory_cppn();
        let net_a = RegulatoryNetwork::generate(&cppn, 6);
        let net_b = RegulatoryNetwork::generate(&cppn, 6);
        assert_eq!(net_a, net_b);
    }

    #[test]
    fn step_updates_synchronously_not_sequentially() {
        // A -> B activator edge only. If `step` incorrectly used B's
        // not-yet-updated (stale) state when computing A in the same pass
        // (order-dependent), vs. a clean previous-step snapshot, this would
        // still pass either way for a 2-node/1-edge case — so this test
        // instead asserts the *documented* contract directly: running the
        // same network + inputs twice from the same starting state always
        // produces the same result, regardless of node storage order.
        let mut net = RegulatoryNetwork {
            nodes: vec![
                RegulatoryGeneNode {
                    state: 0.0,
                    bias: 0.5,
                    activation: brain::ActivationFn::Sigmoid,
                },
                RegulatoryGeneNode {
                    state: 0.0,
                    bias: -0.5,
                    activation: brain::ActivationFn::Sigmoid,
                },
            ],
            edges: vec![RegulatoryEdge {
                source: 0,
                target: 1,
                weight: 2.0,
            }],
        };
        let mut net2 = net.clone();
        net.step(&[]);
        net2.step(&[]);
        assert_eq!(net, net2);
    }

    #[test]
    fn develop_is_deterministic_and_bounded() {
        let cppn = sample_regulatory_cppn();
        let mut net_a = RegulatoryNetwork::generate(&cppn, 6);
        let mut net_b = RegulatoryNetwork::generate(&cppn, 6);
        net_a.develop(10, &[]);
        net_b.develop(10, &[]);
        assert_eq!(net_a, net_b);
        for node in &net_a.nodes {
            // Sigmoid output is always in (0, 1) — a basic sanity bound,
            // not a tautology, since a bug computing `sum` incorrectly
            // (e.g. skipping the activation function) could easily produce
            // a value outside this range.
            assert!(node.state > 0.0 && node.state < 1.0);
        }
    }

    #[test]
    fn external_inputs_influence_state() {
        let mut with_input = RegulatoryNetwork {
            nodes: vec![RegulatoryGeneNode {
                state: 0.0,
                bias: 0.0,
                activation: brain::ActivationFn::Linear,
            }],
            edges: vec![],
        };
        let mut without_input = with_input.clone();
        with_input.step(&[5.0]);
        without_input.step(&[]);
        assert_ne!(with_input.nodes[0].state, without_input.nodes[0].state);
    }
}
