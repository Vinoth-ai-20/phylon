//! World state and ECS integration.

use events::EventBus;
use hecs::World;
use physics::Position;
use spatial::UniformGrid;

/// The canonical simulation world containing the ECS and global indices.
pub struct PhylonWorld {
    pub ecs: World,
    pub spatial_index: UniformGrid,
    pub event_bus: EventBus,
    pub field_grid: Vec<[f32; 4]>,
    pub last_events: Vec<events::PhylonEvent>,
    pub species_registry: evolution::SpeciesRegistry,
    pub grid_width: u32,
    pub grid_height: u32,
}

impl PhylonWorld {
    pub fn new(chunk_size: f32) -> Self {
        let event_bus = EventBus::new();

        Self {
            ecs: World::new(),
            spatial_index: UniformGrid::new(chunk_size),
            event_bus,
            field_grid: vec![[0.0; 4]; 256 * 256], // 256x256 grid
            last_events: Vec::new(),
            species_registry: evolution::SpeciesRegistry::new(),
            grid_width: 256,
            grid_height: 256,
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

pub struct GridSampler<'a> {
    pub grid: &'a [[f32; 4]],
    pub width: u32,
    pub height: u32,
}

impl<'a> sensing::FieldSampler for GridSampler<'a> {
    fn sample(&self, pos: common::Vec2) -> [f32; 4] {
        let half_w = self.width as f32 / 2.0;
        let half_h = self.height as f32 / 2.0;
        let gx = (pos.x + half_w).floor() as i32;
        let gy = (pos.y + half_h).floor() as i32;
        if gx >= 0 && gx < self.width as i32 && gy >= 0 && gy < self.height as i32 {
            let idx = (gy as u32 * self.width + gx as u32) as usize;
            if idx < self.grid.len() {
                return self.grid[idx];
            }
        }
        [0.0; 4]
    }
}
