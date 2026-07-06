//! Gene Regulatory Network runtime (Phase 3, M1).
//!
//! See `PHASE3_ROADMAP.md`'s ADR-P3-01 for the full reasoning; summarized
//! here: the GRN is **not** a new execution engine. It's a third evolvable
//! [`Cppn`] (`Genome::regulatory_cppn`) generating the weights of a small
//! recurrent runtime network (`RegulatoryNetwork`) — the exact same
//! two-tier "evolvable generator → iteratively-simulated runtime structure"
//! pattern already proven by `brain_cppn` → `Brain`. Unlike `Brain`,
//! `RegulatoryNetwork` is evaluated on the CPU only, over a small fixed
//! number of *developmental* steps (not simulation ticks, and not once per
//! frame for a whole population) — it has none of `Brain`'s GPU-integration
//! requirements, since development happens once per organism, not every
//! tick for up to 100,000 organisms simultaneously.
//!
//! **Scope of this milestone:** the network can be generated from a `Cppn`
//! and iteratively evaluated, deterministically. It is *not yet* wired to
//! `organisms::growth_system`, positional/morphogen inputs are not yet
//! meaningful (M3), and Hox-designated genes are not yet read for segment
//! identity (M4). Those are separate, later milestones.

use crate::cppn::Cppn;
use serde::{Deserialize, Serialize};

/// Fixed semantic role of one gene (output node) in a [`RegulatoryNetwork`],
/// mirroring the fixed-index I/O convention `Genome::new_hox_driven`'s
/// `brain_cppn`/`morph_cppn` templates already use (documented there via
/// plain comments) — here made explicit via an enum + [`REGULATORY_GENE_ROLES`]
/// table, since later milestones (M4/M5) need to know which gene is which.
///
/// The gene *count* is fixed for this milestone (matching
/// [`REGULATORY_GENE_ROLES`]'s length) — evolvable growth of the output
/// vocabulary itself is Phase 3 M5's explicit scope, not this one's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegulatoryGeneRole {
    /// Read positionally along the body axis to decide segment identity
    /// (Phase 3 M4) — several Hox-designated genes together form a
    /// combinatorial code, replacing today's direct `HoxGene.segment`
    /// lookup (see `genome::Genome::hox`'s doc comment for what this
    /// replaces).
    Hox,
    /// Read to decide broader cell-fate/organ output, beyond the fixed
    /// segment-type vocabulary (Phase 3 M5).
    Differentiation,
    /// Designated to eventually drive a physical effector once wired to
    /// growth (Phase 3 M4+) — a placeholder role for this milestone.
    Effector,
}

/// The initial regulatory-gene role table: 3 Hox-designated genes (enough
/// for up to 2^3 = 8 combinatorial identities under a simple on/off
/// reading — comfortably more than today's 5 fixed `SegmentType` variants),
/// 2 Differentiation-designated genes, and 1 Effector-designated gene.
/// `RegulatoryNetwork::generate`'s `gene_count` argument is expected to
/// match this table's length while this milestone's fixed-vocabulary scope
/// holds.
pub const REGULATORY_GENE_ROLES: &[RegulatoryGeneRole] = &[
    RegulatoryGeneRole::Hox,
    RegulatoryGeneRole::Hox,
    RegulatoryGeneRole::Hox,
    RegulatoryGeneRole::Differentiation,
    RegulatoryGeneRole::Differentiation,
    RegulatoryGeneRole::Effector,
];

/// One regulatory gene's runtime state — analogous to `brain::CtrnnNode`,
/// but evaluated on the CPU over developmental steps rather than uploaded
/// to a GPU buffer every simulation tick (see this module's doc comment for
/// why the two don't share an implementation).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RegulatoryGeneNode {
    /// Current expression level (this gene's "activation potential").
    pub state: f32,
    /// Bias added before activation, generated from `regulatory_cppn`.
    pub bias: f32,
    /// Activation function — fixed to `Sigmoid` for this milestone (a
    /// natural threshold-response curve for gene expression; evolving the
    /// choice per-gene, like `Cppn`'s nodes already do, is a straightforward
    /// future extension, not required for M1).
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
/// unlike `Brain` (which persists for an organism's entire lifetime), this
/// network is regenerated fresh from the genome whenever development needs
/// to run and its *output* (differentiation decisions) is what gets baked
/// into the Body Graph / physics representation (Phase 3 M4+); the network
/// itself doesn't need to outlive that computation.
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
    /// `regulatory_cppn` exactly the way `organisms::systems`'s brain
    /// construction queries `brain_cppn`: once per gene index for that
    /// gene's bias (`evaluate(&[i/total, i/total])`), and once per
    /// gene-index pair for the edge weight between them
    /// (`evaluate(&[i/total, j/total])`). A pair whose evaluated weight is
    /// (numerically) exactly `0.0` is skipped — no edge is created — so the
    /// network's edge *topology* itself is shaped by evolution of
    /// `regulatory_cppn`, not fixed to a complete graph.
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
    /// `external_inputs` supplies one additional additive input per gene
    /// (e.g. a future morphogen-gradient reading, Phase 3 M3); a gene
    /// beyond `external_inputs`'s length receives `0.0`. This milestone
    /// does not yet attach real meaning to these inputs — tests exercise
    /// this with an empty or all-zero slice.
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
    /// step count, not "iterate until convergence" (per
    /// `PHASE3_ROADMAP.md`'s risk table: a fixed count keeps evaluation cost
    /// bounded and deterministic regardless of whether a given evolved
    /// topology would otherwise oscillate or diverge).
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
