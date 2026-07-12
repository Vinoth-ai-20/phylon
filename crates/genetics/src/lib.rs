//! # Phylon Genetics
//!
//! Genome representation, mutation operators, and crossover.
//!
//! The genome is the heritable blueprint of an organism. It is **not** a
//! bitstring — it's three independent [`Cppn`] graphs: one for neural
//! wiring, one for body morphology, and one that generates a gene
//! regulatory network (a small network of interacting "genes" whose
//! settled activation levels decide what grows where — see [`regulatory`]
//! for the full explanation). There is no separate direct-lookup body-plan
//! table: segment identity, branching, actuation, and pigmentation are all
//! decoded from the regulatory network at a body position — see
//! [`develop::develop_at_position`]. See [`Genome`]'s doc comment for the
//! full structure, including diploid second-allele support.
//!
//! All stochastic operations (mutation, crossover) take a caller-supplied
//! `rand::Rng` — see `common::SimRng` for why a fresh, unseeded RNG is never
//! used here. This keeps every run reproducible from its seed: given the
//! same seed and the same sequence of calls, mutation and crossover always
//! produce byte-identical results.
//!
//! Each [`cppn::CppnConnection`] carries its own evolvable `mutation_rate`
//! (self-adaptive: it drifts slightly on every mutation pass and is
//! inherited, like the weight itself, through crossover and connection
//! splitting) — this is genuine per-locus mutation control, not a single
//! genome-wide constant.
//!
//! ## Not yet implemented
//!
//! Epigenetic markers, horizontal gene transfer, and non-disjunction have
//! no code here yet.

#![warn(missing_docs)]
#![warn(clippy::all)]

/// Base genetic types and IDs.
pub mod types;
pub use types::{GenomeId, Ploidy, SegmentType};

/// Compositional Pattern Producing Network representation.
pub mod cppn;
pub use cppn::{
    Cppn, CppnConnection, CppnNode, GlobalInnovationTracker, DISJOINT_COEFFICIENT,
    EXCESS_COEFFICIENT, WEIGHT_DIFF_COEFFICIENT,
};

/// The primary Genome container and operations.
pub mod genome;
pub use genome::Genome;

/// Gene Regulatory Network runtime: a third evolvable `Cppn` that generates
/// the weights of a small recurrent runtime network, rather than a bespoke
/// execution engine — see the module doc for why this mirrors the
/// brain-wiring `Cppn` → `Brain` pattern used elsewhere in this workspace.
pub mod regulatory;
pub use regulatory::{
    RegulatoryEdge, RegulatoryGeneNode, RegulatoryGeneRole, RegulatoryNetwork,
    REGULATORY_GENE_ROLES,
};

/// Analytic morphogen gradients — closed-form positional inputs to a
/// `RegulatoryNetwork` (a diffused PDE field would be more physically
/// realistic but far more expensive; see the module doc for why a
/// closed-form approximation is a reasonable first model, not just a
/// convenience shortcut).
pub mod morphogen;
pub use morphogen::{ap_position, distance_from_head_gradient, external_inputs_for_position};

/// Positional decode of a `RegulatoryNetwork` into segment identity,
/// branching, actuation, and pigmentation.
pub mod develop;
pub use develop::{
    decode_apoptosis, decode_segment_type, develop_at_position,
    develop_at_position_with_life_stage, hox_states_at_position, DevelopmentalOutputs,
};

#[cfg(test)]
mod tests {
    use super::*;
    use common::EntityId;

    #[test]
    fn new_genome_is_empty() {
        let g = Genome::new_minimal(GenomeId(1), EntityId(0));
        assert_eq!(g.brain_cppn.nodes.len(), 0);
        assert_eq!(g.brain_cppn.connections.len(), 0);
    }

    #[test]
    fn genome_id_equality() {
        assert_eq!(GenomeId(1), GenomeId(1));
        assert_ne!(GenomeId(1), GenomeId(2));
    }
}
