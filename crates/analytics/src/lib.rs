//! # Phylon Analytics
//!
//! Metrics collection, population history, diversity indices, spatial
//! heatmaps, lineage tracking, and research report generation.
//!
//! The analytics crate is a pure consumer of the event bus — it never
//! mutates simulation state. It accumulates time-series data and exposes
//! query APIs for the UI and research crates.

#![warn(missing_docs)]
#![warn(clippy::all)]

use common::Tick;
use serde::{Deserialize, Serialize};

/// A single population count sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopulationSample {
    /// The tick at which this sample was recorded.
    pub tick: Tick,
    /// Total organism count across all species.
    pub total: u64,
}

/// A single compute pass timing record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassTiming {
    /// Identifier for the compute pass (e.g., "Muscle", "Diffusion").
    pub name: String,
    /// CPU-side estimated duration in milliseconds.
    pub duration_ms: f64,
}

/// Placeholder for the analytics accumulator.
pub struct AnalyticsAccumulator {
    samples: Vec<PopulationSample>,
}

impl AnalyticsAccumulator {
    /// Creates a new empty accumulator.
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
        }
    }

    /// Records a population sample.
    pub fn record_population(&mut self, tick: Tick, total: u64) {
        self.samples.push(PopulationSample { tick, total });
    }

    /// Returns the number of recorded samples.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }
}

impl Default for AnalyticsAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

/// Maximum number of time-series samples to keep in the ring buffers.
pub const METRICS_RING_CAPACITY: usize = 512;

/// A generic history buffer for time-series data.
#[derive(Debug, Clone)]
pub struct HistoryBuffer<T> {
    samples: std::collections::VecDeque<[f64; 2]>,
    capacity: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Into<f64> + Copy> HistoryBuffer<T> {
    /// Create a new history buffer with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: std::collections::VecDeque::with_capacity(capacity),
            capacity,
            _marker: std::marker::PhantomData,
        }
    }

    /// Push a new value at the given time.
    pub fn push(&mut self, time: f64, value: T) {
        if self.samples.len() >= self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back([time, value.into()]);
    }

    /// Get a reference to the internal samples for rendering.
    pub fn samples(&self) -> &std::collections::VecDeque<[f64; 2]> {
        &self.samples
    }

    /// Get the latest sample value.
    pub fn latest(&self) -> Option<f64> {
        self.samples.back().map(|&[_, v]| v)
    }
}

impl<T> std::ops::Deref for HistoryBuffer<T> {
    type Target = std::collections::VecDeque<[f64; 2]>;

    fn deref(&self) -> &Self::Target {
        &self.samples
    }
}

/// Current population counts for various entity types.
#[derive(Debug, Clone, Default)]
pub struct PopulationCounts {
    /// Number of producers
    pub producers: usize,
    /// Number of herbivores
    pub herbivores: usize,
    /// Number of carnivores
    pub carnivores: usize,
    /// Number of omnivores
    pub omnivores: usize,
    /// Number of decomposers
    pub decomposers: usize,
    /// Number of food pellets
    pub food_pellets: usize,
    /// Number of mineral pellets
    pub minerals: usize,
    /// Number of corpses
    pub corpses: usize,
}

