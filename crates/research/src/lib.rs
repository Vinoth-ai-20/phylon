//! # Phylon Research
//!
//! Experiment manager, scenario authoring, batch run orchestration,
//! and research report generation.
//!
//! The research crate provides the data types for defining, running, and
//! comparing experiments: [`ExperimentManifest`] (what a single run's
//! identity/seed/config is), [`BatchRunConfig`] (a set of seeds to run), and
//! [`ExperimentReport`] (a completed run's summary, renderable as
//! Markdown). This crate is deliberately independent of `app`/`bevy_ecs` —
//! it can't drive a simulation itself, since `app` is the composition root
//! and nothing may be depended on by it in reverse. The actual
//! orchestration loop (constructing a `PhylonApp` per seed, stepping it,
//! collecting the final state into an `ExperimentReport`) lives in `app`
//! (see `app::batch::run_batch`), which now depends on this crate — closing
//! the gap where `ExperimentManifest` was declared but never constructed
//! outside this crate's own tests.

#![warn(missing_docs)]
#![warn(clippy::all)]

use common::Tick;
use serde::{Deserialize, Serialize};

/// Errors from experiment manifest/report serialization.
#[derive(Debug, thiserror::Error)]
pub enum ResearchError {
    /// The manifest/report file could not be read or written.
    #[error("I/O error: {source}")]
    Io {
        /// Underlying I/O error.
        #[from]
        source: std::io::Error,
    },
    /// The manifest/report could not be serialized to RON.
    #[error("RON serialization error: {source}")]
    Serialize {
        /// Underlying RON error.
        #[from]
        source: ron::Error,
    },
    /// The manifest/report could not be deserialized from RON.
    #[error("RON deserialization error: {source}")]
    Deserialize {
        /// Underlying RON spanned error.
        #[from]
        source: ron::error::SpannedError,
    },
}

impl common::PhylonError for ResearchError {}

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
/// Constructed once per run in `app::PhylonApp::new` from `config::ResearchConfig::experiment_id`
/// and `config::SimulationConfig::rng_seed`, then persisted via [`ExperimentManifest::save_to_ron`]
/// so a saved run's exact seed/description can be recovered later even without the original
/// config file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    /// Serializes this manifest to a human-readable RON file, creating parent
    /// directories as needed.
    pub fn save_to_ron(&self, path: &std::path::Path) -> Result<(), ResearchError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())?;
        std::fs::write(path, text)?;
        Ok(())
    }

    /// Deserializes a manifest previously written by [`ExperimentManifest::save_to_ron`].
    pub fn load_from_ron(path: &std::path::Path) -> Result<Self, ResearchError> {
        let text = std::fs::read_to_string(path)?;
        Ok(ron::de::from_str(&text)?)
    }
}

/// # Batch Run Configuration
///
/// ## 1. What Happens
/// Describes a set of headless experiment runs to execute in sequence, one
/// per seed in `seeds`, sharing everything else (base description, tick
/// budget).
///
/// ## 2. Why It Happens
/// A single headless run answers "what happens with this exact seed?" —
/// research usually needs "what happens *across* seeds?" (does a
/// speciation pattern reproduce, or was it a one-off). Batch running is the
/// minimal structure that answers that without hand-rolling a shell loop
/// around the binary.
///
/// ## 3. How It Happens
/// `app::batch::run_batch` consumes this: for each seed, it clones the base
/// `PhylonConfig`, overrides `simulation.rng_seed`, builds a fresh
/// `ExperimentManifest`, runs a `PhylonApp` headlessly for `max_ticks`, and
/// collects the result into an [`ExperimentReport`].
#[derive(Debug, Clone)]
pub struct BatchRunConfig {
    /// Shared human-readable description for every run in this batch.
    pub base_description: String,
    /// One experiment is run per seed, in order.
    pub seeds: Vec<u64>,
    /// Ticks to run each experiment for.
    pub max_ticks: u64,
}

