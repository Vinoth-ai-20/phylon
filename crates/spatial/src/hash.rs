use crate::index::SpatialIndex;
use crate::{SpatialError, SpatialResult};
use bevy_ecs::entity::Entity;
use common::Vec2;
use std::collections::HashMap;

/// # Fixed-Table Spatial Hash
///
/// ## 1. What Happens
/// `SpatialHash` divides space into the same kind of grid cells as
/// [`UniformGrid`](crate::UniformGrid), but instead of one `HashMap` entry
/// per populated cell, every cell hashes into one of a **fixed** number of
/// buckets (`table_size`, rounded up to a power of two). Multiple distant
/// cells may collide into the same bucket.
///
/// ## 2. Why It Happens
/// `UniformGrid`'s per-cell `HashMap` entry count scales with the number of
/// *occupied* cells — fine for a population clustered in one region, but a
/// sparse population spread very unevenly across a large or unbounded area
/// (the spec's chunked, infinite-expanding world model) can still end up
/// with a large, cache-unfriendly set of scattered `HashMap` entries.
/// `SpatialHash` trades exact per-cell bucketing for a flat, fixed-size
/// `Vec<Vec<Entity>>` table — bounded memory and a single contiguous
/// allocation regardless of how far entities are spread, at the cost of
/// hash collisions occasionally grouping distant cells into one bucket
/// (query correctness is unaffected — see below — only candidate-scan cost).
///
/// ## 3. How It Happens
/// A cell coordinate hashes into a bucket via a standard 2D integer mix
/// (`hash(x, y) = (x * P1) XOR (y * P2), mod table_size`, `P1`/`P2` large
/// primes — this is the same mixing function commonly used for spatial
/// hashing, e.g. Optimized Spatial Hashing for Collision Detection). Query
/// correctness holds despite collisions: every candidate entity found in a
/// scanned bucket is still filtered by its exact stored position before
/// being returned, exactly like `UniformGrid` — collisions only ever add
/// extra candidates to filter, never drop real ones, and each bucket index
/// touched by the query's cell range is visited exactly once (via a
/// dedup'd set of bucket indices), so a collision never causes an entity to
/// be returned twice.
pub struct SpatialHash {
    cell_size: f32,
    table_size: usize,
    buckets: Vec<Vec<Entity>>,
    positions: HashMap<Entity, Vec2>,
}

impl SpatialHash {
    /// Creates a new empty spatial hash with the given cell size and a
    /// requested bucket-table size (rounded up to the next power of two,
    /// minimum 16).
    ///
    /// # Errors
    ///
    /// Returns [`SpatialError::InvalidConfig`] if `cell_size ≤ 0`.
    pub fn new(cell_size: f32, table_size_hint: usize) -> SpatialResult<Self> {
        if cell_size <= 0.0 {
            return Err(SpatialError::InvalidConfig {
                message: format!("cell_size must be > 0, got {cell_size}"),
            });
        }
        let table_size = table_size_hint.max(16).next_power_of_two();
        Ok(Self {
            cell_size,
            table_size,
            buckets: (0..table_size).map(|_| Vec::new()).collect(),
            positions: HashMap::new(),
        })
    }

    fn cell_of(&self, position: Vec2) -> (i32, i32) {
        (
            (position.x / self.cell_size).floor() as i32,
            (position.y / self.cell_size).floor() as i32,
        )
    }

    /// Hashes a cell coordinate to a bucket index. `table_size` is a power
    /// of two, so `% table_size` is equivalent to `& (table_size - 1)` — the
    /// standard fast-path for power-of-two hash tables.
    fn bucket_of(&self, cell: (i32, i32)) -> usize {
        const P1: i64 = 73_856_093;
        const P2: i64 = 19_349_663;
        let h = (cell.0 as i64).wrapping_mul(P1) ^ (cell.1 as i64).wrapping_mul(P2);
        (h.unsigned_abs() as usize) & (self.table_size - 1)
    }

    /// Creates a new empty spatial hash with a sensible default table size
    /// (1024 buckets).
    pub fn with_cell_size(cell_size: f32) -> SpatialResult<Self> {
        Self::new(cell_size, 1024)
    }

