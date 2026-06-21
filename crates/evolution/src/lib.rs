//! # Phylon Evolution
//!
//! Selection pressure, speciation, lineage tracking, fitness metrics, and
//! hybridization barriers.
//!
//! Evolution in Phylon is **emergent** — there is no explicit fitness function.
//! Survival and reproduction pressure exerted by the ecology system acts as
//! the selection gradient.
//!
//! ## Phase 0 scope
//!
//! Type declarations only. Implementation: Phase 5.

#![warn(missing_docs)]
#![warn(clippy::all)]

use common::EntityId;
use serde::{Deserialize, Serialize};

/// A lineage identifier linking related organisms across generations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LineageId(pub u64);

/// A species cluster identifier assigned by the speciation algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpeciesId(pub u64);

/// Tracks the lifecycle of a single lineage instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageRecord {
    /// The entity this record belongs to.
    pub entity: EntityId,
    /// The parent entity, if any.
    pub parent_id: Option<EntityId>,
    /// The lineage cluster this organism belongs to.
    pub lineage: LineageId,
    /// The species cluster assigned at last speciation check.
    pub species: SpeciesId,
    /// Generation number (0 for initial population).
    pub generation: u64,
    /// The tick at which this organism was born.
    pub birth_tick: u64,
    /// The tick at which this organism died.
    pub death_tick: Option<u64>,
    /// The cause of death, if applicable.
    pub cause_of_death: Option<String>,
}

/// A centralized resource tracking active lineage histories in-memory.
#[derive(bevy_ecs::system::Resource)]
pub struct LineageTracker {
    next_lineage_id: u64,
    records: std::collections::HashMap<EntityId, LineageRecord>,
}

impl LineageTracker {
    /// Creates a new lineage tracker.
    pub fn new() -> Self {
        Self {
            next_lineage_id: 1,
            records: std::collections::HashMap::new(),
        }
    }

    /// Allocates a new lineage ID for completely new organisms.
    pub fn new_lineage_id(&mut self) -> LineageId {
        let id = LineageId(self.next_lineage_id);
        self.next_lineage_id += 1;
        id
    }

    /// Registers a newly born organism.
    pub fn register_birth(
        &mut self,
        entity: EntityId,
        parent_id: Option<EntityId>,
        lineage: LineageId,
        species: SpeciesId,
        generation: u64,
        birth_tick: u64,
    ) {
        self.records.insert(
            entity,
            LineageRecord {
                entity,
                parent_id,
                lineage,
                species,
                generation,
                birth_tick,
                death_tick: None,
                cause_of_death: None,
            },
        );
    }

    /// Records the death of an organism.
    pub fn register_death(&mut self, entity: EntityId, death_tick: u64, cause: String) {
        if let Some(record) = self.records.get_mut(&entity) {
            record.death_tick = Some(death_tick);
            record.cause_of_death = Some(cause);
        }
    }

    /// Retrieves an active record.
    pub fn get_record(&self, entity: EntityId) -> Option<&LineageRecord> {
        self.records.get(&entity)
    }

    /// Extracts all completed records and removes them from active tracking,
    /// suitable for background flushing to SQLite.
    pub fn extract_completed_records(&mut self) -> Vec<LineageRecord> {
        let completed: Vec<EntityId> = self
            .records
            .iter()
            .filter(|(_, record)| record.death_tick.is_some())
            .map(|(e, _)| *e)
            .collect();

        let mut extracted = Vec::with_capacity(completed.len());
        for e in completed {
            if let Some(record) = self.records.remove(&e) {
                extracted.push(record);
            }
        }
        extracted
    }
}

impl Default for LineageTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lineage_id_equality() {
        assert_eq!(LineageId(1), LineageId(1));
        assert_ne!(LineageId(1), LineageId(2));
    }
}
