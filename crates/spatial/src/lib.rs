//! # Phylon Spatial
//!
//! Spatial indexing structures for efficient entity neighbourhood queries.
//!
//! ## Currently implemented
//!
//! All three share the [`SpatialIndex`] trait (`insert`/`update`/`remove`/
//! `query_radius`/`clear`), so a caller can pick whichever fits its access
//! pattern:
//!
//! - **[`UniformGrid`]** — O(1) insert and radius query for dense, uniformly
//!   distributed entities, via cell-bucketed `HashMap` storage. This is the
//!   index every pre-existing caller (physics broad-phase, sensing,
//!   reproduction proximity search, ecology foraging) already uses.
//! - **[`SpatialHash`]** — same cell-bucketing idea, but a fixed-size hash
//!   table instead of a per-cell `HashMap` entry, for populations spread
//!   unevenly across a large or unbounded area.
//! - **[`Octree`]** — sparse, logarithmic-depth structure over a fixed
//!   bounded region, for long-range queries on static or slow-moving
//!   objects.
//!
//! ## Dependency rules
//!
//! No rendering, UI, or storage types may appear in this crate.

#![warn(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::entity::Entity;

mod hash;
mod index;
mod octree;
mod uniform_grid;

pub use hash::SpatialHash;
pub use index::SpatialIndex;
pub use octree::Octree;
pub use uniform_grid::UniformGrid;

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

    /// An entity's position falls outside a bounded index's fixed region
    /// (currently only [`Octree`] has bounds — [`UniformGrid`] and
    /// [`SpatialHash`] are unbounded).
    #[error("entity {0:?}'s position is outside the spatial index's bounds")]
    OutOfBounds(Entity),
}

impl common::PhylonError for SpatialError {}
