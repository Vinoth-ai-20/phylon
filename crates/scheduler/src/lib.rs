//! # Phylon Scheduler
//!
//! The fixed-tick deterministic scheduler that drives all simulation updates.
//!
//! The scheduler owns the canonical [`Tick`] counter and is responsible for
//! advancing it, invoking registered systems in a deterministic [`SystemOrder`],
//! maintaining the configured tick rate, and measuring per-tick wall-clock time
//! for profiling.
//!
//! ## Design principles
//!
//! - **Determinism**: Systems execute in a fixed, canonical order defined by
//!   [`SystemOrder`]. This ordering is recorded in `docs/04_simulation_model.md`
//!   and must not change between runs for experiment reproducibility.
//!
//! - **Fixed timestep**: The scheduler tracks the accumulated real-time delta
//!   and fires simulation ticks at a fixed `1 / tick_rate` interval. Any excess
//!   wall-clock time is carried forward as an accumulator, preventing ticks
//!   from drifting due to processing jitter.
//!
//! - **No blocking I/O**: The scheduler runs on the main rayon thread pool.
//!   It must never perform blocking I/O directly — that is the responsibility
//!   of the `storage` and `network` crates via tokio channels.
//!
//! ## Implementation scope
//!
//! The scheduler operates as a self-contained tick counter with timing and
//! boxed-closure system dispatch (see [`SystemFn`]). It does not use
//! `bevy_ecs` system graphs internally — callers register plain closures
//! against a [`SystemOrder`] phase instead.
//!
//! ## Current status: not wired into the live app
//!
//! **This crate's [`SimulationScheduler`] is not used by the running `app`
//! binary.** `app::simulation::update_simulation` drives every real
//! simulation tick directly against the `bevy_ecs::World`, without going
//! through this crate. This crate is retained deliberately, not as dead
//! weight, for two real, still-exercised consumers: `benchmarks`'
//! `scheduler_throughput` criterion benchmark, and `tests`'
//! `scheduler_integrates_with_event_bus` integration test — both
//! measure/exercise this scheduler's own tick-accumulator and event-bus
//! integration in isolation, independent of whether the live app uses it.
//! `research` has no dependency on this crate. If a future need arises for
//! a real `bevy_ecs`-system-graph scheduler as the live app's driver, this
//! crate's design principles above remain a valid starting point, but that
//! would be a deliberate design decision to make at that time, not something
//! implied by this crate's continued existence.

#![warn(missing_docs)]
#![warn(clippy::all)]

use std::time::{Duration, Instant};

use thiserror::Error;
use tracing::{debug, instrument, span, warn, Level};

use common::Tick;
use config::PhylonConfig;
use events::EventBus;

// ────────────────────────────────────────────────────────────────────────────
// SystemOrder
// ────────────────────────────────────────────────────────────────────────────

/// # Deterministic System Execution Order
///
/// ## 1. What Happens
/// `SystemOrder` defines the strict, canonical enumeration of the phases in a single
/// simulation tick.
///
/// ## 2. Why It Happens
/// In complex ECS-driven ALife engines, execution order bugs ("System A reads X before
/// System B writes it") are the leading cause of non-determinism. By forcing all systems
/// into a rigid pipeline, a simulation run with RNG seed 42 will always produce the exact
/// same ecology 1,000,000 ticks later across different machines.
///
/// ## 3. How It Happens
/// Systems registered to the `SimulationScheduler` are sorted by this enum. During `run_tick`,
/// the scheduler loops over the sorted vector and invokes the boxed closures sequentially.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SystemOrder {
    /// Spatial index updates, broad-phase collision preparation.
    PrePhysics = 0,
    /// Force integration, collision response, constraint solving.
    Physics = 1,
    /// Diffusion field update step (one sub-step per tick).
    Diffusion = 2,
    /// Sensory field sampling for all organisms.
    Sensing = 3,
    /// Neural network forward pass.
    Brain = 4,
    /// Action selection and locomotion output.
    Behavior = 5,
    /// Energy consumption, hunger, ageing.
    Metabolism = 6,
    /// Food web interactions, predation, decomposition.
    Ecology = 7,
    /// Reproduction checks and offspring spawning.
    Reproduction = 8,
    /// Event drain, world state finalization, analytics snapshot.
    PostTick = 9,
    /// Metric recording (runs after `PostTick` so it sees the finalized state).
    Analytics = 10,
}

