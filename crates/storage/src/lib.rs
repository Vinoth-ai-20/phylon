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
//! There is currently no migration path between versions — a bump means
//! files saved under the old version fail to deserialize (see
//! [`SchemaVersion::CURRENT`]'s doc comment for the most recent bump).

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
    /// The current `.phylon` snapshot schema version.
    ///
    /// Because bincode is a non-self-describing, positional binary format,
    /// *any* change to the layout of a type embedded in [`SimulationSnapshot`]
    /// (adding/removing/reordering fields, changing a field's type) makes
    /// existing saved files fail to deserialize against the new layout —
    /// there is no way to "skip" or reinterpret bytes the way a
    /// self-describing format (JSON, RON) could. This project has
    /// consistently chosen not to write migration code for these breaks;
    /// instead every layout change bumps this version, and
    /// [`StorageManager::load_simulation_state`] rejects any file whose
    /// `schema_version` exceeds what this build supports with a clear
    /// [`StorageError::UnsupportedSchema`] rather than corrupting data or
    /// panicking. Version history:
    ///
    /// - **1 -> 2**: `brain::Brain` gained `winner_take_all` and
    ///   `plasticity_enabled` fields. `SnapshotNode.brain` embeds
    ///   `brain::Brain` directly, so any file saved under version 1 breaks.
    /// - **2 -> 3**: `brain::Brain` gained `node_regions: Vec<RegionId>`,
    ///   for the same reason as above.
    /// - **3 -> 4**: `SnapshotNode` gained roughly 15 new optional fields
    ///   covering physiology (`ChemicalEconomy`, `Age`, `Metabolism`,
    ///   `Health`, `Hydration`, `BodyTemperature`, `HormoneLevel`,
    ///   neuromodulator channels, disease state) and the persistent body
    ///   graph, all of which previously vanished silently on save/load.
    /// - **4 -> 5**: every world-space position/velocity field
    ///   (`SnapshotNode.position`/`velocity`,
    ///   `SnapshotFood`/`SnapshotMineral`/`SnapshotCorpse.position`) changed
    ///   from a 2-field `SerializedVec2` (which silently dropped `z` on save
    ///   and re-extended it with `0.0` on restore) to a 3-field
    ///   `SerializedVec3` with full `Vec3` fidelity and no truncation.
    ///
    /// As with every prior bump, **no migration path is provided** for this
    /// change: any `.phylon`/`.phylon-replay` file saved under version ≤ 4
    /// will fail to load, and this must be communicated to
    /// users/researchers explicitly rather than discovered as a silent data
    /// loss.
    pub const CURRENT: Self = Self(5);
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

/// Deterministic replay: initial snapshot + recorded intervention log.
pub mod replay;

use snapshot::SimulationSnapshot;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

/// # Simulation Storage Manager
///
/// ## 1. What Happens
/// The `StorageManager` handles disk I/O, writing binary state snapshots, and logging
/// genealogical and event data to a persistent SQLite database.
///
/// ## 2. Why It Happens
/// A simulation with 10,000 organisms running for days will generate enormous amounts of
/// data. We split this into two formats: fast binary snapshots (for pausing/resuming the
/// app) and relational SQL data (for offline analytics, lineage tracking, and graphing).
///
/// ## 3. How It Happens
/// It maintains a long-lived connection to `data/runs.db`. When `flush_lineages` is called,
/// it batches `LineageRecord` inserts. When `save_simulation_state` is called, it serializes
/// the ECS world using `bincode` into a `.phylon` snapshot file.
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

    /// Exports every row of the `lineages` table to a CSV file.
    pub fn export_lineages_csv(&self, path: &Path) -> Result<(), StorageError> {
        let Some(db) = &self.run_db else {
            return Ok(());
        };
        let csv = lineages_csv_from_db(db)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, csv)?;
        Ok(())
    }

    /// Exports every row of the `events` table to a CSV file.
    pub fn export_events_csv(&self, path: &Path) -> Result<(), StorageError> {
        let Some(db) = &self.run_db else {
            return Ok(());
        };
        let csv = events_csv_from_db(db)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, csv)?;
        Ok(())
    }
}

