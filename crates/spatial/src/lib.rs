//! # Phylon Spatial
//!
//! Spatial indexing structures for efficient entity neighbourhood queries.
//!
//! Three complementary indexing strategies are provided, all sharing a common
//! `SpatialQuery` interface:
//!
//! - **[`UniformGrid`]** — O(1) insert and radius query for dense, uniformly
//!   distributed entities. The primary structure for active chunks.
//! - **`SpatialHash`** — same asymptotic complexity as the uniform grid but
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

use bevy_ecs::entity::Entity;
use common::Vec2;
use std::collections::HashMap;

/// Result type for spatial operations.
pub type SpatialResult<T> = Result<T, SpatialError>;

/// Errors produced by spatial indexing operations.
#[derive(Debug, thiserror::Error)]
pub enum SpatialError {
    /// An entity was inserted with an ID that already exists in the index.
    #[error("entity {0:?} is already registered in the spatial index")]
    DuplicateEntity(Entity),

    /// An operation was attempted on an entity that is not in the index.
    #[error("entity {0:?} is not registered in the spatial index")]
    UnknownEntity(Entity),

    /// A configuration parameter is invalid (e.g., cell size ≤ 0).
    #[error("invalid spatial index configuration: {message}")]
    InvalidConfig {
        /// Description of the invalid parameter.
        message: String,
    },
}

impl common::PhylonError for SpatialError {}

