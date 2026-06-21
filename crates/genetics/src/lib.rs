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

/// Base genetic types and IDs.
pub mod types;
pub use types::{GenomeId, Ploidy, SegmentType};

/// Hox gene definitions for structural sequencing.
pub mod hox;
pub use hox::{HoxGene, HoxSequence};

/// Compositional Pattern Producing Network representation.
pub mod cppn;
pub use cppn::{Cppn, CppnConnection, CppnNode, GlobalInnovationTracker};

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
