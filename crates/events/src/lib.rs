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
//! ## Current wiring caveat (corrected, Phase 7 W0f — see
//! `PHASE7_WORKBENCH_ROADMAP.md`'s event-architecture audit)
//!
//! `scheduler::SimulationScheduler` does own an [`EventBus`] instance as
//! described above, but the live `app` binary's actual per-tick driver
//! (`PhylonApp::update_simulation`) calls simulation systems directly via
//! `bevy_ecs::system::RunSystemOnce` rather than ticking through
//! `SimulationScheduler` — so [`EventBus`] itself is genuinely unused by
//! the running application. **[`PhylonEvent`] itself is not dead**,
//! though, contrary to what this paragraph used to claim: `OrganismBorn`,
//! `OrganismDied`, and `ReproductionEvent` are published via bevy_ecs's
//! own native `Event`/`EventWriter`/`EventReader` machinery (a second,
//! simpler delivery path alongside [`EventBus`], described above) and
//! consumed today by `crates/app/src/systems.rs`'s
//! `interaction_event_log_system`. Only [`ExperimentCheckpoint`](PhylonEvent::ExperimentCheckpoint)
//! remains defined but never actually published or consumed by anything —
//! a real, disclosed gap, not a description of every variant.
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

use common::{EntityId, Tick};

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

// ────────────────────────────────────────────────────────────────────────────
// PhylonEvent enum
// ────────────────────────────────────────────────────────────────────────────

