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
/// Each body position is assigned one of these 8 variants by decoding a
/// 3-bit combinatorial "Hox code" — a short bit-string read off three
/// designated genes in the regulatory network, named after the biological
/// Hox gene family that plays the analogous body-segment-identity role in
/// real animals — via `genetics::decode_segment_type`. There are exactly 8
/// variants because a 3-bit code has exactly 8 possible values and every
/// value should map to a distinct, meaningful identity rather than
/// wrapping back onto an earlier variant.
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
    /// Transport/circulatory tissue: lower stiffness than `Torso`, and a
    /// `Passive` physics constraint rather than an actuated one.
    Vascular,
    /// Neural-cluster tissue, a precursor to a centralized nervous
    /// structure. Currently an enum-only placeholder — see
    /// `organisms::brain_wiring` for how brain topology is actually wired
    /// today; dedicated `Ganglion` behavior is a future extension.
    Ganglion,
    /// Germ-line/reproductive tissue. Unconditionally protected from
    /// developmental apoptosis — see `genetics::decode_apoptosis` — mirroring
    /// real biology's germ-soma separation (germ-line cells are shielded
    /// from the programmed cell death that shapes the rest of the body).
    Germinal,
}
