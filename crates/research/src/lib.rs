//! # Phylon Research
//!
//! Experiment manager, scenario authoring, batch run orchestration,
//! and research report generation.
//!
//! The research crate provides the high-level API for defining, running,
//! and comparing experiments. It coordinates the scheduler (for headless
//! execution), the analytics accumulator, and the storage manager.
//!
//! ## Phase 0 scope
//!
//! Experiment manifest type. Implementation: Phase 9.

#![warn(missing_docs)]
#![warn(clippy::all)]

use common::Tick;

/// The manifest for a single experiment run.
///
/// Recorded at experiment start and stored alongside every snapshot and
/// dataset export so results are always traceable back to their origin
/// conditions.
#[derive(Debug, Clone)]
pub struct ExperimentManifest {
    /// A unique identifier for this experiment (usually a UUID or timestamp).
    pub id: String,
    /// Human-readable description of this experiment's goal.
    pub description: String,
    /// The tick at which this experiment started (usually 0).
    pub start_tick: Tick,
    /// The RNG seed recorded from the config.
    pub rng_seed: u64,
}

impl ExperimentManifest {
    /// Creates a new manifest.
    pub fn new(id: impl Into<String>, description: impl Into<String>, rng_seed: u64) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            start_tick: Tick::ZERO,
            rng_seed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_starts_at_zero() {
        let m = ExperimentManifest::new("test", "A test experiment", 42);
        assert_eq!(m.start_tick, Tick::ZERO);
        assert_eq!(m.rng_seed, 42);
    }
}