/// # Phylon Global Observable Events
///
/// ## 1. What Happens
/// `PhylonEvent` is an immutable, cloneable value type representing a state change or milestone
/// that has definitively occurred in the simulation (e.g., Birth, Death, Spikes).
///
/// ## 2. Why It Happens
/// For post-simulation analytics and machine-learning fitness evaluations, the engine needs an
/// audit trail of what happened, not just the final state. Since ECS data is mutated in-place
/// (and dead organisms are despawned and lost), events serve as the historical ledger.
///
/// ## 3. How It Happens
/// When a state-mutating system finishes its logic, it constructs a variant of this enum,
/// attaching the exact temporal frame (Tick $t$). Handlers reading this enum are guaranteed
/// to see the causal timeline:
///
/// $$ T_{birth} \le t \le T_{death} $$
///
/// **Phase 4, P4-F-adjacent milestone P4-E1:** also derives `bevy_ecs::event::Event`,
/// so it can be published/consumed as a native `bevy_ecs::event::Events<PhylonEvent>`
/// resource directly on the simulation `World` — the same pattern already
/// established by `reproduction::BirthRequest`/`ecology::catastrophe::HazardSpawned`
/// (see `crates/app/src/app.rs`'s resource registration and
/// `crates/app/src/simulation.rs`'s per-tick `Events::update()` calls). This
/// is deliberately a *second*, simpler delivery path alongside [`EventBus`]
/// (which remains for `SimulationScheduler`-based, cross-thread use) rather
/// than a replacement — the running `app` binary drives systems directly via
/// `RunSystemOnce`, bypassing `SimulationScheduler` entirely, so a bevy-native
/// `Events<T>` resource is what the live app can actually wire up today.
#[derive(Debug, Clone, Serialize, Deserialize, bevy_ecs::prelude::Event)]
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
    ///
    /// Phase 5, SX-3a: `generation`/`lineage` were added so a real consumer
    /// (`crates/app/src/systems.rs`'s `interaction_event_log_system`) could
    /// log lineage milestones *from this event* instead of a parallel,
    /// never-actually-reading-this-event code path in
    /// `SpawnOrganismCommand::apply` that duplicated the same "every 5th
    /// generation" logic directly against `NarrationLog`.
    ReproductionEvent {
        /// The parent organism's ID.
        parent: EntityId,
        /// The new child organism's ID.
        child: EntityId,
        /// The tick on which reproduction completed.
        tick: Tick,
        /// The child's generation number (0 for an initial seed organism
        /// with no parent — though such organisms don't publish this event
        /// at all, see `SpawnOrganismCommand::apply`).
        generation: u32,
        /// The lineage this reproduction belongs to (`evolution::LineageId`'s
        /// inner value — `events` doesn't depend on `evolution`, so this is
        /// the raw `u64`, not the newtype).
        lineage: u64,
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

/// # The Central Typed Event Bus
///
/// ## 1. What Happens
/// The `EventBus` is an MPMC (Multi-Producer, Multi-Consumer) channel wrapper using `crossbeam::channel`.
/// It acts as the backbone for cross-subsystem communication without requiring hard dependencies.
///
/// ## 2. Why It Happens
/// In complex simulations (especially ones spanning CPU ECS and GPU pipelines), tight coupling
/// between systems causes deadlocks and borrow-checker conflicts. By using a deferred event bus,
/// an organism can "die" in the physics step, and the analytics/UI system can process that death
/// at the end of the tick, perfectly preserving temporal order $O(N)$ without locking ECS resources.
///
/// ## 3. How It Happens
/// A bounded channel of size `C` is created. Producers push $E$ events into the channel lock-free.
/// If $E > C$, the bus drops the event to prevent out-of-memory cascading failures:
///
/// $$ \text{Result} = \begin{cases} \text{Ok}(), & \text{if } |Q| < C \\ \text{Err}(\text{ChannelFull}), & \text{if } |Q| \ge C \end{cases} $$
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
// Timed Effects (Phase 4, P4-E1)
// ────────────────────────────────────────────────────────────────────────────

/// One kind of transient, position-anchored visual effect. Deliberately a
/// single variant for now — this milestone (P4-E1) is the shared
/// infrastructure, not the effect catalog; individual visual effects
/// (predation flash, blood-flow particle trail, hormone diffusion glow) are
/// Epic 8's job (`PHASE4_ROADMAP.md`'s P4-V1/P4-V2), gated behind this one
/// per ADR-P4-05.
#[derive(Debug, Clone, PartialEq)]
pub enum TimedEffectKind {
    /// A short line of text that appears at a world position and expires.
    FloatingText {
        /// The text to display.
        text: String,
        /// RGB color, `[0, 1]` per channel.
        color: [f32; 3],
    },
}

/// One active transient visual effect, anchored to a world position and a
/// tick-based expiry.
#[derive(Debug, Clone, PartialEq)]
pub struct TimedEffect {
    /// World-space position this effect is anchored to.
    pub position: common::Vec2,
    /// What kind of effect this is.
    pub kind: TimedEffectKind,
    /// The tick after which this effect is no longer active.
    pub expires_at_tick: u64,
}

/// # Timed Effects
///
/// ## 1. What Happens
/// Holds every currently-active transient visual effect (Phase 4, P4-E1) —
/// e.g. a "such-and-such happened" floating text — each with a world
/// position and a tick-based expiry.
///
/// ## 2. Why It Happens
/// Before this milestone, every visual in the running app was steady-state,
/// recomputed fresh every frame from current ECS state (diet rings, hover
/// highlight, a selection pulse driven by `total_sim_time`) — there was no
/// way to represent "this happened, briefly show something about it, then
/// stop," which every future interaction VFX (Epic 8) and physiology
/// visualization (blood flow, hormone diffusion) needs. This resource, plus
/// [`expire`](Self::expire), is that missing primitive — modeled on the
/// existing `WorkbenchState::push_toast`/`cleanup_toasts` pattern already
/// used for UI notifications (`crates/ui/src/state.rs`), but anchored to a
/// world position and a simulation tick instead of a screen corner and
/// wall-clock time.
///
/// ## 3. How It Happens
/// [`spawn`](Self::spawn) appends a new effect with `expires_at_tick =
/// current_tick + duration_ticks`; [`expire`](Self::expire), called once per
/// tick, retains only effects whose `expires_at_tick` is still in the
/// future. **Rendering is explicitly out of scope for this milestone** — per
/// ADR-P4-05, Epic 6 (this milestone) is the shared data-side
/// infrastructure; drawing these onto the viewport is Epic 8's job, once a
/// real visual effect is designed to consume them.
#[derive(Debug, Clone, Default, bevy_ecs::prelude::Resource)]
pub struct TimedEffects {
    /// Every effect currently active, in spawn order.
    pub active: Vec<TimedEffect>,
}

impl TimedEffects {
    /// Adds a new effect that will remain active through
    /// `current_tick + duration_ticks`, inclusive.
    pub fn spawn(
        &mut self,
        position: common::Vec2,
        kind: TimedEffectKind,
        current_tick: u64,
        duration_ticks: u64,
    ) {
        self.active.push(TimedEffect {
            position,
            kind,
            expires_at_tick: current_tick + duration_ticks,
        });
    }

    /// Drops every effect whose expiry has passed as of `current_tick`.
    pub fn expire(&mut self, current_tick: u64) {
        self.active.retain(|e| e.expires_at_tick > current_tick);
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

    #[test]
    fn timed_effects_spawn_is_active_immediately() {
        let mut effects = TimedEffects::default();
        effects.spawn(
            common::Vec2::new(1.0, 2.0),
            TimedEffectKind::FloatingText {
                text: "Eaten!".to_string(),
                color: [1.0, 0.0, 0.0],
            },
            100,
            30,
        );
        assert_eq!(effects.active.len(), 1);
        assert_eq!(effects.active[0].expires_at_tick, 130);
    }

    #[test]
    fn timed_effects_expire_removes_only_past_expiry() {
        let mut effects = TimedEffects::default();
        effects.spawn(
            common::Vec2::ZERO,
            TimedEffectKind::FloatingText {
                text: "short".to_string(),
                color: [1.0, 1.0, 1.0],
            },
            0,
            10,
        );
        effects.spawn(
            common::Vec2::ZERO,
            TimedEffectKind::FloatingText {
                text: "long".to_string(),
                color: [1.0, 1.0, 1.0],
            },
            0,
            1000,
        );

        effects.expire(11);

        assert_eq!(effects.active.len(), 1);
        assert_eq!(
            effects.active[0].kind,
            TimedEffectKind::FloatingText {
                text: "long".to_string(),
                color: [1.0, 1.0, 1.0],
            }
        );
    }

    #[test]
    fn timed_effects_expire_at_exact_tick_removes_it() {
        // `expires_at_tick` is the last tick an effect is still active;
        // `current_tick == expires_at_tick` should already be expired.
        let mut effects = TimedEffects::default();
        effects.spawn(
            common::Vec2::ZERO,
            TimedEffectKind::FloatingText {
                text: "x".to_string(),
                color: [0.0, 0.0, 0.0],
            },
            0,
            10,
        );
        effects.expire(10);
        assert!(effects.active.is_empty());
    }
}
