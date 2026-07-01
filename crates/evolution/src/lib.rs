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

/// # Lineage Trajectory Record
///
/// ## 1. What Happens
/// The `LineageRecord` structurally tracks the demographic lifecycle of a single specific organism,
/// linking it to its ancestral topology (parent), demographic cluster (lineage/species), and temporal bounds (birth/death).
///
/// ## 2. Why It Happens
/// Evolution is emergent, meaning fitness is entirely implicit—organisms survive because they didn't die.
/// To study how genetic configurations correlate with survival, researchers must reconstruct the phylogenetic
/// tree post-simulation. This record is the irreducible quantum of that tree.
///
/// ## 3. How It Happens
/// When an organism is spawned via reproduction, $Entity_{child}$ is linked to $Entity_{parent}$.
/// The fitness metric (Lifespan $L$) can be defined mathematically upon death:
///
/// $$ L = T_{death} - T_{birth} $$
///
/// The collection of all records forms a Directed Acyclic Graph (DAG) representing the evolutionary tree.
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

/// # In-Memory Phylogeny Tracker
///
/// ## 1. What Happens
/// `LineageTracker` is a central ECS resource that acts as an ephemeral holding buffer for the
/// evolutionary Directed Acyclic Graph (DAG) of the current active population.
///
/// ## 2. Why It Happens
/// Logging every birth and death directly to an SQLite disk database causes extreme I/O bottlenecking
/// during periods of high population turnover (e.g., mass extinction events or invasive species blooms).
/// Maintaining an in-memory hash map allows $O(1)$ updates without blocking the simulation thread.
///
/// ## 3. How It Happens
/// The tracker maintains an active set $A$. When an organism is born, it is inserted into $A$.
/// When it dies, its record in $A$ is mutated to include $T_{death}$. The set $A$ is then partitioned
/// during the `extract_completed_records` phase to flush completed lineages to cold storage.
#[derive(bevy_ecs::prelude::Resource)]
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

    /// # Ephemeral DAG Cold-Storage Extraction
    ///
    /// ## 1. What Happens
    /// The `extract_completed_records` method filters the in-memory active set $A$ for all records
    /// where `death_tick` is populated, removes them from the tracker, and returns them as a batch.
    ///
    /// ## 2. Why It Happens
    /// Memory cannot grow infinitely. To prevent Out-Of-Memory (OOM) panics over a multi-day simulation
    /// run with millions of generations, completed dead lineages must be evicted from the active map
    /// and passed to the asynchronous `storage` crate for permanent SQLite persistence.
    ///
    /// ## 3. How It Happens
    /// The filter operation runs over the active set $A$:
    ///
    /// $$ D = \{ r \in A \mid r.death\_tick \ne \emptyset \} $$
    /// $$ A' = A \setminus D $$
    ///
    /// The extracted set $D$ is returned as an owned `Vec` to be handed over to a background rayon
    /// thread, preventing garbage collection stuttering.
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