    /// Removes all entities from the index, keeping the allocated buckets.
    pub fn clear(&mut self) {
        for bucket in &mut self.buckets {
            bucket.clear();
        }
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
        let bucket = self.bucket_of(self.cell_of(position));
        self.buckets[bucket].push(id);
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
        let old_bucket = self.bucket_of(self.cell_of(old_position));
        let new_bucket = self.bucket_of(self.cell_of(position));
        if old_bucket != new_bucket {
            self.buckets[old_bucket].retain(|&e| e != id);
            self.buckets[new_bucket].push(id);
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
        let bucket = self.bucket_of(self.cell_of(position));
        self.buckets[bucket].retain(|&e| e != id);
        Ok(())
    }

    /// Returns all entity IDs within `radius` simulation units of `center`.
    pub fn query_radius(&self, center: Vec2, radius: f32) -> Vec<Entity> {
        if radius <= 0.0 {
            return Vec::new();
        }
        let min_cell = self.cell_of(center - Vec2::splat(radius));
        let max_cell = self.cell_of(center + Vec2::splat(radius));
        let radius_sq = radius * radius;

        // Dedup bucket indices before scanning — two distinct cells in the
        // query range may collide into the same bucket, and each bucket
        // must be scanned exactly once or a colliding entity could be
        // returned twice.
        let mut visited_buckets: Vec<usize> = Vec::new();
        let mut results = Vec::new();
        for cy in min_cell.1..=max_cell.1 {
            for cx in min_cell.0..=max_cell.0 {
                let bucket_idx = self.bucket_of((cx, cy));
                if visited_buckets.contains(&bucket_idx) {
                    continue;
                }
                visited_buckets.push(bucket_idx);

                for &id in &self.buckets[bucket_idx] {
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

impl SpatialIndex for SpatialHash {
    fn insert(&mut self, id: Entity, position: Vec2) -> SpatialResult<()> {
        SpatialHash::insert(self, id, position)
    }

    fn update(&mut self, id: Entity, position: Vec2) -> SpatialResult<()> {
        SpatialHash::update(self, id, position)
    }

    fn remove(&mut self, id: Entity) -> SpatialResult<()> {
        SpatialHash::remove(self, id)
    }

    fn query_radius(&self, center: Vec2, radius: f32) -> Vec<Entity> {
        SpatialHash::query_radius(self, center, radius)
    }

    fn clear(&mut self) {
        SpatialHash::clear(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_zero_cell_size() {
        assert!(SpatialHash::new(0.0, 16).is_err());
        assert!(SpatialHash::new(-1.0, 16).is_err());
    }

    #[test]
    fn table_size_rounds_up_to_power_of_two() {
        let hash = SpatialHash::new(16.0, 100).unwrap();
        assert_eq!(hash.table_size, 128);
    }

    #[test]
    fn table_size_has_a_floor_of_16() {
        let hash = SpatialHash::new(16.0, 1).unwrap();
        assert_eq!(hash.table_size, 16);
    }

    #[test]
    fn query_radius_on_empty_hash_returns_empty() {
        let hash = SpatialHash::with_cell_size(16.0).unwrap();
        assert!(hash.query_radius(Vec2::ZERO, 50.0).is_empty());
    }

    #[test]
    fn query_radius_finds_nearby_and_excludes_far_entities() {
        let mut hash = SpatialHash::with_cell_size(16.0).unwrap();
        let near = Entity::from_raw(1);
        let far = Entity::from_raw(2);
        hash.insert(near, Vec2::new(5.0, 5.0)).unwrap();
        hash.insert(far, Vec2::new(5000.0, 5000.0)).unwrap();

        let results = hash.query_radius(Vec2::ZERO, 50.0);
        assert_eq!(results, vec![near]);
    }

    #[test]
    fn insert_rejects_duplicate_entity() {
        let mut hash = SpatialHash::with_cell_size(16.0).unwrap();
        let id = Entity::from_raw(1);
        hash.insert(id, Vec2::ZERO).unwrap();
        assert!(matches!(
            hash.insert(id, Vec2::ZERO),
            Err(SpatialError::DuplicateEntity(_))
        ));
    }

    #[test]
    fn update_moves_entity_between_buckets() {
        let mut hash = SpatialHash::with_cell_size(16.0).unwrap();
        let id = Entity::from_raw(1);
        hash.insert(id, Vec2::ZERO).unwrap();
        hash.update(id, Vec2::new(1000.0, 1000.0)).unwrap();

        assert!(hash.query_radius(Vec2::ZERO, 5.0).is_empty());
        assert_eq!(hash.query_radius(Vec2::new(1000.0, 1000.0), 5.0), vec![id]);
    }

    #[test]
    fn remove_drops_entity_from_queries() {
        let mut hash = SpatialHash::with_cell_size(16.0).unwrap();
        let id = Entity::from_raw(1);
        hash.insert(id, Vec2::ZERO).unwrap();
        hash.remove(id).unwrap();
        assert!(hash.query_radius(Vec2::ZERO, 5.0).is_empty());
        assert!(matches!(
            hash.remove(id),
            Err(SpatialError::UnknownEntity(_))
        ));
    }

    /// Forces a small table (16 buckets) with entities spread far enough
    /// apart that some of their cells are very likely to collide into the
    /// same bucket — the test verifies that collisions never cause a
    /// spurious duplicate or a missed exact-distance filter.
    #[test]
    fn small_table_with_many_entities_never_returns_duplicates_or_false_positives() {
        let mut hash = SpatialHash::new(10.0, 16).unwrap();
        for i in 0..200u32 {
            let angle = i as f32 * 0.31;
            let dist = i as f32 * 37.0;
            let pos = Vec2::new(angle.cos() * dist, angle.sin() * dist);
            hash.insert(Entity::from_raw(i), pos).unwrap();
        }

        let results = hash.query_radius(Vec2::ZERO, 300.0);

        // No duplicates.
        let mut seen = std::collections::HashSet::new();
        for &id in &results {
            assert!(seen.insert(id), "duplicate entity {id:?} in results");
        }

        // No false positives: every returned entity is really within radius.
        for &id in &results {
            let pos = hash.positions[&id];
            assert!(pos.length() <= 300.0 + f32::EPSILON);
        }
    }
}
