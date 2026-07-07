//! # Phylon Integration Tests
//!
//! Cross-crate integration test scenarios.
//!
//! This workspace member is built by `cargo test` and exercises interactions
//! between multiple crates. Unit tests live in their respective crates;
//! integration tests that require multiple crates belong here.
//!
//! ## Phase 0 tests
//!
//! - Scheduler advances ticks correctly.
//! - Config loads defaults without errors.
//! - Event bus publish/drain round-trip.

#![warn(missing_docs)]
#![warn(clippy::all)]

#[cfg(test)]
mod integration {
    use common::Tick;
    use config::PhylonConfig;
    use events::PhylonEvent;
    use scheduler::{SimulationScheduler, SystemOrder};

    #[test]
    fn scheduler_integrates_with_event_bus() {
        let cfg = PhylonConfig::default();
        let mut sched = SimulationScheduler::new(&cfg);

        // Register a system that publishes an event
        sched.register(
            SystemOrder::PostTick,
            Box::new(|tick, bus| {
                bus.publish(PhylonEvent::ExperimentCheckpoint {
                    tick,
                    label: "integration-test".into(),
                })
                .map_err(|e| e.to_string())
            }),
        );

        sched.step().expect("step must succeed");

        let events = sched.event_bus().drain();
        assert_eq!(events.len(), 1, "expected exactly one event");
        assert!(
            matches!(events[0], PhylonEvent::ExperimentCheckpoint { tick, .. } if tick == Tick(0))
        );
    }

    #[test]
    fn ten_tick_sequence_is_monotone() {
        let cfg = PhylonConfig::default();
        let mut sched = SimulationScheduler::new(&cfg);
        for expected in 0u64..10 {
            assert_eq!(sched.current_tick(), Tick(expected));
            sched.step().expect("step must succeed");
        }
        assert_eq!(sched.current_tick(), Tick(10));
    }
}

/// Phase 4, P4-E1: proves `events::PhylonEvent` works as a native
/// `bevy_ecs::event::Events<T>` resource on the simulation `World` — the
/// delivery path the running `app` binary actually uses (see
/// `crates/app/src/app.rs`'s resource registration and
/// `crates/app/src/systems.rs`'s `process_deaths_system`/
/// `interaction_event_log_system`), as opposed to the crossbeam-channel
/// `EventBus` the `scheduler_integrates_with_event_bus` test above exercises.
/// Per `PHASE4_ROADMAP.md`'s own verification plan for this milestone: "An
/// integration test publishing a `PhylonEvent` and confirming it's drained
/// and consumed exactly once — the first such test in the codebase."
#[cfg(test)]
mod phylon_event_ecs_wiring {
    use bevy_ecs::prelude::*;
    use bevy_ecs::system::RunSystemOnce;
    use common::{EntityId, Tick};
    use events::{DeathCause, PhylonEvent};

    #[derive(Resource, Default)]
    struct Consumed(Vec<PhylonEvent>);

    fn consume(mut reader: EventReader<PhylonEvent>, mut consumed: ResMut<Consumed>) {
        for event in reader.read() {
            consumed.0.push(event.clone());
        }
    }

    #[test]
    fn phylon_event_is_published_and_consumed_exactly_once() {
        let mut world = World::new();
        world.insert_resource(Events::<PhylonEvent>::default());
        world.insert_resource(Consumed::default());

        world.send_event(PhylonEvent::OrganismDied {
            id: EntityId(7),
            cause: DeathCause::Predation,
            tick: Tick(42),
        });

        world.run_system_once(consume);

        let consumed = world.resource::<Consumed>();
        assert_eq!(consumed.0.len(), 1, "expected exactly one event consumed");
        assert!(matches!(
            consumed.0[0],
            PhylonEvent::OrganismDied {
                id,
                cause: DeathCause::Predation,
                ..
            } if id == EntityId(7)
        ));
    }
}
