//! Spatial indexing structures for broad-phase queries.

use common::{EntityId, IVec2, Vec2};
use rustc_hash::FxHashMap;

/// A flat hash-based uniform grid for O(1) expected cell lookups.
pub struct UniformGrid {
    cell_size: f32,
    cells: FxHashMap<IVec2, Vec<EntityId>>,
}

impl UniformGrid {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            cells: FxHashMap::default(),
        }
    }

    /// Returns the size of each grid cell.
    pub fn cell_size(&self) -> f32 {
        self.cell_size
    }

    /// Computes the 2D cell coordinate for a given continuous position.
    #[inline]
    pub fn pos_to_cell(&self, pos: Vec2) -> IVec2 {
        IVec2::new(
            (pos.x / self.cell_size).floor() as i32,
            (pos.y / self.cell_size).floor() as i32,
        )
    }

    /// Clears the grid without deallocating backing memory.
    pub fn clear(&mut self) {
        for bucket in self.cells.values_mut() {
            bucket.clear();
        }
    }

    /// Inserts an entity at a given position.
    pub fn insert(&mut self, id: EntityId, pos: Vec2) {
        let cell = self.pos_to_cell(pos);
        self.cells.entry(cell).or_default().push(id);
    }

    /// Returns an iterator over all entity IDs in a specific cell.
    pub fn query_cell(&self, cell: IVec2) -> impl Iterator<Item = &EntityId> {
        self.cells
            .get(&cell)
            .into_iter()
            .flat_map(|bucket| bucket.iter())
    }

    /// Retains elements in the grid based on a predicate, useful for removal.
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(IVec2, &EntityId) -> bool,
    {
        for (&cell, bucket) in self.cells.iter_mut() {
            bucket.retain(|id| f(cell, id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_insertion_and_query() {
        let mut grid = UniformGrid::new(10.0);
        let e1 = EntityId(1);
        let e2 = EntityId(2);

        grid.insert(e1, Vec2::new(5.0, 5.0));
        grid.insert(e2, Vec2::new(15.0, 5.0));

        let cell_0_0: Vec<_> = grid.query_cell(IVec2::new(0, 0)).copied().collect();
        assert_eq!(cell_0_0, vec![e1]);

        let cell_1_0: Vec<_> = grid.query_cell(IVec2::new(1, 0)).copied().collect();
        assert_eq!(cell_1_0, vec![e2]);
    }
}
