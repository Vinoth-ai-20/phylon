use crate::components::{Corpse, FoodPellet, MineralPellet, ResourceSpatialGrids};
use bevy_ecs::prelude::*;

/// Rebuilds [`ResourceSpatialGrids`] from this tick's pellet positions. Must
/// run before both `sensing::sensing_system` and
/// `crate::systems::foraging::foraging_system`.
pub fn build_resource_grids_system(
    mut grids: ResMut<ResourceSpatialGrids>,
    food_query: Query<(Entity, &FoodPellet)>,
    mineral_query: Query<(Entity, &MineralPellet)>,
    corpse_query: Query<(Entity, &Corpse)>,
) {
    grids.food.clear();
    for (entity, food) in food_query.iter() {
        let _ = grids.food.insert(entity, food.position);
    }
    grids.minerals.clear();
    for (entity, mineral) in mineral_query.iter() {
        let _ = grids.minerals.insert(entity, mineral.position);
    }
    grids.corpses.clear();
    for (entity, corpse) in corpse_query.iter() {
        let _ = grids.corpses.insert(entity, corpse.position);
    }
}
