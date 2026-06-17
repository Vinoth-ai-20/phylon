//! # Phylon Metabolism
//!
//! Energy management, ageing, aerobic/anaerobic respiration, starvation
//! cascade, and hunger drive systems.
//!
//! ## Phase 0 scope
//!
//! Type declarations and constants. Implementation: Phase 3.

#![warn(missing_docs)]
#![warn(clippy::all)]

use common::SimEnergy;

/// The base energy cost of simply existing for one tick.
///
/// TODO(phase-3): Load from `SimulationConfig` instead of using a constant.
pub const BASE_METABOLIC_COST: SimEnergy = SimEnergy(0.01);

/// Respiration mode of an organism.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RespirationMode {
    /// Standard aerobic respiration — requires oxygen field availability.
    Aerobic,
    /// Anaerobic fallback — less efficient but functions without oxygen.
    Anaerobic,
}

/// Placeholder for the metabolism system.
///
/// TODO(phase-3): Implement per-organism energy tick, hunger calculation,
/// and starvation event emission.
pub struct MetabolismSystem;

impl MetabolismSystem {
    /// Creates a new metabolism system.
    pub fn new() -> Self {
        Self
    }
}

impl Default for MetabolismSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metabolic_cost_is_positive() {
        assert!(BASE_METABOLIC_COST.0 > 0.0);
    }
}
