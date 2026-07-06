//! Deterministic replay: an initial [`SimulationSnapshot`] plus a
//! chronological log of external interventions, together sufficient to
//! reproduce a run bit-for-bit (per the spec's determinism guarantee:
//! "replay is guaranteed by recording the RNG seed + all external
//! interventions").

use crate::snapshot::{SerializedVec2, SimulationSnapshot};
use crate::StorageError;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The subset of simulation-mutating `ui::MenuAction` interventions that
/// are safe to record and replay literally.
///
/// Deliberately excludes anything referencing a live `bevy_ecs::Entity`
/// (`KillEntity`, `TrackEntity`, `SelectEntity`, ...) — entity IDs are an
/// implementation detail of one specific run's `bevy_ecs::World` allocation
/// order, not a stable identity a *fresh* replay run is guaranteed to
/// reproduce. Recording "kill entity #47" and replaying it against a
/// differently-allocated entity #47 would silently corrupt the replay
/// rather than fail loudly, which is worse than not supporting it. Camera,
/// panel-visibility, and other pure-UI actions are excluded because they
/// never touch simulation state — replay only needs actions that do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ReplayAction {
    /// `ui::MenuAction::ReseedEcosystem` — no parameters; the reseed itself
    /// draws from the already-deterministic `SimRng`.
    ReseedEcosystem,
    /// `ui::MenuAction::SpawnPreset` — `position` is the resolved spawn
    /// point at record time (originally `self.ui.camera_pos`, which isn't
    /// itself deterministic/replayable), not re-derived during playback.
    SpawnPreset {
        /// The preset's name, matched against `organisms::sandbox::PresetDefinition`.
        name: String,
        /// The resolved world-space spawn position.
        position: SerializedVec2,
    },
    /// `ui::MenuAction::SpawnProtoFish` — same resolved-position rationale
    /// as `SpawnPreset`.
    SpawnProtoFish {
        /// The resolved world-space spawn position.
        position: SerializedVec2,
    },
    /// `ui::MenuAction::SpawnManualHazard` — same resolved-position
    /// rationale as `SpawnPreset`.
    SpawnManualHazard {
        /// The resolved world-space hazard position.
        position: SerializedVec2,
    },
}

/// One recorded intervention, tagged with the simulation tick it occurred
/// at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayEvent {
    /// The tick at which `action` was originally applied.
    pub tick: u64,
    /// The recorded action.
    pub action: ReplayAction,
}

/// # Deterministic Replay Log
///
/// ## 1. What Happens
/// Records every safe external intervention (see [`ReplayAction`]) in
/// chronological tick order, alongside the RNG seed the run started from.
///
/// ## 2. Why It Happens
/// A purely emergent run (no manual interventions) is already
/// bit-reproducible from its initial snapshot + seed, since every
/// stochastic decision draws from the same seeded `common::SimRng`. Manual
/// god-mode interventions are the only *external* source of divergence —
/// recording them (and only them, not full per-tick state) is what the
/// spec means by "replayable experiments" without the cost of a full
/// per-tick recording.
///
/// ## 3. How It Happens
/// `record` appends events in call order (already tick-ascending in
/// practice, since interventions are recorded live during a forward-running
/// simulation); [`ReplayLog::events_at`] returns everything recorded at a
/// given tick, in original order, for the replay driver
/// (`app::replay::run_replay`) to re-apply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayLog {
    /// The RNG seed this run started from.
    pub seed: u64,
    /// Every recorded intervention, in chronological order.
    pub events: Vec<ReplayEvent>,
}

impl ReplayLog {
    /// Creates a new, empty replay log for a run started with `seed`.
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            events: Vec::new(),
        }
    }

    /// Records one intervention at `tick`.
    pub fn record(&mut self, tick: u64, action: ReplayAction) {
        self.events.push(ReplayEvent { tick, action });
    }

    /// Iterates every recorded action at exactly `tick`, in the order they
    /// were originally recorded.
    pub fn events_at(&self, tick: u64) -> impl Iterator<Item = &ReplayAction> {
        self.events
            .iter()
            .filter(move |e| e.tick == tick)
            .map(|e| &e.action)
    }

    /// The tick of the last recorded event, or `0` if none were recorded.
    pub fn last_event_tick(&self) -> u64 {
        self.events.iter().map(|e| e.tick).max().unwrap_or(0)
    }
}