/// # Completed Experiment Report
///
/// ## 1. What Happens
/// Summarizes one finished experiment run: its manifest (identity/seed),
/// how many ticks it actually ran, and its final population/species counts.
///
/// ## 2. Why It Happens
/// Per-run Markdown reports are the spec's "research reports (auto-generated
/// Markdown summary per experiment)" — a human-readable artifact a
/// researcher can skim without opening SQLite or a `.phylon` binary file.
///
/// ## 3. How It Happens
/// [`ExperimentReport::to_markdown`] renders a single run; a batch's reports
/// are further aggregated by [`render_batch_summary_markdown`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperimentReport {
    /// The manifest this report belongs to.
    pub manifest: ExperimentManifest,
    /// How many ticks were actually simulated.
    pub ticks_run: u64,
    /// Population size at the end of the run.
    pub final_population: u32,
    /// Number of distinct species at the end of the run.
    pub final_species_count: usize,
}

impl ExperimentReport {
    /// Renders this report as a Markdown summary.
    pub fn to_markdown(&self) -> String {
        format!(
            "# Experiment Report: {}\n\n\
             - **ID**: {}\n\
             - **Seed**: {}\n\
             - **Ticks run**: {}\n\
             - **Final population**: {}\n\
             - **Final species count**: {}\n",
            self.manifest.description,
            self.manifest.id,
            self.manifest.rng_seed,
            self.ticks_run,
            self.final_population,
            self.final_species_count,
        )
    }
}

/// Renders a batch of [`ExperimentReport`]s as one aggregate Markdown
/// summary — a table of every run plus the population mean/min/max across
/// the batch, so a researcher can see at a glance whether an outcome is
/// typical or an outlier.
pub fn render_batch_summary_markdown(reports: &[ExperimentReport]) -> String {
    let mut out = String::from("# Batch Run Summary\n\n");
    if reports.is_empty() {
        out.push_str("_No runs in this batch._\n");
        return out;
    }

    out.push_str("| Seed | Ticks Run | Final Population | Final Species Count |\n");
    out.push_str("|------|-----------|-------------------|----------------------|\n");
    for r in reports {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            r.manifest.rng_seed, r.ticks_run, r.final_population, r.final_species_count
        ));
    }

    let populations: Vec<u32> = reports.iter().map(|r| r.final_population).collect();
    let sum: u64 = populations.iter().map(|&p| p as u64).sum();
    let mean = sum as f64 / populations.len() as f64;
    let min = populations.iter().min().copied().unwrap_or(0);
    let max = populations.iter().max().copied().unwrap_or(0);

    out.push_str(&format!(
        "\n**Final population across {} runs**: mean {:.1}, min {}, max {}\n",
        reports.len(),
        mean,
        min,
        max
    ));

    out
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

    #[test]
    fn manifest_round_trips_through_ron() {
        let dir = std::env::temp_dir().join(format!("phylon-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("manifest.ron");

        let original = ExperimentManifest::new("exp-1", "A round-trip test", 12345);
        original.save_to_ron(&path).unwrap();
        let loaded = ExperimentManifest::load_from_ron(&path).unwrap();

        assert_eq!(original, loaded);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn batch_summary_reports_mean_min_max() {
        let reports = vec![
            ExperimentReport {
                manifest: ExperimentManifest::new("a", "d", 1),
                ticks_run: 100,
                final_population: 10,
                final_species_count: 2,
            },
            ExperimentReport {
                manifest: ExperimentManifest::new("b", "d", 2),
                ticks_run: 100,
                final_population: 20,
                final_species_count: 3,
            },
        ];
        let markdown = render_batch_summary_markdown(&reports);
        assert!(markdown.contains("mean 15.0, min 10, max 20"));
    }

    #[test]
    fn batch_summary_handles_empty_batch() {
        let markdown = render_batch_summary_markdown(&[]);
        assert!(markdown.contains("No runs in this batch"));
    }
}
