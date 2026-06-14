//! World state and ECS integration.

pub mod snapshot;

use events::EventBus;
use hecs::World;
use physics::Position;
use spatial::UniformGrid;

/// The canonical simulation world containing the ECS and global indices.
pub struct PhylonWorld {
    pub ecs: World,
    pub spatial_index: UniformGrid,
    pub event_bus: EventBus,
}

impl PhylonWorld {
    pub fn new(chunk_size: f32) -> Self {
        let mut event_bus = EventBus::new();
        event_bus.register::<events::PhylonEvent>();

        Self {
            ecs: World::new(),
            spatial_index: UniformGrid::new(chunk_size),
            event_bus,
        }
    }

    /// Spawns an entity into the ECS. If the entity has a Position component,
    /// it is also inserted into the spatial index.
    pub fn spawn(&mut self, components: impl hecs::DynamicBundle) -> hecs::Entity {
        let entity = self.ecs.spawn(components);

        // If the entity has a position, index it
        if let Ok(pos) = self.ecs.get::<&Position>(entity) {
            self.spatial_index
                .insert(common::EntityId(entity.to_bits().get()), pos.0);
        }

        entity
    }

    /// Synchronises the spatial index with the ECS.
    /// This should be called after physics steps that move entities.
    pub fn update_spatial_index(&mut self) {
        puffin::profile_function!();
        self.spatial_index.clear();

        for (entity, pos) in self.ecs.query_mut::<&Position>() {
            self.spatial_index
                .insert(common::EntityId(entity.to_bits().get()), pos.0);
        }
    }
}

impl Default for PhylonWorld {
    fn default() -> Self {
        Self::new(256.0) // Default chunk size
    }
}
