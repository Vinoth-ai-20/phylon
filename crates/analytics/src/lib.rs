//! # Phylon Analytics
//!
//! Metrics collection, population history, diversity indices, spatial
//! heatmaps, lineage tracking, and research report generation.
//!
//! The analytics crate is a pure consumer of the event bus — it never
//! mutates simulation state. It accumulates time-series data and exposes
//! query APIs for the UI and research crates.
//!
//! ## Phase 0 scope
//!
//! Metric type declarations. Implementation: Phase 9.

#![warn(missing_docs)]
#![warn(clippy::all)]

use common::Tick;
use serde::{Deserialize, Serialize};

/// A single population count sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopulationSample {
    /// The tick at which this sample was recorded.
    pub tick: Tick,
    /// Total organism count across all species.
    pub total: u64,
}

/// Placeholder for the analytics accumulator.
///
/// TODO(phase-9): Implement full metrics collection, diversity indices,
/// SQLite persistence, and export APIs.
pub struct AnalyticsAccumulator {
    samples: Vec<PopulationSample>,
}

impl AnalyticsAccumulator {
    /// Creates a new empty accumulator.
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
        }
    }

    /// Records a population sample.
    pub fn record_population(&mut self, tick: Tick, total: u64) {
        self.samples.push(PopulationSample { tick, total });
    }

    /// Returns the number of recorded samples.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }
}

impl Default for AnalyticsAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulator_records_samples() {
        let mut acc = AnalyticsAccumulator::new();
        acc.record_population(Tick(0), 100);
        acc.record_population(Tick(60), 105);
        assert_eq!(acc.sample_count(), 2);
    }
}
