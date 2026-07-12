use crate::index::SpatialIndex;
use crate::{SpatialError, SpatialResult};
use bevy_ecs::entity::Entity;
use common::Vec3;
use std::collections::HashMap;

/// A single node of the [`Octree`] — either a leaf holding entities
/// directly, or an internal node with exactly eight children (one per
/// octant of its bounding box, in a fixed `(x >= center.x, y >= center.y, z
/// >= center.z)` bit-order: index 0 = `(-,-,-)`, 1 = `(+,-,-)`, 2 = `(-,+,-)`,
/// 3 = `(+,+,-)`, 4 = `(-,-,+)`, 5 = `(+,-,+)`, 6 = `(-,+,+)`, 7 = `(+,+,+)`).
struct OctNode {
    min: Vec3,
    max: Vec3,
    entities: Vec<Entity>,
    children: Option<Box<[OctNode; 8]>>,
}

impl OctNode {
    fn new(min: Vec3, max: Vec3) -> Self {
        Self {
            min,
            max,
            entities: Vec::new(),
            children: None,
        }
    }

    fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    fn contains(&self, pos: Vec3) -> bool {
        pos.x >= self.min.x
            && pos.x < self.max.x
            && pos.y >= self.min.y
            && pos.y < self.max.y
            && pos.z >= self.min.z
            && pos.z < self.max.z
    }

    /// Index of the child octant `pos` falls into, assuming
    /// `self.contains(pos)` — see this struct's own doc comment for the bit
    /// order. A free function of `center` rather than a `&self` method so
    /// callers can compute it while already holding a disjoint mutable
    /// borrow of `self.children` — see the `insert`/`remove` call sites
    /// below.
    fn octant_of(center: Vec3, pos: Vec3) -> usize {
        let bit_x = (pos.x >= center.x) as usize;
        let bit_y = (pos.y >= center.y) as usize;
        let bit_z = (pos.z >= center.z) as usize;
        bit_x | (bit_y << 1) | (bit_z << 2)
    }

    fn split(&mut self) {
        let c = self.center();
        let mk = |min: Vec3, max: Vec3| OctNode::new(min, max);
        let children = [
            mk(self.min, c), // (-,-,-)
            mk(
                Vec3::new(c.x, self.min.y, self.min.z),
                Vec3::new(self.max.x, c.y, c.z),
            ), // (+,-,-)
            mk(
                Vec3::new(self.min.x, c.y, self.min.z),
                Vec3::new(c.x, self.max.y, c.z),
            ), // (-,+,-)
            mk(
                Vec3::new(c.x, c.y, self.min.z),
                Vec3::new(self.max.x, self.max.y, c.z),
            ), // (+,+,-)
            mk(
                Vec3::new(self.min.x, self.min.y, c.z),
                Vec3::new(c.x, c.y, self.max.z),
            ), // (-,-,+)
            mk(
                Vec3::new(c.x, self.min.y, c.z),
                Vec3::new(self.max.x, c.y, self.max.z),
            ), // (+,-,+)
            mk(
                Vec3::new(self.min.x, c.y, c.z),
                Vec3::new(c.x, self.max.y, self.max.z),
            ), // (-,+,+)
            mk(c, self.max), // (+,+,+)
        ];
        self.children = Some(Box::new(children));
    }

    /// Inserts an already-bounds-checked entity, splitting this leaf (and
    /// redistributing its current contents) if it grows past `max_entities`
    /// and hasn't hit `max_depth` yet.
    #[allow(clippy::too_many_arguments)]
    fn insert(
        &mut self,
        id: Entity,
        pos: Vec3,
        positions: &HashMap<Entity, Vec3>,
        max_entities: usize,
        max_depth: u32,
        depth: u32,
    ) {
        let center = self.center();
        let q = OctNode::octant_of(center, pos);
        if let Some(children) = &mut self.children {
            children[q].insert(id, pos, positions, max_entities, max_depth, depth + 1);
            return;
        }

        self.entities.push(id);

        if self.entities.len() > max_entities && depth < max_depth {
            self.split();
            let entities = std::mem::take(&mut self.entities);
            let children = self.children.as_mut().unwrap();
            for e in entities {
                // Every entity reaching this point was already inserted via
                // this same path, so it's guaranteed present in `positions`.
                let p = positions[&e];
                let q = OctNode::octant_of(center, p);
                children[q].insert(e, p, positions, max_entities, max_depth, depth + 1);
            }
        }
    }