/// A full replay bundle: the initial state a run started from, plus the log
/// of interventions applied while it ran. Saved/loaded together as one
/// `.phylon-replay` file so the two can never accidentally become
/// mismatched (an initial snapshot from one run paired with another run's
/// intervention log would replay nonsense).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayBundle {
    /// The simulation state at tick 0 (or whenever recording started).
    pub initial_snapshot: SimulationSnapshot,
    /// The recorded interventions.
    pub log: ReplayLog,
}

impl ReplayBundle {
    /// Serializes this bundle to a binary file using bincode, mirroring
    /// [`crate::StorageManager::save_simulation_state`]'s format choice.
    pub fn save_to_file(&self, path: &Path) -> Result<(), StorageError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let encoded = bincode::serialize(self).map_err(std::io::Error::other)?;
        std::fs::write(path, encoded)?;
        Ok(())
    }

    /// Deserializes a bundle previously written by
    /// [`ReplayBundle::save_to_file`].
    pub fn load_from_file(path: &Path) -> Result<Self, StorageError> {
        let bytes = std::fs::read(path)?;
        let bundle: Self = bincode::deserialize(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(bundle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_at_returns_only_matching_tick_in_order() {
        let mut log = ReplayLog::new(42);
        log.record(10, ReplayAction::ReseedEcosystem);
        log.record(
            20,
            ReplayAction::SpawnProtoFish {
                position: SerializedVec2 { x: 1.0, y: 2.0 },
            },
        );
        log.record(
            20,
            ReplayAction::SpawnManualHazard {
                position: SerializedVec2 { x: 3.0, y: 4.0 },
            },
        );

        let at_20: Vec<&ReplayAction> = log.events_at(20).collect();
        assert_eq!(at_20.len(), 2);
        assert_eq!(
            at_20[0],
            &ReplayAction::SpawnProtoFish {
                position: SerializedVec2 { x: 1.0, y: 2.0 }
            }
        );

        let at_10: Vec<&ReplayAction> = log.events_at(10).collect();
        assert_eq!(at_10, vec![&ReplayAction::ReseedEcosystem]);

        let at_99: Vec<&ReplayAction> = log.events_at(99).collect();
        assert!(at_99.is_empty());
    }

    #[test]
    fn last_event_tick_is_zero_when_empty() {
        let log = ReplayLog::new(1);
        assert_eq!(log.last_event_tick(), 0);
    }

    #[test]
    fn last_event_tick_tracks_the_maximum() {
        let mut log = ReplayLog::new(1);
        log.record(5, ReplayAction::ReseedEcosystem);
        log.record(50, ReplayAction::ReseedEcosystem);
        log.record(20, ReplayAction::ReseedEcosystem);
        assert_eq!(log.last_event_tick(), 50);
    }

    #[test]
    fn replay_bundle_round_trips_through_bincode() {
        let mut log = ReplayLog::new(7);
        log.record(
            3,
            ReplayAction::SpawnPreset {
                name: "Herbivore".to_string(),
                position: SerializedVec2 { x: 5.0, y: -5.0 },
            },
        );
        let bundle = ReplayBundle {
            initial_snapshot: SimulationSnapshot {
                schema_version: crate::SchemaVersion::CURRENT.0,
                seed: 7,
                total_sim_time: 0.0,
                nodes: vec![],
                springs: vec![],
                food_pellets: vec![],
                mineral_pellets: vec![],
                corpses: vec![],
                diffusion_data: vec![],
            },
            log,
        };

        let dir = std::env::temp_dir().join(format!("phylon-replay-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.phylon-replay");

        bundle.save_to_file(&path).unwrap();
        let loaded = ReplayBundle::load_from_file(&path).unwrap();

        assert_eq!(loaded.log.seed, 7);
        assert_eq!(loaded.log.events.len(), 1);
        assert_eq!(loaded.initial_snapshot.seed, 7);

        std::fs::remove_file(&path).unwrap();
    }
}
