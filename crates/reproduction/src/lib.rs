//! # Phylon Reproduction
//!
//! Reproduction strategies, birth event emission, offspring dispersal,
//! and malformed offspring detection.
//!
//! ## Phase 0 scope
//!
//! Strategy enum and placeholder types. Implementation: Phase 5.

#![warn(missing_docs)]
#![warn(clippy::all)]

use common::EntityId;

/// The reproductive strategy an organism can use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReproductionStrategy {
    /// Produces offspring from a single parent (clonal / budding).
    Asexual,
    /// Requires two parents to produce offspring.
    Sexual,
    /// Can reproduce either way depending on mate availability.
    Facultative,
}

/// Placeholder for a pending birth event.
///
/// TODO(phase-5): Implement offspring construction from parent genomes.
#[allow(dead_code)]
pub struct PendingBirth {
    /// The parent organism initiating reproduction.
    parent: EntityId,
    /// Strategy used for this birth.
    strategy: ReproductionStrategy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reproduction_strategy_is_copy() {
        let s = ReproductionStrategy::Sexual;
        let _s2 = s;
    }
}