impl SystemOrder {
    /// Returns all system order variants in their canonical execution order.
    pub fn all_ordered() -> &'static [SystemOrder] {
        &[
            SystemOrder::PrePhysics,
            SystemOrder::Physics,
            SystemOrder::Diffusion,
            SystemOrder::Sensing,
            SystemOrder::Brain,
            SystemOrder::Behavior,
            SystemOrder::Metabolism,
            SystemOrder::Ecology,
            SystemOrder::Reproduction,
            SystemOrder::PostTick,
            SystemOrder::Analytics,
        ]
    }
}

impl std::fmt::Display for SystemOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            SystemOrder::PrePhysics => "PrePhysics",
            SystemOrder::Physics => "Physics",
            SystemOrder::Diffusion => "Diffusion",
            SystemOrder::Sensing => "Sensing",
            SystemOrder::Brain => "Brain",
            SystemOrder::Behavior => "Behavior",
            SystemOrder::Metabolism => "Metabolism",
            SystemOrder::Ecology => "Ecology",
            SystemOrder::Reproduction => "Reproduction",
            SystemOrder::PostTick => "PostTick",
            SystemOrder::Analytics => "Analytics",
        };
        write!(f, "{name}")
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Scheduler error
// ────────────────────────────────────────────────────────────────────────────

/// Errors produced by the scheduler.
#[derive(Debug, Error)]
pub enum SchedulerError {
    /// A registered system callback returned an error during execution.
    #[error("system '{phase}' at tick {tick} returned an error: {message}")]
    SystemError {
        /// The phase in which the error occurred.
        phase: SystemOrder,
        /// The tick at which the error occurred.
        tick: Tick,
        /// Human-readable error description.
        message: String,
    },
}

impl common::PhylonError for SchedulerError {}

// ────────────────────────────────────────────────────────────────────────────
// Per-tick statistics
// ────────────────────────────────────────────────────────────────────────────

/// Wall-clock timing statistics for a single simulation tick.
///
/// Recorded by the scheduler and made available to the `analytics` crate
/// and the `puffin` profiler integration.
#[derive(Debug, Clone)]
pub struct TickStats {
    /// The tick this measurement covers.
    pub tick: Tick,
    /// Total wall-clock time spent processing this tick.
    pub total_duration: Duration,
    /// Wall-clock time spent in each phase (parallel to [`SystemOrder::all_ordered`]).
    pub phase_durations: Vec<(SystemOrder, Duration)>,
}

// ────────────────────────────────────────────────────────────────────────────
// SimulationScheduler
// ────────────────────────────────────────────────────────────────────────────

/// Type alias for a registered system callback.
///
/// Systems are registered as boxed closures rather than `bevy_ecs` systems,
/// keeping this crate's dispatch mechanism independent of the ECS. The
/// closure receives:
/// - The current [`Tick`] value.
/// - A shared reference to the [`EventBus`] so systems can publish events.
///
/// The return value is `Ok(())` on success or a string error message. The
/// scheduler converts the string into a [`SchedulerError::SystemError`].
pub type SystemFn = Box<dyn FnMut(Tick, &EventBus) -> Result<(), String> + Send>;