/// Renders every row of the `lineages` table as a CSV string. Split out
/// from [`StorageManager::export_lineages_csv`] so it's testable against an
/// in-memory `rusqlite::Connection`, without needing a real `runs.db` file.
fn lineages_csv_from_db(db: &rusqlite::Connection) -> Result<String, StorageError> {
    let mut stmt = db
        .prepare(
            "SELECT lineage_id, species_id, entity_id, parent_id, generation, \
             birth_tick, death_tick, cause_of_death FROM lineages",
        )
        .map_err(std::io::Error::other)?;

    let mut out = String::from(
        "lineage_id,species_id,entity_id,parent_id,generation,birth_tick,death_tick,cause_of_death\n",
    );
    let mut rows = stmt.query([]).map_err(std::io::Error::other)?;
    while let Some(row) = rows.next().map_err(std::io::Error::other)? {
        let lineage_id: i64 = row.get(0).map_err(std::io::Error::other)?;
        let species_id: i64 = row.get(1).map_err(std::io::Error::other)?;
        let entity_id: i64 = row.get(2).map_err(std::io::Error::other)?;
        let parent_id: Option<i64> = row.get(3).map_err(std::io::Error::other)?;
        let generation: i64 = row.get(4).map_err(std::io::Error::other)?;
        let birth_tick: i64 = row.get(5).map_err(std::io::Error::other)?;
        let death_tick: Option<i64> = row.get(6).map_err(std::io::Error::other)?;
        let cause_of_death: Option<String> = row.get(7).map_err(std::io::Error::other)?;

        out.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            lineage_id,
            species_id,
            entity_id,
            parent_id.map(|v| v.to_string()).unwrap_or_default(),
            generation,
            birth_tick,
            death_tick.map(|v| v.to_string()).unwrap_or_default(),
            csv_escape(&cause_of_death.unwrap_or_default()),
        ));
    }
    Ok(out)
}

/// Renders every row of the `events` table as a CSV string — see
/// [`lineages_csv_from_db`]'s doc comment for why this is a free function.
fn events_csv_from_db(db: &rusqlite::Connection) -> Result<String, StorageError> {
    let mut stmt = db
        .prepare("SELECT id, tick, event_type, description FROM events")
        .map_err(std::io::Error::other)?;

    let mut out = String::from("id,tick,event_type,description\n");
    let mut rows = stmt.query([]).map_err(std::io::Error::other)?;
    while let Some(row) = rows.next().map_err(std::io::Error::other)? {
        let id: i64 = row.get(0).map_err(std::io::Error::other)?;
        let tick: i64 = row.get(1).map_err(std::io::Error::other)?;
        let event_type: String = row.get(2).map_err(std::io::Error::other)?;
        let description: String = row.get(3).map_err(std::io::Error::other)?;

        out.push_str(&format!(
            "{},{},{},{}\n",
            id,
            tick,
            csv_escape(&event_type),
            csv_escape(&description),
        ));
    }
    Ok(out)
}

/// Exports a snapshot's organism nodes (position, velocity, mass, diet,
/// category — not the nested genome/brain, which don't fit a flat CSV row)
/// to a CSV file. Takes an already-built [`SimulationSnapshot`] rather than
/// querying the ECS itself, reusing the exact same data `.phylon` saves
/// already collect instead of a second, parallel query path.
pub fn export_organisms_csv(
    snapshot: &SimulationSnapshot,
    path: &Path,
) -> Result<(), StorageError> {
    // `z`/`vz` columns exist alongside `SnapshotNode`'s `SerializedVec3`
    // fields (full 3D fidelity — see `SchemaVersion::CURRENT`'s doc comment).
    let mut out =
        String::from("id,x,y,z,vx,vy,vz,mass,segment_type,organism_id,is_fixed,diet,category\n");
    for node in &snapshot.nodes {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            node.id,
            node.position.x,
            node.position.y,
            node.position.z,
            node.velocity.x,
            node.velocity.y,
            node.velocity.z,
            node.mass,
            node.segment_type,
            node.organism_id,
            node.is_fixed,
            node.diet
                .as_ref()
                .map(|d| csv_escape(&format!("{d:?}")))
                .unwrap_or_default(),
            node.category
                .as_ref()
                .map(|c| csv_escape(&format!("{c:?}")))
                .unwrap_or_default(),
        ));
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, out)?;
    Ok(())
}

