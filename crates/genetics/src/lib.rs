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
}

/// The genome of an organism.
///
/// Holds a sequence of morphological segments that dictate the structural
/// composition of the organism during the procedural growth phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Genome {
    /// Unique identifier for this genome sequence.
    pub id: GenomeId,
    /// The ID of the organism that created this genome (for lineage tracking).
    pub origin: EntityId,
    /// Ploidy level (haploid or diploid).
    pub ploidy: Ploidy,
    /// Morphological sequence of segments.
    pub segments: Vec<SegmentType>,
}

impl Genome {
    /// Creates a new genome with the given segment sequence.
    pub fn new(id: GenomeId, origin: EntityId, segments: Vec<SegmentType>) -> Self {
        Self {
            id,
            origin,
            ploidy: Ploidy::Haploid,
            segments,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_genome_has_correct_segments() {
        let g = Genome::new(
            GenomeId(1),
            EntityId(0),
            vec![SegmentType::Head, SegmentType::Tail],
        );
        assert_eq!(g.segments.len(), 2);
        assert_eq!(g.segments[0], SegmentType::Head);
    }

    #[test]
    fn genome_id_equality() {
        assert_eq!(GenomeId(1), GenomeId(1));
        assert_ne!(GenomeId(1), GenomeId(2));
    }
}