/// # Deterministic Tick Scheduler
///
/// ## 1. What Happens
/// The `SimulationScheduler` is the beating heart of the Phylon engine. It manages the `Tick`
/// counter, accumulates wall-clock time, and dispatches systems in canonical order.
///
/// ## 2. Why It Happens
/// Real-time rendering framerates fluctuate (e.g., $144Hz$ vs $60Hz$). If physics or metabolism
/// were tied to the frame delta, the simulation would become non-deterministic. A fixed timestep
/// accumulator decouples simulation updates from rendering.
///
/// ## 3. How It Happens
/// The `app` event loop calls `advance(max_ticks_per_frame)` passing the elapsed frame time.
/// 1. The time is added to `accumulator`.
/// 2. While `accumulator >= tick_duration`, it calls `run_tick()` and subtracts `tick_duration`.
/// 3. `run_tick()` iterates through `SystemOrder`, executing all registered closures, then
///    increments `current_tick`.
pub struct SimulationScheduler {
    /// The current tick (starts at zero, incremented by [`SimulationScheduler::advance`]).
    current_tick: Tick,

    /// The fixed duration of one simulation tick (derived from `tick_rate`).
    tick_duration: Duration,

    /// Wall-clock time accumulated since the last tick completed.
    /// Used to implement a fixed-timestep accumulator.
    accumulator: Duration,

    /// Timestamp of the last call to [`SimulationScheduler::advance`] (for accumulator updates).
    last_advance: Instant,

    /// The event bus shared between all systems.
    event_bus: EventBus,

    /// Registered system callbacks, sorted by [`SystemOrder`].
    systems: Vec<(SystemOrder, SystemFn)>,

    /// Statistics for the most recently completed tick.
    last_tick_stats: Option<TickStats>,
}

impl SimulationScheduler {
    /// Creates a new scheduler from a loaded configuration.
    ///
    /// The scheduler starts at [`Tick::ZERO`] with an empty accumulator.
    /// Systems must be registered via [`SimulationScheduler::register`] before calling [`SimulationScheduler::advance`].
    pub fn new(config: &PhylonConfig) -> Self {
        let tick_duration = config.tick_duration();
        let event_bus = EventBus::new(4096);
        Self {
            current_tick: Tick::ZERO,
            tick_duration,
            accumulator: Duration::ZERO,
            last_advance: Instant::now(),
            event_bus,
            systems: Vec::new(),
            last_tick_stats: None,
        }
    }

    /// Registers a system callback at the given execution phase.
    ///
    /// Systems at the same [`SystemOrder`] are invoked in registration order.
    /// This is deterministic as long as registration order is deterministic
    /// (i.e., it always happens in the same sequence at startup).
    pub fn register(&mut self, order: SystemOrder, system: SystemFn) {
        // Insert in sorted position to maintain ordering invariant.
        let pos = self.systems.partition_point(|(o, _)| *o <= order);
        self.systems.insert(pos, (order, system));
    }

    /// Returns the current simulation tick.
    #[inline]
    pub fn current_tick(&self) -> Tick {
        self.current_tick
    }

    /// Returns a shared reference to the event bus.
    #[inline]
    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    /// Returns timing statistics from the most recently completed tick,
    /// or `None` if no tick has completed yet.
    #[inline]
    pub fn last_tick_stats(&self) -> Option<&TickStats> {
        self.last_tick_stats.as_ref()
    }

    /// Updates the internal accumulator and fires simulation ticks.
    ///
    /// Call this once per frame from the `winit` event loop. The scheduler
    /// will fire **zero or more** simulation ticks depending on how much
    /// real time has elapsed since the last call. Returns timing stats for
    /// every tick that fired during this call.
    ///
    /// Ticks are capped at `max_ticks_per_frame` to prevent a "spiral of
    /// death" when rendering is slow.
    ///
    /// # Errors
    ///
    /// Returns the first [`SchedulerError`] encountered during system
    /// execution. Subsequent ticks in the same frame are **not** executed
    /// after an error — the simulation must be considered dirty.
    #[instrument(skip(self), fields(tick = self.current_tick.0))]
    pub fn advance(&mut self, max_ticks_per_frame: u32) -> Result<Vec<TickStats>, SchedulerError> {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_advance);
        self.last_advance = now;
        self.accumulator += elapsed;

        let mut stats_this_frame = Vec::new();
        let mut ticks_fired = 0u32;

