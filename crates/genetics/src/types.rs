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
/// `Muscle` via modulo. The three new variants are enum-only placeholders
/// for now — `organisms::growth_system` gives them a physically reasonable
/// default (stiffness, constraint type), but their differentiated *behavior*
/// (vascular transport, neural centralization, germ-line protection) is
/// each its own later milestone (DEF-003 → M9, DEF-002 → M8), not this one's.
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
    /// Transport/circulatory tissue (Phase 3 M5; differentiated behavior is
    /// DEF-003, deferred to M9).
    Vascular,
    /// Neural-cluster tissue, a precursor to centralized nervous structure
    /// (Phase 3 M5; differentiated behavior is DEF-003's neural
    /// centralization half, deferred to the M5 stretch goal, M14).
    Ganglion,
    /// Germ-line/reproductive tissue (Phase 3 M5; differentiated behavior —
    /// germ-soma separation — is DEF-002, deferred to M8).
    Germinal,
}
