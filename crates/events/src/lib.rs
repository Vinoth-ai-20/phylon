use common::EntityId;
use genetics::Genome;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeathCause {
    Starvation,
    Age,
    Predation,
    Suffocation,
    Disease,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PhylonEvent {
    DeathEvent {
        id: EntityId,
        reason: DeathCause,
    },
    BirthEvent {
        parent: Option<EntityId>,
        genome: Genome,
        initial_energy: f32,
        position: common::Vec2,
    },
    OrganismBorn {
        id: EntityId,
        parent_id: Option<EntityId>,
        generation: u32,
        tick: u64,
    },
    OrganismDied {
        id: EntityId,
        cause: DeathCause,
        tick: u64,
    },
}

use std::any::Any;
use std::sync::{Arc, Mutex};

pub struct EventBus {
    events: Arc<Mutex<Vec<Box<dyn Any + Send + Sync>>>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn publish<T: Any + Send + Sync>(&self, event: T) {
        if let Ok(mut guard) = self.events.lock() {
            guard.push(Box::new(event));
        }
    }

    pub fn drain<T: Any + Send + Sync>(&self) -> Vec<T> {
        let mut result = Vec::new();
        if let Ok(mut guard) = self.events.lock() {
            let mut keep = Vec::new();
            for item in guard.drain(..) {
                match item.downcast::<T>() {
                    Ok(event) => result.push(*event),
                    Err(original_item) => keep.push(original_item),
                }
            }
            *guard = keep;
        }
        result
    }
}