    /// Removes `id` (known to be at `pos`) from whichever leaf holds it.
    /// Does not merge/collapse children back on underflow — see
    /// [`Octree`]'s doc comment for why that's an accepted limitation.
    fn remove(&mut self, id: Entity, pos: Vec3) {
        let q = OctNode::octant_of(self.center(), pos);
        if let Some(children) = &mut self.children {
            children[q].remove(id, pos);
            return;
        }
        self.entities.retain(|&e| e != id);
    }

    /// Sphere-vs-AABB overlap test: clamp the sphere center into the node's
    /// box, then check whether the clamped point is within `radius`.
    fn intersects_sphere(&self, center: Vec3, radius: f32) -> bool {
        let closest = Vec3::new(
            center.x.clamp(self.min.x, self.max.x),
            center.y.clamp(self.min.y, self.max.y),
            center.z.clamp(self.min.z, self.max.z),
        );
        closest.distance_squared(center) <= radius * radius
    }

    fn query_radius(
        &self,
        center: Vec3,
        radius: f32,
        positions: &HashMap<Entity, Vec3>,
        out: &mut Vec<Entity>,
    ) {
        if !self.intersects_sphere(center, radius) {
            return;
        }
        if let Some(children) = &self.children {
            for child in children.iter() {
                child.query_radius(center, radius, positions, out);
            }
            return;
        }
        let radius_sq = radius * radius;
        for &id in &self.entities {
            if let Some(&pos) = positions.get(&id) {
                if pos.distance_squared(center) <= radius_sq {
                    out.push(id);
                }
            }
        }
    }
}

/// # Bounded Octree Index
///
/// ## Purpose
/// `Octree` recursively splits a fixed cuboid region into eight octants
/// wherever entity density exceeds `max_entities_per_node`, up to
/// `max_depth`, giving logarithmic-depth radius queries over a sparse or
/// unevenly distributed population without scanning empty space.
///
/// ## Architecture
/// Insertion descends from the root, splitting a leaf into eight children
/// (one per octant, in a fixed `(x, y, z)` sign bit-order) once
/// it holds more than `max_entities_per_node` entities, stopping at
/// `max_depth` regardless of count. Queries test each node's bounding box
/// against the query sphere before descending, skipping entire subtrees that
/// can't possibly contain a match.
///
/// ## Why an Octree (vs. a grid)
/// [`UniformGrid`](crate::UniformGrid) and [`SpatialHash`](crate::SpatialHash)
/// both assume a roughly uniform cell size tuned to the typical query
/// radius — cheap for dense, evenly-spread populations, but wasteful for
/// sparse, long-range queries (e.g. spectator-mode "nearest interesting
/// organism" across a mostly-empty world): a uniform grid sized for
/// close-range queries forces scanning many empty cells to cover a large
/// radius. An octree's cell size adapts to local density instead, so a
/// large empty region costs only a handful of coarse nodes rather than many
/// empty grid cells.
///
/// ## Complexity
/// Insert, update, and remove are `O(depth)` (bounded by `max_depth`).
/// `query_radius` prunes subtrees whose bounding box doesn't intersect the
/// query sphere, so its cost scales with how much of the tree the query
/// sphere actually overlaps rather than total entity count — close to
/// `O(log n + k)` for `k` matching entities in a well-balanced tree, degrading
/// toward `O(n)` only if entities cluster so densely that `max_depth` is hit
/// and a single leaf ends up holding far more than `max_entities_per_node`.
///
/// ## Design Decisions
/// CPU-side spatial indexing (this type) uses a tree, while the GPU-side
/// physics broad-phase (`gpu::physics_pipeline`) uses a fixed-size spatial
/// hash instead — the two sides face different constraints. GPU compute
/// buffers need a fixed layout decided at buffer-creation time, so a
/// dynamically-shaped tree isn't a natural fit there; a dense grid sized to
/// match this octree's finest resolution would cost far more GPU memory than
/// the fixed hash table `gpu::physics_pipeline` actually allocates, most of
/// it empty for a sparse population. On the CPU side, a tree's adaptive
/// resolution is worth the extra pointer-chasing since queries there are
/// less latency-sensitive and don't pay a fixed-buffer-size penalty.
///
/// **Accepted limitation:** nodes never merge back on removal, so a tree
/// that briefly reached a high population stays deeply split even after
/// entities leave — this trades a full incremental-removal implementation
/// for simplicity, appropriate for the "static or slow-moving objects" use
/// case this index targets (per the crate's module doc). A caller with a
/// fast-churning population should rebuild the tree periodically (`clear`
/// then re-`insert` everything) rather than rely on long-run incremental
/// `remove`/`update`.
pub struct Octree {
    root: OctNode,
    max_entities_per_node: usize,
    max_depth: u32,
    positions: HashMap<Entity, Vec3>,
}