/// Minimal RFC 4180 quoting: wraps a field in double quotes (doubling any
/// embedded quotes) if it contains a comma, quote, or newline.
fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
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

    /// An incompatible/old-schema file must be rejected cleanly (a returned
    /// `Err`, never a panic or silently-corrupted data) — per the "no
    /// migration path" policy described on `SchemaVersion::CURRENT`. Rather
    /// than hand-crafting a byte-perfect old-schema-shaped artifact, this
    /// feeds `load_simulation_state` a byte sequence bincode cannot possibly
    /// parse as the current `SimulationSnapshot` shape — the same failure
    /// mode a real mismatched-schema file produces, since bincode is a
    /// non-self-describing positional format that fails to deserialize
    /// against a struct with a different layout before `schema_version` is
    /// ever inspected.
    #[test]
    fn load_simulation_state_rejects_incompatible_data_cleanly() {
        let dir = std::env::temp_dir().join(format!("phylon-schema-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.phylon");
        std::fs::write(&path, b"not a valid bincode-encoded snapshot at all").unwrap();

        let result = StorageManager::load_simulation_state(&path);
        assert!(
            matches!(result, Err(StorageError::Io { .. })),
            "expected a clean Err, got {result:?}"
        );

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn csv_escape_passes_through_plain_fields() {
        assert_eq!(csv_escape("plain"), "plain");
    }

    #[test]
    fn csv_escape_quotes_fields_with_commas_and_doubles_quotes() {
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    fn in_memory_db_with_lineages() -> rusqlite::Connection {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute(
            "CREATE TABLE lineages (
                lineage_id INTEGER, species_id INTEGER, entity_id INTEGER PRIMARY KEY,
                parent_id INTEGER, generation INTEGER, birth_tick INTEGER,
                death_tick INTEGER, cause_of_death TEXT
            )",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO lineages VALUES (1, 2, 100, NULL, 0, 5, 50, 'starved')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO lineages VALUES (1, 2, 101, 100, 1, 10, NULL, NULL)",
            [],
        )
        .unwrap();
        db
    }

    #[test]
    fn lineages_csv_includes_header_and_all_rows() {
        let db = in_memory_db_with_lineages();
        let csv = lineages_csv_from_db(&db).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "lineage_id,species_id,entity_id,parent_id,generation,birth_tick,death_tick,cause_of_death");
        assert_eq!(lines.len(), 3); // header + 2 rows
        assert!(lines[1].contains("100") && lines[1].contains("starved"));
        // NULL parent_id/death_tick/cause_of_death render as empty fields.
        assert!(lines[2].contains("101,100,1,10,,"));
    }

    fn in_memory_db_with_events() -> rusqlite::Connection {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute(
            "CREATE TABLE events (
                id INTEGER PRIMARY KEY, tick INTEGER, event_type TEXT, description TEXT
            )",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO events VALUES (1, 10, 'Lineage', 'reached, generation 5')",
            [],
        )
        .unwrap();
        db
    }

    #[test]
    fn events_csv_escapes_embedded_commas() {
        let db = in_memory_db_with_events();
        let csv = events_csv_from_db(&db).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "id,tick,event_type,description");
        assert_eq!(lines[1], "1,10,Lineage,\"reached, generation 5\"");
    }

    #[test]
    fn export_organisms_csv_writes_header_and_rows() {
        use snapshot::{SerializedVec3, SnapshotNode};

        let snapshot = SimulationSnapshot {
            schema_version: SchemaVersion::CURRENT.0,
            seed: 1,
            total_sim_time: 0.0,
            nodes: vec![SnapshotNode {
                id: 1,
                position: SerializedVec3 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                velocity: SerializedVec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                mass: 1.0,
                segment_type: 0,
                is_fixed: false,
                organism_id: 1,
                color: None,
                diet: Some(ecology::Diet::Herbivore),
                category: None,
                genome: None,
                brain: None,
                chemical_economy: None,
                age: None,
                metabolism: None,
                health: None,
                hydration: None,
                body_temperature: None,
                generation: None,
                spawn_tick: None,
                life_stage: None,
                morphogen_level: None,
                hormone_level: None,
                neuromodulator_channels: None,
                infection: None,
                segment_infection: None,
                segment_immunity: None,
                developmental_graph: None,
            }],
            springs: vec![],
            food_pellets: vec![],
            mineral_pellets: vec![],
            corpses: vec![],
            diffusion_data: vec![],
        };

        let dir = std::env::temp_dir().join(format!("phylon-csv-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("organisms.csv");

        export_organisms_csv(&snapshot, &path).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(
            lines[0],
            "id,x,y,z,vx,vy,vz,mass,segment_type,organism_id,is_fixed,diet,category"
        );
        assert!(lines[1].starts_with("1,1,2,3,0,0,0,1,0,1,false,Herbivore"));

        std::fs::remove_file(&path).unwrap();
    }
}
