//! # Phylon Research
//!
//! Experiment manager, scenario authoring, batch run orchestration,
//! and research report generation.
//!
//! The research crate provides the high-level API for defining, running,
//! and comparing experiments. It coordinates the scheduler (for headless
//! execution), the analytics accumulator, and the storage manager.
//!
//! ## Current scope
//!
//! [`ExperimentManifest`] only — a plain data record with no batch-run
//! orchestration, scenario editor, or report generation implemented yet,
//! and (notably) not yet constructed anywhere outside this crate's own
//! tests: `app` does not currently depend on `research` at all, so the
//! seed/manifest recording described in the type's own doc comment below
//! doesn't happen in practice today. See the implementation roadmap's
//! "Research Infrastructure" epic.

#![warn(missing_docs)]
#![warn(clippy::all)]

use common::Tick;

/// # Scientific Experiment Manifest
///
/// ## 1. What Happens
/// The `ExperimentManifest` is a data record defining the parameters, deterministic seeds,
/// and metadata for a specific headless simulation run.
///
/// ## 2. Why It Happens
/// Academic ALife research requires reproducibility. If a user observes a fascinating
/// speciation event at tick $1,000,000$, they need to be able to re-run the exact simulation
/// with the exact same RNG seed to study it. The manifest ensures all exported SQLite databases
/// are strictly tied to their initial conditions.
///
/// ## 3. How It Happens
/// In Phase 9, this struct is instantiated at the start of a headless batch run, serialized
/// into the `analytics` output folder, and embedded into the `storage` SQLite headers.
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
