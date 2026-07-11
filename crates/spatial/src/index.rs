use crate::SpatialResult;
use bevy_ecs::entity::Entity;
use common::Vec3;

/// Common interface shared by every spatial index in this crate.
///
/// Lets a caller (physics broad-phase, sensing, reproduction proximity
/// search, ecology foraging) pick whichever index fits its access pattern
/// — [`UniformGrid`](crate::UniformGrid) for dense, uniformly distributed
/// entities, [`SpatialHash`](crate::SpatialHash) for uneven density,
/// [`Octree`](crate::Octree) for sparse long-range queries — without
/// changing call-site code beyond construction. `Vec3`-based since Phase 8,
/// Epic 8.9 (previously `Vec2`, widened alongside every index's own
/// inherent methods in Epic 8.0/8.9) — confirmed via a workspace-wide
/// search that no live caller uses any index through this trait, only
/// through each type's own inherent methods directly.
pub trait SpatialIndex {
    /// Inserts an entity at the given position.
    ///
    /// # Errors
    ///
    /// Returns an error if the entity is already registered (or, for
    /// bounded indices like [`Octree`](crate::Octree), if `position` falls
    /// outside the index's bounds).
    fn insert(&mut self, id: Entity, position: Vec3) -> SpatialResult<()>;

    /// Updates the position of a registered entity.
    ///
    /// # Errors
    ///
    /// Returns an error if the entity is not registered.
    fn update(&mut self, id: Entity, position: Vec3) -> SpatialResult<()>;

    /// Removes an entity from the index.
    ///
    /// # Errors
    ///
    /// Returns an error if the entity is not registered.
    fn remove(&mut self, id: Entity) -> SpatialResult<()>;

    /// Returns all entity IDs within `radius` simulation units of `center`.
    fn query_radius(&self, center: Vec3, radius: f32) -> Vec<Entity>;

    /// Removes all entities from the index.
    fn clear(&mut self);
}