impl Octree {
    /// Creates a new empty octree over the fixed region `[min, max)`.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialError::InvalidConfig`] if `min.x >= max.x`, `min.y
    /// >= max.y`, or `min.z >= max.z`, or if `max_entities_per_node == 0`.
    pub fn new(
        min: Vec3,
        max: Vec3,
        max_entities_per_node: usize,
        max_depth: u32,
    ) -> SpatialResult<Self> {
        if min.x >= max.x || min.y >= max.y || min.z >= max.z {
            return Err(SpatialError::InvalidConfig {
                message: format!("octree bounds must satisfy min < max, got {min:?}..{max:?}"),
            });
        }
        if max_entities_per_node == 0 {
            return Err(SpatialError::InvalidConfig {
                message: "max_entities_per_node must be > 0".to_string(),
            });
        }
        Ok(Self {
            root: OctNode::new(min, max),
            max_entities_per_node,
            max_depth,
            positions: HashMap::new(),
        })
    }

    /// Removes all entities from the index, collapsing all splits back to
    /// a single empty root node.
    pub fn clear(&mut self) {
        let (min, max) = (self.root.min, self.root.max);
        self.root = OctNode::new(min, max);
        self.positions.clear();
    }

    /// Inserts an entity at the given position.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialError::DuplicateEntity`] if the entity is already
    /// registered, or [`SpatialError::OutOfBounds`] if `position` falls
    /// outside this octree's fixed region.
    pub fn insert(&mut self, id: Entity, position: Vec3) -> SpatialResult<()> {
        if self.positions.contains_key(&id) {
            return Err(SpatialError::DuplicateEntity(id));
        }
        if !self.root.contains(position) {
            return Err(SpatialError::OutOfBounds(id));
        }
        // Record the position *before* descending — if this insert pushes a
        // leaf past `max_entities_per_node`, the resulting split
        // redistributes every entity in that leaf (including this one, just
        // pushed) by looking up each one's position in this map.
        self.positions.insert(id, position);
        self.root.insert(
            id,
            position,
            &self.positions,
            self.max_entities_per_node,
            self.max_depth,
            0,
        );
        Ok(())
    }

    /// Updates the position of a registered entity (implemented as
    /// remove-then-reinsert — see the type's doc comment on why merging
    /// isn't attempted incrementally).
    ///
    /// # Errors
    ///
    /// Returns [`SpatialError::UnknownEntity`] if the entity is not
    /// registered, or [`SpatialError::OutOfBounds`] if `position` falls
    /// outside this octree's fixed region.
    pub fn update(&mut self, id: Entity, position: Vec3) -> SpatialResult<()> {
        let Some(&old_position) = self.positions.get(&id) else {
            return Err(SpatialError::UnknownEntity(id));
        };
        if !self.root.contains(position) {
            return Err(SpatialError::OutOfBounds(id));
        }
        self.root.remove(id, old_position);
        // See the matching comment in `insert` — the position must already
        // be recorded before descending, in case this insert triggers a
        // same-call split.
        self.positions.insert(id, position);
        self.root.insert(
            id,
            position,
            &self.positions,
            self.max_entities_per_node,
            self.max_depth,
            0,
        );
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
        self.root.remove(id, position);
        Ok(())
    }

    /// Returns all entity IDs within `radius` simulation units of `center`.
    pub fn query_radius(&self, center: Vec3, radius: f32) -> Vec<Entity> {
        if radius <= 0.0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        self.root
            .query_radius(center, radius, &self.positions, &mut out);
        out
    }
}

impl SpatialIndex for Octree {
    fn insert(&mut self, id: Entity, position: Vec3) -> SpatialResult<()> {
        Octree::insert(self, id, position)
    }

    fn update(&mut self, id: Entity, position: Vec3) -> SpatialResult<()> {
        Octree::update(self, id, position)
    }

    fn remove(&mut self, id: Entity) -> SpatialResult<()> {
        Octree::remove(self, id)
    }

    fn query_radius(&self, center: Vec3, radius: f32) -> Vec<Entity> {
        Octree::query_radius(self, center, radius)
    }