        while self.accumulator >= self.tick_duration && ticks_fired < max_ticks_per_frame {
            self.accumulator -= self.tick_duration;
            let stats = self.run_tick()?;
            stats_this_frame.push(stats);
            ticks_fired += 1;
        }

        if ticks_fired == max_ticks_per_frame && self.accumulator >= self.tick_duration {
            warn!(
                tick = self.current_tick.0,
                "Simulation is running behind: accumulator = {:?}, capping at {} ticks/frame",
                self.accumulator,
                max_ticks_per_frame
            );
        }

        Ok(stats_this_frame)
    }

    /// Executes a single simulation tick, invoking all registered systems
    /// in canonical [`SystemOrder`] and recording per-phase timings.
    fn run_tick(&mut self) -> Result<TickStats, SchedulerError> {
        let tick = self.current_tick;
        let tick_start = Instant::now();

        let _span = span!(Level::DEBUG, "tick", tick = tick.0).entered();
        debug!("tick start");

        let mut phase_durations = Vec::with_capacity(self.systems.len());

        for (order, system) in &mut self.systems {
            let phase_start = Instant::now();
            let _phase_span = span!(Level::DEBUG, "phase", phase = %order).entered();

            system(tick, &self.event_bus).map_err(|message| SchedulerError::SystemError {
                phase: *order,
                tick,
                message,
            })?;

            phase_durations.push((*order, phase_start.elapsed()));
        }

        // Advance the tick counter after all systems have run.
        self.current_tick = tick.next();

        let total_duration = tick_start.elapsed();
        debug!(
            tick = tick.0,
            total_ms = total_duration.as_secs_f32() * 1000.0,
            "tick complete"
        );

        let stats = TickStats {
            tick,
            total_duration,
            phase_durations,
        };
        self.last_tick_stats = Some(stats.clone());
        Ok(stats)
    }

    /// Advances the scheduler by exactly one tick, regardless of wall-clock time.
    ///
    /// This is a deterministic step function intended for headless research
    /// mode and tests where real-time pacing is not desired.
    ///
    /// # Errors
    ///
    /// Returns a [`SchedulerError`] if any system fails during the tick.
    pub fn step(&mut self) -> Result<TickStats, SchedulerError> {
        self.run_tick()
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_scheduler() -> SimulationScheduler {
        SimulationScheduler::new(&config::PhylonConfig::default())
    }

    #[test]
    fn scheduler_starts_at_zero() {
        let sched = make_scheduler();
        assert_eq!(sched.current_tick(), Tick::ZERO);
    }

    #[test]
    fn step_advances_tick() {
        let mut sched = make_scheduler();
        sched.step().expect("step should succeed");
        assert_eq!(sched.current_tick(), Tick(1));
    }

    #[test]
    fn system_order_is_sorted() {
        let all = SystemOrder::all_ordered();
        let mut prev = all[0];
        for &next in &all[1..] {
            assert!(prev < next, "{prev} must come before {next}");
            prev = next;
        }
    }

    #[test]
    fn register_and_execute_system() {
        let mut sched = make_scheduler();
        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called_clone = called.clone();
        sched.register(
            SystemOrder::PostTick,
            Box::new(move |_tick, _bus| {
                called_clone.store(true, std::sync::atomic::Ordering::Relaxed);
                Ok(())
            }),
        );
        sched.step().expect("step should succeed");
        assert!(called.load(std::sync::atomic::Ordering::Relaxed));
    }

    #[test]
    fn failing_system_returns_error() {
        let mut sched = make_scheduler();
        sched.register(
            SystemOrder::Physics,
            Box::new(|_tick, _bus| Err("deliberate test error".into())),
        );
        let result = sched.step();
        assert!(result.is_err());
    }

    #[test]
    fn tick_stats_recorded() {
        let mut sched = make_scheduler();
        assert!(sched.last_tick_stats().is_none());
        sched.step().expect("step should succeed");
        assert!(sched.last_tick_stats().is_some());
        assert_eq!(sched.last_tick_stats().unwrap().tick, Tick(0));
    }
}
