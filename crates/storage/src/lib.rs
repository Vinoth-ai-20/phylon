//! # Phylon Storage
//!
//! Save/load, binary snapshots, replay capture and playback, dataset export,
//! and SQLite persistence for long-running research experiments.
//!
//! ## Save formats
//!
//! - `.phylon` — fast binary snapshot via `bincode` (complete simulation state)
//! - `.phylon-research` — SQLite + exported CSVs (research archive)
//! - `.ron` — human-readable scenario file (initial conditions)
//!
//! All formats include a `schema_version` field for migration compatibility.
//!
//! ## Phase 0 scope
//!
//! SchemaVersion type and placeholder storage manager. Implementation: Phase 5.

#![warn(missing_docs)]
#![warn(clippy::all)]

use serde::{Deserialize, Serialize};

/// Identifies the serialisation schema version of a saved file.
///
/// Every saved format must include this field so the loader can apply
/// the correct migration path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SchemaVersion(pub u32);

impl SchemaVersion {
    /// The current schema version for Phase 0 snapshots.
    pub const CURRENT: Self = Self(1);
}

impl std::fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "v{}", self.0)
    }
}

/// Errors from storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// The file could not be read or written.
    #[error("I/O error: {source}")]
    Io {
        /// Underlying I/O error.
        #[from]
        source: std::io::Error,
    },

    /// The file's schema version is not supported by this build.
    #[error("unsupported schema version {found}; expected ≤ {max_supported}")]
    UnsupportedSchema {
        /// The version found in the file.
        found: SchemaVersion,
        /// The maximum version this build can read.
        max_supported: SchemaVersion,
    },
}

impl common::PhylonError for StorageError {}

pub mod snapshot;

use snapshot::SimulationSnapshot;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

/// Manages serialization of simulation state and run tracking.
pub struct StorageManager {
    run_db: Option<rusqlite::Connection>,
}

impl StorageManager {
    /// Creates a new storage manager, optionally opening the SQLite run database.
    pub fn new() -> Self {
        let run_db = rusqlite::Connection::open("data/runs.db").ok();
        if let Some(db) = &run_db {
            let _ = db.execute(
                "CREATE TABLE IF NOT EXISTS runs (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    start_time TEXT NOT NULL,
                    seed INTEGER NOT NULL,
                    end_time TEXT,
                    ticks INTEGER,
                    final_population INTEGER
                )",
                [],
            );

            let _ = db.execute(
                "CREATE TABLE IF NOT EXISTS lineages (
                    lineage_id INTEGER,
                    species_id INTEGER,
                    entity_id INTEGER PRIMARY KEY,
                    parent_id INTEGER,
                    generation INTEGER,
                    birth_tick INTEGER,
                    death_tick INTEGER,
                    cause_of_death TEXT
                )",
                [],
            );

            let _ = db.execute(
                "CREATE TABLE IF NOT EXISTS events (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    tick INTEGER NOT NULL,
                    event_type TEXT NOT NULL,
                    description TEXT NOT NULL
                )",
                [],
            );
        }
        Self { run_db }
    }

    /// Serializes the given snapshot to a binary file using bincode.
    pub fn save_simulation_state(
        snapshot: &SimulationSnapshot,
        path: &Path,
    ) -> Result<(), StorageError> {
        let mut file = File::create(path)?;
        let encoded = bincode::serialize(snapshot).map_err(std::io::Error::other)?;
        file.write_all(&encoded)?;
        Ok(())
    }

    /// Deserializes a binary snapshot from a file.
    pub fn load_simulation_state(path: &Path) -> Result<SimulationSnapshot, StorageError> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        let snapshot: SimulationSnapshot = bincode::deserialize(&buffer)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        if snapshot.schema_version > SchemaVersion::CURRENT.0 {
            return Err(StorageError::UnsupportedSchema {
                found: SchemaVersion(snapshot.schema_version),
                max_supported: SchemaVersion::CURRENT,
            });
        }

        Ok(snapshot)
    }

    /// Logs the start of a simulation run to the database and returns the row ID.
    pub fn log_run_start(&self, seed: u64) -> Option<i64> {
        if let Some(db) = &self.run_db {
            let start_time = chrono::Utc::now().to_rfc3339();
            db.execute(
                "INSERT INTO runs (start_time, seed) VALUES (?1, ?2)",
                rusqlite::params![start_time, seed as i64],
            )
            .ok()?;
            return Some(db.last_insert_rowid());
        }
        None
    }

    /// Logs the end of a simulation run to the database, updating ticks and final population.
    pub fn log_run_end(&self, run_id: i64, ticks: u64, final_population: u32) {
        if let Some(db) = &self.run_db {
            let end_time = chrono::Utc::now().to_rfc3339();
            let _ = db.execute(
                "UPDATE runs SET end_time = ?1, ticks = ?2, final_population = ?3 WHERE id = ?4",
                rusqlite::params![end_time, ticks as i64, final_population, run_id],
            );
        }
    }

    /// Flushes an extracted batch of completed lineage records to SQLite.
    pub fn flush_lineages(&self, records: &[evolution::LineageRecord]) {
        if let Some(db) = &self.run_db {
            let mut stmt = db.prepare_cached(
                "INSERT OR REPLACE INTO lineages (lineage_id, species_id, entity_id, parent_id, generation, birth_tick, death_tick, cause_of_death)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
            ).unwrap();

            for r in records {
                let _ = stmt.execute(rusqlite::params![
                    r.lineage.0 as i64,
                    r.species.0 as i64,
                    r.entity.0 as i64,
                    r.parent_id.map(|p| p.0 as i64),
                    r.generation as i64,
                    r.birth_tick as i64,
                    r.death_tick.map(|t| t as i64),
                    r.cause_of_death.clone(),
                ]);
            }
        }
    }

    /// Logs a narrative event to SQLite.
    pub fn log_event(&self, tick: u64, event_type: &str, description: &str) {
        if let Some(db) = &self.run_db {
            let _ = db.execute(
                "INSERT INTO events (tick, event_type, description) VALUES (?1, ?2, ?3)",
                rusqlite::params![tick as i64, event_type, description],
            );
        }
    }
}

impl Default for StorageManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_ordering() {
        assert!(SchemaVersion(1) < SchemaVersion(2));
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn current_schema_version_is_nonzero() {
        assert!(SchemaVersion::CURRENT.0 > 0);
    }
}
