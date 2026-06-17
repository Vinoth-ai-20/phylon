//! # Phylon Spatial
//!
//! Spatial indexing structures for efficient entity neighbourhood queries.
//!
//! Three complementary indexing strategies are provided, all sharing a common
//! [`SpatialQuery`] interface:
//!
//! - **[`UniformGrid`]** — O(1) insert and radius query for dense, uniformly
//!   distributed entities. The primary structure for active chunks.
//! - **[`SpatialHash`]** — same asymptotic complexity as the uniform grid but
//!   with dynamic bucketing; preferred when entity density is uneven.
//! - **Quadtree** — sparse, logarithmic-depth structure for long-range queries
//!   on static or slow-moving objects. Implemented in Phase 2.
//!
//! All structures synchronise with [`common::EntityId`] via position updates
//! and support batch queries for rayon-parallel sensing workloads.
//!
//! ## Phase 0 scope
//!
//! Type signatures and public API surface are declared. Implementations are
//! `// TODO(phase-1)` stubs — filled in when `world` integration is ready.

#![warn(missing_docs)]
#![warn(clippy::all)]

use common::{EntityId, Vec2};

/// Result type for spatial operations.
pub type SpatialResult<T> = Result<T, SpatialError>;

/// Errors produced by spatial indexing operations.
#[derive(Debug, thiserror::Error)]
pub enum SpatialError {
    /// An entity was inserted with an ID that already exists in the index.
    #[error("entity {0} is already registered in the spatial index")]
    DuplicateEntity(EntityId),

    /// An operation was attempted on an entity that is not in the index.
    #[error("entity {0} is not registered in the spatial index")]
    UnknownEntity(EntityId),

    /// A configuration parameter is invalid (e.g., cell size ≤ 0).
    #[error("invalid spatial index configuration: {message}")]
    InvalidConfig {
        /// Description of the invalid parameter.
        message: String,
    },
}

impl common::PhylonError for SpatialError {}

/// A uniform-grid spatial index.
///
/// Divides the 2D plane into fixed-size cells. Each entity maps to exactly
/// one cell. Neighbourhood queries examine a fixed number of cells, giving
/// O(k) query time where k is the number of cells in the query radius.
///
/// ## Phase 0 status
///
/// Declaration only. Full implementation: Phase 1.
#[allow(dead_code)] // TODO(phase-1): implement UniformGrid
pub struct UniformGrid {
    /// Edge length of each grid cell in simulation length units.
    cell_size: f32,
}

impl UniformGrid {
    /// Creates a new empty uniform grid with the given cell size.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialError::InvalidConfig`] if `cell_size ≤ 0`.
    pub fn new(cell_size: f32) -> SpatialResult<Self> {
        if cell_size <= 0.0 {
            return Err(SpatialError::InvalidConfig {
                message: format!("cell_size must be > 0, got {cell_size}"),
            });
        }
        Ok(Self { cell_size })
    }

    /// Inserts an entity at the given position.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialError::DuplicateEntity`] if the entity is already registered.
    ///
    /// # TODO
    ///
    /// TODO(phase-1): Implement backing storage and cell mapping.
    pub fn insert(&mut self, _id: EntityId, _position: Vec2) -> SpatialResult<()> {
        Ok(())
    }

    /// Updates the position of a registered entity.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialError::UnknownEntity`] if the entity is not registered.
    ///
    /// TODO(phase-1): Implement position update with cell re-bucketing.
    pub fn update(&mut self, _id: EntityId, _position: Vec2) -> SpatialResult<()> {
        Ok(())
    }

    /// Removes an entity from the index.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialError::UnknownEntity`] if the entity is not registered.
    ///
    /// TODO(phase-1): Implement entity removal.
    pub fn remove(&mut self, _id: EntityId) -> SpatialResult<()> {
        Ok(())
    }

    /// Returns all entity IDs within `radius` simulation units of `center`.
    ///
    /// TODO(phase-1): Implement radius query with cell enumeration.
    pub fn query_radius(&self, _center: Vec2, _radius: f32) -> Vec<EntityId> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_grid_rejects_zero_cell_size() {
        assert!(UniformGrid::new(0.0).is_err());
        assert!(UniformGrid::new(-1.0).is_err());
    }

    #[test]
    fn uniform_grid_accepts_positive_cell_size() {
        assert!(UniformGrid::new(16.0).is_ok());
    }

    #[test]
    fn query_radius_returns_empty_placeholder() {
        let grid = UniformGrid::new(16.0).unwrap();
        let results = grid.query_radius(Vec2::ZERO, 50.0);
        assert!(results.is_empty(), "Phase 0 stub returns empty");
    }
}
