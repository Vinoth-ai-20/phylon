//! # Phylon Events
//!
//! The typed event bus for cross-crate simulation event dispatch.
//!
//! Events are the primary communication mechanism between decoupled simulation
//! subsystems. Systems **publish** events during their update phase; the
//! `scheduler` drains and dispatches events at `PostTick`; output systems
//! (analytics, storage, UI) consume events without modifying the simulation
//! world.
//!
//! ## Design
//!
//! - [`EventBus`] holds one [`crossbeam::channel`] pair per event variant.
//! - Events are cloneable value types — no heap allocations for consumers.
//! - The bus is `Send + Sync` and can be shared across rayon threads behind
//!   an `Arc`.
//!
//! ## Usage
//!
//! ```rust
//! use events::{EventBus, PhylonEvent};
//! use common::{EntityId, Tick};
//!
//! let bus = EventBus::new(256);
//! bus.publish(PhylonEvent::OrganismBorn { id: EntityId(1), tick: Tick(0) });
//! let drained: Vec<PhylonEvent> = bus.drain();
//! assert_eq!(drained.len(), 1);
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

use crossbeam::channel::{self, Receiver, Sender, TrySendError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use common::{ChunkId, EntityId, Tick};

// ────────────────────────────────────────────────────────────────────────────
// Supporting enums
// ────────────────────────────────────────────────────────────────────────────

/// The cause of an organism's death.
///
/// Stored in [`PhylonEvent::OrganismDied`] for analytics and post-mortem
/// research queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeathCause {
    /// The organism ran out of energy.
    Starvation,
    /// The organism was killed by a predator.
    Predation,
    /// The organism died from a disease.
    Disease,
    /// The organism reached its maximum age.
    Senescence,
    /// The organism was killed by a research tool intervention.
    GodMode,
    /// The organism died from an injury (physical damage).
    Injury,
    /// The organism died from environmental exposure (temperature, toxin, etc.).
    Environment,
    /// An unknown or unclassified cause.
    Unknown,
}

/// Identifies which diffusion field triggered a [`PhylonEvent::FieldSpike`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FieldType {
    /// Oxygen concentration field.
    Oxygen,
    /// Carbon dioxide concentration field.
    CarbonDioxide,
    /// Food / nutrient availability field.
    Nutrient,
    /// Pheromone signal field.
    Pheromone,
    /// Thermal energy field.
    Temperature,
    /// Toxin concentration field.
    Toxin,
    /// Disease / pathogen concentration field.
    Disease,
    /// Bioluminescence intensity field.
    Bioluminescence,
    /// Sound pressure field (used by directional hearing sensor).
    SoundPressure,
}

// ────────────────────────────────────────────────────────────────────────────
// PhylonEvent enum
// ────────────────────────────────────────────────────────────────────────────

/// All observable events that can occur during a simulation tick.
///
/// Events are immutable value types dispatched through [`EventBus`]. Handlers
/// receive a snapshot of the event state at the moment it was published;
/// later mutations to simulation entities do not retroactively change events.
///
/// New variants should be added here as later phases introduce new subsystems.
/// Every variant must include a `tick` field so events can be ordered and
/// replayed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhylonEvent {
    /// A new organism has been born (either initial spawn or reproduction).
    OrganismBorn {
        /// The newly created entity's ID.
        id: EntityId,
        /// The tick on which the birth occurred.
        tick: Tick,
    },

    /// An organism has died and will be removed from the ECS world.
    OrganismDied {
        /// The entity that died.
        id: EntityId,
        /// Why the organism died.
        cause: DeathCause,
        /// The tick on which death was recorded.
        tick: Tick,
    },

    /// An organism has reproduced, creating a child entity.
    ReproductionEvent {
        /// The parent organism's ID.
        parent: EntityId,
        /// The new child organism's ID.
        child: EntityId,
        /// The tick on which reproduction completed.
        tick: Tick,
    },

    /// A diffusion field value in a chunk has exceeded a significant threshold.
    ///
    /// Used by analytics to record environmental spikes and by the ecology
    /// system to trigger reactive responses.
    FieldSpike {
        /// The chunk where the spike occurred.
        chunk: ChunkId,
        /// The field type that spiked.
        field: FieldType,
        /// The peak value recorded.
        value: f32,
        /// The tick on which the spike was detected.
        tick: Tick,
    },

    /// A researcher-defined checkpoint in the experiment timeline.
    ///
    /// Published by the research crate or via god-mode interventions to mark
    /// significant moments for later analysis.
    ExperimentCheckpoint {
        /// The tick at which the checkpoint was created.
        tick: Tick,
        /// A human-readable label for this checkpoint.
        label: String,
    },
}