/// # Live Simulation Metrics State
///
/// ## 1. What Happens
/// `MetricsState` stores time-series ring buffers of ecological populations, performance metrics
/// (FPS/TPS), and environmental variables (Sunlight, O2, CO2, Temp) for use by the GUI plotters.
///
/// ## 2. Why It Happens
/// A core part of artificial life research is observing macro-level emergent trends (e.g.,
/// predator-prey Lotka-Volterra cycles). The UI needs a sliding window of historical data to
/// render live graphs without causing memory bloat.
///
/// ## 3. How It Happens
/// The `Analytics` system (running at the end of `SystemOrder`) aggregates counts by querying
/// the ECS and pushes `[sim_time_s, value]` pairs onto `VecDeque`s capped at `METRICS_RING_CAPACITY`.
#[derive(bevy_ecs::prelude::Resource, Debug, Clone)]
pub struct MetricsState {
    /// Ring buffer for Producers.
    pub producers_history: HistoryBuffer<f32>,
    /// Ring buffer for Herbivores.
    pub herbivores_history: HistoryBuffer<f32>,
    /// Ring buffer for Carnivores.
    pub carnivores_history: HistoryBuffer<f32>,
    /// Ring buffer for Omnivores.
    pub omnivores_history: HistoryBuffer<f32>,
    /// Ring buffer for Decomposers.
    pub decomposers_history: HistoryBuffer<f32>,
    /// Ring buffer for FoodPellets.
    pub food_history: HistoryBuffer<f32>,
    /// Ring buffer for Minerals.
    pub minerals_history: HistoryBuffer<f32>,
    /// Ring buffer for Corpses.
    pub corpses_history: HistoryBuffer<f32>,
    /// Ring buffer for the FPS plot.
    pub fps_history: HistoryBuffer<f32>,
    /// Ring buffer for TPS (Ticks Per Second).
    pub tps_history: HistoryBuffer<f32>,
    /// Ring buffer for Memory usage (MB).
    pub memory_history: HistoryBuffer<f32>,
    /// Ring buffer for Sunlight.
    pub sunlight_history: HistoryBuffer<f32>,
    /// Ring buffer for O2.
    pub o2_history: HistoryBuffer<f32>,
    /// Ring buffer for CO2.
    pub co2_history: HistoryBuffer<f32>,
    /// Ring buffer for Temperature.
    pub temp_history: HistoryBuffer<f32>,
    /// Accumulated simulation time in seconds.
    pub sim_time: f64,
    /// Smoothed FPS estimate (exponential moving average).
    pub smoothed_fps: f64,
    /// Smoothed TPS estimate.
    pub smoothed_tps: f64,
    /// CPU-side timings for the most recent frame's compute passes.
    pub compute_profiles: Vec<PassTiming>,
}

impl MetricsState {
    /// Creates a new, empty `MetricsState`.
    pub fn new() -> Self {
        Self {
            producers_history: HistoryBuffer::new(METRICS_RING_CAPACITY),
            herbivores_history: HistoryBuffer::new(METRICS_RING_CAPACITY),
            carnivores_history: HistoryBuffer::new(METRICS_RING_CAPACITY),
            omnivores_history: HistoryBuffer::new(METRICS_RING_CAPACITY),
            decomposers_history: HistoryBuffer::new(METRICS_RING_CAPACITY),
            food_history: HistoryBuffer::new(METRICS_RING_CAPACITY),
            minerals_history: HistoryBuffer::new(METRICS_RING_CAPACITY),
            corpses_history: HistoryBuffer::new(METRICS_RING_CAPACITY),
            fps_history: HistoryBuffer::new(METRICS_RING_CAPACITY),
            tps_history: HistoryBuffer::new(METRICS_RING_CAPACITY),
            memory_history: HistoryBuffer::new(METRICS_RING_CAPACITY),
            sunlight_history: HistoryBuffer::new(METRICS_RING_CAPACITY),
            o2_history: HistoryBuffer::new(METRICS_RING_CAPACITY),
            co2_history: HistoryBuffer::new(METRICS_RING_CAPACITY),
            temp_history: HistoryBuffer::new(METRICS_RING_CAPACITY),
            sim_time: 0.0,
            smoothed_fps: 60.0,
            smoothed_tps: 60.0,
            compute_profiles: Vec::new(),
        }
    }

    /// Records one simulation frame with the current population counts and frame delta time.
    ///
    /// `dt` is in seconds (typically 0.016 for 60 Hz).
    pub fn record_frame(&mut self, counts: PopulationCounts, sim_dt: f64, real_dt: f64) {
        self.sim_time += sim_dt;

        // Exponential moving average for FPS (α = 0.05)
        let raw_fps = if real_dt > 0.0 { 1.0 / real_dt } else { 60.0 };
        self.smoothed_fps = self.smoothed_fps * 0.95 + raw_fps * 0.05;

        self.producers_history
            .push(self.sim_time, counts.producers as f32);
        self.herbivores_history
            .push(self.sim_time, counts.herbivores as f32);
        self.carnivores_history
            .push(self.sim_time, counts.carnivores as f32);
        self.omnivores_history
            .push(self.sim_time, counts.omnivores as f32);
        self.decomposers_history
            .push(self.sim_time, counts.decomposers as f32);
        self.food_history
            .push(self.sim_time, counts.food_pellets as f32);
        self.minerals_history
            .push(self.sim_time, counts.minerals as f32);
        self.corpses_history
            .push(self.sim_time, counts.corpses as f32);

        self.fps_history
            .push(self.sim_time, self.smoothed_fps as f32);
    }

