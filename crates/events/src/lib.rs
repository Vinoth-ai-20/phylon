//! Typed event bus for cross-domain communication in Phylon.

use common::{ChunkId, EntityId, Tick};
use crossbeam::channel::{unbounded, Receiver, Sender};
use std::any::{Any, TypeId};
use std::collections::HashMap;

/// A generic placeholder for field types pending full implementation in `diffusion`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldType {
    Resource,
    Toxin,
    Heat,
}

/// A generic placeholder for death causes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeathCause {
    Starvation,
    OldAge,
    Predation,
    Disease,
}

/// The core events of the simulation.
#[derive(Debug, Clone)]
pub enum PhylonEvent {
    OrganismBorn { id: EntityId, tick: Tick },
    OrganismDied { id: EntityId, cause: DeathCause, tick: Tick },
    ReproductionEvent { parent: EntityId, child: EntityId, tick: Tick },
    FieldSpike { chunk: ChunkId, field: FieldType, value: f32, tick: Tick },
    ExperimentCheckpoint { tick: Tick, label: String },
}

/// A type-erased event bus that allows publishing and draining strongly-typed events.
pub struct EventBus {
    senders: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    receivers: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            senders: HashMap::new(),
            receivers: HashMap::new(),
        }
    }

    /// Register a new event type on the bus.
    pub fn register<E: 'static + Send + Sync + Clone>(&mut self) {
        let tid = TypeId::of::<E>();
        if !self.senders.contains_key(&tid) {
            let (tx, rx) = unbounded::<E>();
            self.senders.insert(tid, Box::new(tx));
            self.receivers.insert(tid, Box::new(rx));
        }
    }

    /// Publish an event to the bus.
    pub fn publish<E: 'static + Send + Sync + Clone>(&self, event: E) {
        let tid = TypeId::of::<E>();
        if let Some(tx_any) = self.senders.get(&tid) {
            if let Some(tx) = tx_any.downcast_ref::<Sender<E>>() {
                let _ = tx.send(event);
            }
        }
    }

    /// Drain all queued events of type `E`.
    pub fn drain<E: 'static + Send + Sync + Clone>(&self) -> Vec<E> {
        let tid = TypeId::of::<E>();
        if let Some(rx_any) = self.receivers.get(&tid) {
            if let Some(rx) = rx_any.downcast_ref::<Receiver<E>>() {
                return rx.try_iter().collect();
            }
        }
        Vec::new()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_bus() {
        let mut bus = EventBus::new();
        bus.register::<PhylonEvent>();
        
        bus.publish(PhylonEvent::ExperimentCheckpoint { 
            tick: Tick(1), 
            label: "Start".to_string() 
        });

        let events: Vec<PhylonEvent> = bus.drain();
        assert_eq!(events.len(), 1);
        if let PhylonEvent::ExperimentCheckpoint { tick, label } = &events[0] {
            assert_eq!(*tick, Tick(1));
            assert_eq!(label, "Start");
        } else {
            panic!("Wrong event type");
        }
    }
}
