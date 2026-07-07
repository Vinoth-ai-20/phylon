//! # Phylon Genetics
//!
//! Genome representation, mutation operators, and crossover.
//!
//! The genome is the heritable blueprint of an organism. Contrary to this
//! module's original design intent, it is **not** a bitstring — it's three
//! independent [`Cppn`] graphs (neural wiring, body morphology, and — as of
//! Phase 3, M1 — a gene-regulatory-network generator). As of Phase 3 M4, the
//! explicit `HoxSequence` direct-lookup body plan is retired entirely (see
//! `PHASE3_ROADMAP.md`'s ADR-P3-02): segment identity, branching, actuation,
//! and pigmentation are all decoded from the regulatory network at a body
//! position — see [`develop::develop_at_position`]. See [`Genome`]'s doc
//! comment for the full structure, including diploid second-allele support.
//!
//! All stochastic operations (mutation, crossover) take a caller-supplied
//! `rand::Rng` — see `common::SimRng` for why a fresh, unseeded RNG is never
//! used here.
//!
//! Each [`cppn::CppnConnection`] carries its own evolvable `mutation_rate`
//! (self-adaptive: it drifts slightly on every mutation pass and is
//! inherited, like the weight itself, through crossover and connection
//! splitting) — this is genuine per-locus mutation control, not a single
//! genome-wide constant.
//!
//! ## Not yet implemented
//!
//! Epigenetic markers, horizontal gene transfer, and non-disjunction are
//! all named in the original spec but have no code here yet.

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

/// Gene Regulatory Network runtime (Phase 3, M1) — see `PHASE3_ROADMAP.md`'s
/// ADR-P3-01 for why this is a third evolvable `Cppn` plus a small recurrent
/// runtime network, not a new execution engine.
pub mod regulatory;
pub use regulatory::{
    RegulatoryEdge, RegulatoryGeneNode, RegulatoryGeneRole, RegulatoryNetwork,
    REGULATORY_GENE_ROLES,
};

/// Analytic morphogen gradients (Phase 3, M3) — closed-form positional
/// inputs to a `RegulatoryNetwork`, see `PHASE3_ROADMAP.md`'s ADR-P3-03.
pub mod morphogen;
pub use morphogen::{ap_position, distance_from_head_gradient, external_inputs_for_position};

/// Positional decode of a `RegulatoryNetwork` into segment identity,
/// branching, actuation, and pigmentation (Phase 3, M4) — see
/// `PHASE3_ROADMAP.md`'s ADR-P3-02.
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
