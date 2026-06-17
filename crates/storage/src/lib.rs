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

/// Placeholder for the storage manager.
///
/// TODO(phase-5): Implement snapshot serialisation, incremental autosave,
/// replay recording, and SQLite experiment database.
pub struct StorageManager;

impl StorageManager {
    /// Creates a new storage manager.
    pub fn new() -> Self {
        Self
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
    fn current_schema_version_is_nonzero() {
        assert!(SchemaVersion::CURRENT.0 > 0);
    }
}