    fn clear(&mut self) {
        Octree::clear(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_tree() -> Octree {
        Octree::new(
            Vec3::new(-1000.0, -1000.0, -1000.0),
            Vec3::new(1000.0, 1000.0, 1000.0),
            4,
            6,
        )
        .unwrap()
    }

    #[test]
    fn rejects_inverted_bounds() {
        assert!(Octree::new(
            Vec3::new(10.0, 10.0, 10.0),
            Vec3::new(-10.0, -10.0, -10.0),
            4,
            6
        )
        .is_err());
    }

    #[test]
    fn rejects_zero_max_entities() {
        assert!(Octree::new(
            Vec3::new(-10.0, -10.0, -10.0),
            Vec3::new(10.0, 10.0, 10.0),
            0,
            6
        )
        .is_err());
    }

    #[test]
    fn insert_rejects_out_of_bounds_position() {
        let mut tree = small_tree();
        let id = Entity::from_raw(1);
        assert!(matches!(
            tree.insert(id, Vec3::new(5000.0, 5000.0, 5000.0)),
            Err(SpatialError::OutOfBounds(_))
        ));
    }

    #[test]
    fn insert_rejects_duplicate_entity() {
        let mut tree = small_tree();
        let id = Entity::from_raw(1);
        tree.insert(id, Vec3::ZERO).unwrap();
        assert!(matches!(
            tree.insert(id, Vec3::ZERO),
            Err(SpatialError::DuplicateEntity(_))
        ));
    }

    #[test]
    fn query_radius_on_empty_tree_returns_empty() {
        let tree = small_tree();
        assert!(tree.query_radius(Vec3::ZERO, 50.0).is_empty());
    }

    #[test]
    fn query_radius_finds_nearby_and_excludes_far_entities() {
        let mut tree = small_tree();
        let near = Entity::from_raw(1);
        let far = Entity::from_raw(2);
        tree.insert(near, Vec3::new(5.0, 5.0, 5.0)).unwrap();
        tree.insert(far, Vec3::new(900.0, 900.0, 900.0)).unwrap();

        let results = tree.query_radius(Vec3::ZERO, 50.0);
        assert_eq!(results, vec![near]);
    }

    /// A genuinely-3D correctness check: two entities share the same XY
    /// position but differ only in `Z` — the octree must distinguish "near"
    /// from "far" using all three axes, not just X/Y.
    #[test]
    fn query_radius_distinguishes_entities_by_z_alone() {
        let mut tree = small_tree();
        let near = Entity::from_raw(1);
        let far = Entity::from_raw(2);
        tree.insert(near, Vec3::new(0.0, 0.0, 5.0)).unwrap();
        tree.insert(far, Vec3::new(0.0, 0.0, 900.0)).unwrap();

        let results = tree.query_radius(Vec3::ZERO, 50.0);
        assert_eq!(results, vec![near]);
    }

    #[test]
    fn splitting_past_capacity_preserves_all_entities() {
        // max_entities_per_node = 4, so inserting 50 clustered entities
        // forces several splits — every one of them must still be
        // findable afterward.
        let mut tree = small_tree();
        for i in 0..50u32 {
            let pos = Vec3::new((i % 10) as f32, (i / 10) as f32, 0.0);
            tree.insert(Entity::from_raw(i), pos).unwrap();
        }
        let results = tree.query_radius(Vec3::new(4.5, 2.0, 0.0), 100.0);
        assert_eq!(results.len(), 50);
    }

    #[test]
    fn update_moves_entity_and_respects_bounds() {
        let mut tree = small_tree();
        let id = Entity::from_raw(1);
        tree.insert(id, Vec3::ZERO).unwrap();
        tree.update(id, Vec3::new(500.0, 500.0, 500.0)).unwrap();

        assert!(tree.query_radius(Vec3::ZERO, 5.0).is_empty());
        assert_eq!(
            tree.query_radius(Vec3::new(500.0, 500.0, 500.0), 5.0),
            vec![id]
        );

        assert!(matches!(
            tree.update(id, Vec3::new(5000.0, 5000.0, 5000.0)),
            Err(SpatialError::OutOfBounds(_))
        ));
    }

    #[test]
    fn remove_drops_entity_from_queries() {
        let mut tree = small_tree();
        let id = Entity::from_raw(1);
        tree.insert(id, Vec3::ZERO).unwrap();
        tree.remove(id).unwrap();
        assert!(tree.query_radius(Vec3::ZERO, 5.0).is_empty());
        assert!(matches!(
            tree.remove(id),
            Err(SpatialError::UnknownEntity(_))
        ));
    }

    #[test]
    fn clear_resets_to_empty_root() {
        let mut tree = small_tree();
        for i in 0..50u32 {
            let pos = Vec3::new((i % 10) as f32, (i / 10) as f32, 0.0);
            tree.insert(Entity::from_raw(i), pos).unwrap();
        }
        tree.clear();
        assert!(tree.query_radius(Vec3::ZERO, 2000.0).is_empty());
        // Confirms the root's own bounds survive `clear` (re-insertion at a
        // previously-valid position must still succeed, not error as if
        // bounds had collapsed to a point).
        tree.insert(Entity::from_raw(999), Vec3::ZERO).unwrap();
    }
}