/// # Broad-Phase Uniform Grid Index
///
/// ## 1. What Happens
/// The `UniformGrid` divides the continuous 2D space into discrete grid cells.
/// Entities update their cell buckets as they move, allowing O(1) broad-phase radius queries.
///
/// ## 2. Why It Happens
/// In an engine with 10,000 interacting agents, checking collisions and sensory proximity
/// for every pair requires $O(N^2)$ checks (100,000,000 distance calculations per tick).
/// By bucketing entities into cells roughly the size of their maximum interaction radius,
/// we reduce this to $O(N \cdot k)$ where $k$ is the number of entities in adjacent cells.
///
/// ## 3. How It Happens
/// The spatial space is mapped via modulo arithmetic:
/// $$ Cell_{X, Y} = \left( \lfloor \frac{Pos_x}{Size} \rfloor, \lfloor \frac{Pos_y}{Size} \rfloor \right) $$
/// Queries calculate the bounding box of the query radius, convert to cell coordinates,
/// and iterate only through the entities in that subset of buckets.
///
/// Rebuilt from scratch every tick by callers (see `UniformGrid::clear` /
/// `rebuild`), so it doesn't need to handle every incremental-update edge
/// case efficiently — `update`/`remove` are provided for callers that do want
/// to maintain it incrementally across a tick.
pub struct UniformGrid {
    /// Edge length of each grid cell in simulation length units.
    cell_size: f32,
    /// Bucketed entities by cell coordinate.
    cells: HashMap<(i32, i32), Vec<Entity>>,
    /// Reverse index: last known position of each registered entity, used to
    /// find its current cell on `update`/`remove` without a linear scan.
    positions: HashMap<Entity, Vec2>,
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
        Ok(Self {
            cell_size,
            cells: HashMap::new(),
            positions: HashMap::new(),
        })
    }

    fn cell_of(&self, position: Vec2) -> (i32, i32) {
        (
            (position.x / self.cell_size).floor() as i32,
            (position.y / self.cell_size).floor() as i32,
        )
    }

    /// Removes all entities from the index, keeping the allocated buckets.
    pub fn clear(&mut self) {
        self.cells.clear();
        self.positions.clear();
    }

    /// Inserts an entity at the given position.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialError::DuplicateEntity`] if the entity is already registered.
    pub fn insert(&mut self, id: Entity, position: Vec2) -> SpatialResult<()> {
        if self.positions.contains_key(&id) {
            return Err(SpatialError::DuplicateEntity(id));
        }
        let cell = self.cell_of(position);
        self.cells.entry(cell).or_default().push(id);
        self.positions.insert(id, position);
        Ok(())
    }

    /// Updates the position of a registered entity.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialError::UnknownEntity`] if the entity is not registered.
    pub fn update(&mut self, id: Entity, position: Vec2) -> SpatialResult<()> {
        let Some(old_position) = self.positions.get(&id).copied() else {
            return Err(SpatialError::UnknownEntity(id));
        };
        let old_cell = self.cell_of(old_position);
        let new_cell = self.cell_of(position);
        if old_cell != new_cell {
            if let Some(bucket) = self.cells.get_mut(&old_cell) {
                bucket.retain(|&e| e != id);
            }
            self.cells.entry(new_cell).or_default().push(id);
        }
        self.positions.insert(id, position);
        Ok(())
    }

    /// Removes an entity from the index.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialError::UnknownEntity`] if the entity is not registered.
    pub fn remove(&mut self, id: Entity) -> SpatialResult<()> {
        let Some(position) = self.positions.remove(&id) else {
            return Err(SpatialError::UnknownEntity(id));
        };
        let cell = self.cell_of(position);
        if let Some(bucket) = self.cells.get_mut(&cell) {
            bucket.retain(|&e| e != id);
        }
        Ok(())
    }

    /// Returns all entity IDs within `radius` simulation units of `center`.
    ///
    /// Scans the grid cells overlapping the query's bounding box, then
    /// filters candidates by exact distance so results are precise rather
    /// than just "same cell block".
    pub fn query_radius(&self, center: Vec2, radius: f32) -> Vec<Entity> {
        if radius <= 0.0 {
            return Vec::new();
        }
        let min_cell = self.cell_of(center - Vec2::splat(radius));
        let max_cell = self.cell_of(center + Vec2::splat(radius));
        let radius_sq = radius * radius;

        let mut results = Vec::new();
        for cy in min_cell.1..=max_cell.1 {
            for cx in min_cell.0..=max_cell.0 {
                let Some(bucket) = self.cells.get(&(cx, cy)) else {
                    continue;
                };
                for &id in bucket {
                    if let Some(&pos) = self.positions.get(&id) {
                        if pos.distance_squared(center) <= radius_sq {
                            results.push(id);
                        }
                    }
                }
            }
        }
        results
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
    fn query_radius_on_empty_grid_returns_empty() {
        let grid = UniformGrid::new(16.0).unwrap();
        let results = grid.query_radius(Vec2::ZERO, 50.0);
        assert!(results.is_empty());
    }

    #[test]
    fn query_radius_finds_nearby_and_excludes_far_entities() {
        let mut grid = UniformGrid::new(16.0).unwrap();
        let near = Entity::from_raw(1);
        let far = Entity::from_raw(2);
        grid.insert(near, Vec2::new(5.0, 5.0)).unwrap();
        grid.insert(far, Vec2::new(500.0, 500.0)).unwrap();

        let results = grid.query_radius(Vec2::ZERO, 50.0);
        assert_eq!(results, vec![near]);
    }

    #[test]
    fn insert_rejects_duplicate_entity() {
        let mut grid = UniformGrid::new(16.0).unwrap();
        let id = Entity::from_raw(1);
        grid.insert(id, Vec2::ZERO).unwrap();
        assert!(matches!(
            grid.insert(id, Vec2::ZERO),
            Err(SpatialError::DuplicateEntity(_))
        ));
    }

    #[test]
    fn update_moves_entity_between_cells() {
        let mut grid = UniformGrid::new(16.0).unwrap();
        let id = Entity::from_raw(1);
        grid.insert(id, Vec2::ZERO).unwrap();
        grid.update(id, Vec2::new(1000.0, 1000.0)).unwrap();

        assert!(grid.query_radius(Vec2::ZERO, 5.0).is_empty());
        assert_eq!(grid.query_radius(Vec2::new(1000.0, 1000.0), 5.0), vec![id]);
    }

    #[test]
    fn remove_drops_entity_from_queries() {
        let mut grid = UniformGrid::new(16.0).unwrap();
        let id = Entity::from_raw(1);
        grid.insert(id, Vec2::ZERO).unwrap();
        grid.remove(id).unwrap();
        assert!(grid.query_radius(Vec2::ZERO, 5.0).is_empty());
        assert!(matches!(
            grid.remove(id),
            Err(SpatialError::UnknownEntity(_))
        ));
    }
}
