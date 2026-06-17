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
