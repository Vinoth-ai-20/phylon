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

/// The raw genome of an organism.
///
/// Stored as a byte vector in base-4 encoding (2 bits per base).
/// Each locus corresponds to a parameter defined by the HOX mapping table.
///
/// TODO(phase-5): Implement full genome encoding, HOX map, and mutation operators.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Genome {
    /// Unique identifier for this genome sequence.
    pub id: GenomeId,
    /// The ID of the organism that created this genome (for lineage tracking).
    pub origin: EntityId,
    /// Ploidy level (haploid or diploid).
    pub ploidy: Ploidy,
    /// Raw genome data in base-4 encoding (2 bits per locus).
    pub data: Vec<u8>,
}

impl Genome {
    /// Creates a minimal placeholder genome with all-zero data.
    ///
    /// TODO(phase-5): Replace with proper genome seeding from RNG.
    pub fn placeholder(id: GenomeId, origin: EntityId, length_bytes: usize) -> Self {
        Self {
            id,
            origin,
            ploidy: Ploidy::Haploid,
            data: vec![0u8; length_bytes],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_genome_has_correct_length() {
        let g = Genome::placeholder(GenomeId(1), EntityId(0), 64);
        assert_eq!(g.data.len(), 64);
    }

    #[test]
    fn genome_id_equality() {
        assert_eq!(GenomeId(1), GenomeId(1));
        assert_ne!(GenomeId(1), GenomeId(2));
    }
}
