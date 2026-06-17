//! # Phylon Evolution
//!
//! Selection pressure, speciation, lineage tracking, fitness metrics, and
//! hybridization barriers.
//!
//! Evolution in Phylon is **emergent** — there is no explicit fitness function.
//! Survival and reproduction pressure exerted by the ecology system acts as
//! the selection gradient.
//!
//! ## Phase 0 scope
//!
//! Type declarations only. Implementation: Phase 5.

#![warn(missing_docs)]
#![warn(clippy::all)]

use common::EntityId;

/// A lineage identifier linking related organisms across generations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineageId(pub u64);

/// A species cluster identifier assigned by the speciation algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpeciesId(pub u64);

/// Placeholder for the lineage record of a single organism.
///
/// TODO(phase-5): Implement full lineage tree with SQLite persistence.
#[allow(dead_code)]
pub struct LineageRecord {
    /// The entity this record belongs to.
    entity: EntityId,
    /// The lineage cluster this organism belongs to.
    lineage: LineageId,
    /// The species cluster assigned at last speciation check.
    species: SpeciesId,
    /// Generation number (0 for initial population).
    generation: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lineage_id_equality() {
        assert_eq!(LineageId(1), LineageId(1));
        assert_ne!(LineageId(1), LineageId(2));
    }
}
