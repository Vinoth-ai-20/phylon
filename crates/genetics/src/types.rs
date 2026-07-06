use serde::{Deserialize, Serialize};

/// A unique identifier for a genome sequence.
///
/// Distinct from [`common::EntityId`] because multiple organisms can share the same
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
///
/// Phase 3 M5 broadened this from 5 to 8 variants so all 8 codes a 3-gene
/// Hox combinatorial decode can produce (`genetics::decode_segment_type`)
/// map to a distinct identity, instead of wrapping back onto `Head`/`Torso`/
/// `Muscle` via modulo. `Vascular` (M9) and `Germinal` (M8, via apoptosis
/// protection — see `genetics::decode_apoptosis`) now have differentiated
/// behavior; `Ganglion`'s neural-centralization behavior remains an enum-only
/// placeholder, deferred to the M5 stretch goal (M14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// Transport/circulatory tissue (Phase 3 M5; DEF-003's differentiated
    /// physics — lower stiffness, `Passive` constraint — wired in M9).
    Vascular,
    /// Neural-cluster tissue, a precursor to centralized nervous structure
    /// (Phase 3 M5; differentiated behavior is DEF-003's neural
    /// centralization half, deferred to the M5 stretch goal, M14).
    Ganglion,
    /// Germ-line/reproductive tissue (Phase 3 M5; DEF-002's germ-soma
    /// separation — unconditional protection from developmental apoptosis —
    /// wired in M8, see `genetics::decode_apoptosis`).
    Germinal,
}