impl PhylonEvent {
    /// Returns the tick at which this event was created.
    pub fn tick(&self) -> Tick {
        match self {
            PhylonEvent::OrganismBorn { tick, .. }
            | PhylonEvent::OrganismDied { tick, .. }
            | PhylonEvent::ReproductionEvent { tick, .. }
            | PhylonEvent::FieldSpike { tick, .. }
            | PhylonEvent::ExperimentCheckpoint { tick, .. } => *tick,
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// EventBus error
// ────────────────────────────────────────────────────────────────────────────

/// Errors produced by [`EventBus`] operations.
#[derive(Debug, Error)]
pub enum EventBusError {
    /// The internal channel is full; the event was dropped.
    ///
    /// This indicates the consumer is lagging behind the producer. Increase
    /// the `capacity` passed to [`EventBus::new`] or drain more frequently.
    #[error("event bus channel is full; event dropped (capacity: {capacity})")]
    ChannelFull {
        /// The configured channel capacity.
        capacity: usize,
    },
}

impl common::PhylonError for EventBusError {}

// ────────────────────────────────────────────────────────────────────────────
// EventBus
// ────────────────────────────────────────────────────────────────────────────

/// The central typed event bus.
///
/// Internally backed by a bounded [`crossbeam::channel`] MPMC queue.
/// Multiple producers (simulation threads) can safely call [`EventBus::publish`]
/// concurrently. A single consumer (the scheduler's `PostTick` phase) calls
/// [`EventBus::drain`] to collect all pending events.
///
/// The bus is intentionally kept as a single channel for all event types to
/// preserve inter-event ordering within a tick. If per-type filtering becomes
/// a bottleneck in later phases, the implementation can be upgraded to a
/// per-type channel map without changing the public API.
#[derive(Debug, Clone)]
pub struct EventBus {
    sender: Sender<PhylonEvent>,
    receiver: Receiver<PhylonEvent>,
    capacity: usize,
}

impl EventBus {
    /// Creates a new [`EventBus`] with the given channel capacity.
    ///
    /// `capacity` should be set to a multiple of the expected events-per-tick
    /// to avoid drops during high-activity ticks. A value of `1024` is
    /// sufficient for most Phase 0–3 workloads.
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = channel::bounded(capacity);
        Self {
            sender,
            receiver,
            capacity,
        }
    }

    /// Publishes a single event to the bus.
    ///
    /// This method is lock-free and safe to call from multiple rayon threads
    /// simultaneously.
    ///
    /// # Errors
    ///
    /// Returns [`EventBusError::ChannelFull`] if the internal channel is at
    /// capacity. The caller should log and continue — simulation correctness
    /// must not depend on every event being delivered.
    pub fn publish(&self, event: PhylonEvent) -> Result<(), EventBusError> {
        self.sender.try_send(event).map_err(|e| match e {
            TrySendError::Full(_) => EventBusError::ChannelFull {
                capacity: self.capacity,
            },
            TrySendError::Disconnected(_) => {
                // The receiver has been dropped — this is a programming error,
                // not a runtime condition. Panic with a helpful message.
                panic!("EventBus receiver has been dropped; this is a bug");
            }
        })
    }

    /// Drains all pending events from the bus and returns them in order.
    ///
    /// This method is intended to be called **once per tick** by the scheduler
    /// during the `PostTick` phase. Calling it from multiple threads would
    /// race; by design only the scheduler thread calls `drain`.
    pub fn drain(&self) -> Vec<PhylonEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.receiver.try_recv() {
            events.push(event);
        }
        events
    }

    /// Returns the number of events currently waiting in the queue.
    pub fn pending_count(&self) -> usize {
        self.receiver.len()
    }

    /// Returns the configured channel capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_born(id: u64) -> PhylonEvent {
        PhylonEvent::OrganismBorn {
            id: EntityId(id),
            tick: Tick(0),
        }
    }

    #[test]
    fn publish_and_drain() {
        let bus = EventBus::new(16);
        bus.publish(make_born(1)).unwrap();
        bus.publish(make_born(2)).unwrap();
        let events = bus.drain();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn drain_empty_bus() {
        let bus = EventBus::new(16);
        let events = bus.drain();
        assert!(events.is_empty());
    }

    #[test]
    fn channel_full_returns_error() {
        let bus = EventBus::new(2);
        bus.publish(make_born(1)).unwrap();
        bus.publish(make_born(2)).unwrap();
        // Third publish should fail
        let result = bus.publish(make_born(3));
        assert!(matches!(result, Err(EventBusError::ChannelFull { .. })));
    }

    #[test]
    fn event_tick_accessor() {
        let ev = PhylonEvent::OrganismDied {
            id: EntityId(42),
            cause: DeathCause::Starvation,
            tick: Tick(999),
        };
        assert_eq!(ev.tick(), Tick(999));
    }

    #[test]
    fn pending_count() {
        let bus = EventBus::new(16);
        assert_eq!(bus.pending_count(), 0);
        bus.publish(make_born(1)).unwrap();
        assert_eq!(bus.pending_count(), 1);
        bus.drain();
        assert_eq!(bus.pending_count(), 0);
    }
}