    /// Records additional environment and performance metrics.
    pub fn record_env_perf(
        &mut self,
        tps: f64,
        memory_mb: f64,
        sunlight: f64,
        o2: f64,
        co2: f64,
        temp: f64,
    ) {
        self.smoothed_tps = self.smoothed_tps * 0.95 + tps * 0.05;

        self.tps_history
            .push(self.sim_time, self.smoothed_tps as f32);
        self.memory_history.push(self.sim_time, memory_mb as f32);
        self.sunlight_history.push(self.sim_time, sunlight as f32);
        self.o2_history.push(self.sim_time, o2 as f32);
        self.co2_history.push(self.sim_time, co2 as f32);
        self.temp_history.push(self.sim_time, temp as f32);
    }
}

impl Default for MetricsState {
    fn default() -> Self {
        Self::new()
    }
}

/// A narrative event generated by the simulation.
#[derive(Debug, Clone)]
pub struct NarrativeEvent {
    /// Simulation tick when the event occurred.
    pub tick: u64,
    /// Categorical type of event (e.g. "Lineage", "Hazard", "Global").
    pub event_type: String,
    /// Human-readable description.
    pub description: String,
}

/// # Narrative Event Logger
///
/// ## 1. What Happens
/// `NarrationLog` is a ring buffer that records distinct, human-readable milestones that
/// occur during the simulation (e.g., "Species Extinction", "Mutation Discovered").
///
/// ## 2. Why It Happens
/// Raw numbers in a graph are useful, but contextualizing *why* a population graph suddenly
/// dropped is harder. The Event Log acts as a "dungeon master", giving the user a readable
/// chronological history of major ecological events.
///
/// ## 3. How It Happens
/// Various systems (like `reproduction` or `ecology`) detect edge-case conditions and call
/// `push_event`. The UI renders this queue in the bottom panel. Oldest events are evicted
/// once `max_events` is reached to cap memory usage.
#[derive(bevy_ecs::prelude::Resource, Debug, Clone)]
pub struct NarrationLog {
    /// Buffer of recent events.
    pub events: std::collections::VecDeque<NarrativeEvent>,
    max_events: usize,
}

impl NarrationLog {
    /// Create a new narration log with max capacity.
    pub fn new(max_events: usize) -> Self {
        Self {
            events: std::collections::VecDeque::with_capacity(max_events),
            max_events,
        }
    }

    /// Push a new event. Removes oldest if at capacity.
    pub fn push_event(
        &mut self,
        tick: u64,
        event_type: impl Into<String>,
        description: impl Into<String>,
    ) {
        if self.events.len() >= self.max_events {
            self.events.pop_front();
        }
        self.events.push_back(NarrativeEvent {
            tick,
            event_type: event_type.into(),
            description: description.into(),
        });
    }
}

impl Default for NarrationLog {
    fn default() -> Self {
        Self::new(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulator_records_samples() {
        let mut acc = AnalyticsAccumulator::new();
        acc.record_population(Tick(0), 100);
        acc.record_population(Tick(60), 105);
        assert_eq!(acc.sample_count(), 2);
    }

    #[test]
    fn metrics_state_ring_buffer_caps_at_max() {
        let mut m = MetricsState::new();
        for _ in 0..(METRICS_RING_CAPACITY + 10) {
            m.record_frame(PopulationCounts::default(), 0.016, 0.016);
        }
        assert_eq!(m.producers_history.samples().len(), METRICS_RING_CAPACITY);
        assert_eq!(m.fps_history.samples().len(), METRICS_RING_CAPACITY);
    }
}
