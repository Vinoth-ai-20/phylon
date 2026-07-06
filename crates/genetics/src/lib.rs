//! # Phylon Genetics
//!
//! Genome representation, mutation operators, and crossover.
//!
//! The genome is the heritable blueprint of an organism. Contrary to this
//! module's original design intent, it is **not** a bitstring — it's two
//! independent [`Cppn`] graphs (one for neural wiring, one for body
//! morphology) plus an optional explicit [`HoxSequence`] body plan. See
//! [`Genome`]'s doc comment for the full structure, including diploid
//! second-allele support.
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

/// Hox gene definitions for structural sequencing.
pub mod hox;
pub use hox::{HoxGene, HoxSequence};

/// Compositional Pattern Producing Network representation.
pub mod cppn;
pub use cppn::{
    Cppn, CppnConnection, CppnNode, GlobalInnovationTracker, DISJOINT_COEFFICIENT,
    EXCESS_COEFFICIENT, WEIGHT_DIFF_COEFFICIENT,
};

/// The primary Genome container and operations.
pub mod genome;
pub use genome::Genome;

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
