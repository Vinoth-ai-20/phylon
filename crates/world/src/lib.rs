//! # Phylon World
//!
//! The central world state, entity registry, graph model, and snapshot
//! coordination layer.
//!
//! The `world` crate owns the canonical simulation state:
//!
//! - **Entity registry**: maps [`EntityId`] to ECS component storage.
//! - **Chunk manager**: tracks which chunks are active and loads/unloads them.
//! - **Interaction graph**: edges between entities (springs, lineage, colony bonds).
//! - **Snapshot coordinator**: orchestrates binary serialisation of world state.
//!
//! ## Dependency rules
//!
//! No rendering, UI, or storage types may appear in this crate. The world is
//! a pure data layer consumed by all simulation subsystems.
//!
//! ## Phase 0 scope
//!
//! Minimal type skeleton. Full entity registry and chunk management: Phase 1.

#![warn(missing_docs)]
#![warn(clippy::all)]

use common::{ChunkId, EntityId};

/// Errors from world-layer operations.
#[derive(Debug, thiserror::Error)]
pub enum WorldError {
    /// An operation targeted an entity that does not exist.
    #[error("entity {0} does not exist in the world")]
    EntityNotFound(EntityId),

    /// An operation targeted a chunk that is not currently active.
    #[error("chunk {0} is not active")]
    ChunkNotActive(ChunkId),
}

impl common::PhylonError for WorldError {}

/// The core ECS-backed world state.
pub struct World {
    /// The underlying bevy_ecs world.
    pub ecs: bevy_ecs::world::World,
    /// The total number of entities ever created (used for ID generation).
    pub entity_counter: u64,
}

impl World {
    /// Creates a new, empty world.
    pub fn new() -> Self {
        Self {
            ecs: bevy_ecs::world::World::new(),
            entity_counter: 0,
        }
    }

    /// Spawns an empty bevy_ecs entity and returns its Bevy Entity ID.
    pub fn spawn_empty(&mut self) -> bevy_ecs::entity::Entity {
        self.entity_counter += 1;
        self.ecs.spawn_empty().id()
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_creates_empty() {
        let world = World::new();
        assert_eq!(world.entity_counter, 0);
        assert_eq!(world.ecs.entities().len(), 0);
    }
}
