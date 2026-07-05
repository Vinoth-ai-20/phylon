//! # Phylon Common
//!
//! Foundational types shared across the entire Phylon workspace.
//!
//! This crate has **zero internal dependencies** — it is the base layer of the
//! dependency graph. Every other crate in the workspace may depend on `common`.
//!
//! ## Contents
//!
//! - **Entity identity**: [`EntityId`], [`ChunkId`], [`Tick`]
//! - **Simulation unit newtypes**: [`SimLength`], [`SimMass`], [`SimEnergy`], [`SimTime`]
//! - **Math re-exports**: [`Vec2`], [`IVec2`]
//! - **Error base**: [`PhylonError`] trait and [`PhylonResult`] type alias
//! - **Determinism**: [`SimRng`], the single seeded source of randomness
//! - **Tick timing**: [`TickRate`], the single source of truth for the fixed per-tick delta-time

#![warn(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::prelude::Resource;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

// ────────────────────────────────────────────────────────────────────────────
// Math re-exports
// ────────────────────────────────────────────────────────────────────────────

/// 2-D floating-point vector — the primary spatial type for all simulation
/// coordinates, velocities, and forces.
pub use glam::Vec2;

/// 2-D integer vector — used for chunk grid coordinates and spatial hash keys.
pub use glam::IVec2;

// ────────────────────────────────────────────────────────────────────────────
// Entity identity
// ────────────────────────────────────────────────────────────────────────────

/// # Phylon Global Entity Identifier
///
/// ## 1. What Happens
/// `EntityId` is a 64-bit unsigned integer acting as the canonical unique identifier
/// for any discrete object in the simulation.
///
/// ## 2. Why It Happens
/// We cannot rely solely on Bevy's internal 32-bit/generation Entity IDs because those
/// are local to the current runtime process and are re-used when entities are despawned.
/// A global `EntityId` ensures that an organism's lineage record remains stable across
/// snapshot save/loads and future distributed network topologies.
///
/// ## 3. How It Happens
/// The `World` crate tracks a monotonic `u64` counter. Every time a new entity is spawned,
/// the counter increments and assigns the new `EntityId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(pub u64);

impl EntityId {
    /// The sentinel "null" entity ID representing the absence of an entity.
    pub const NULL: Self = Self(u64::MAX);

    /// Returns `true` if this ID is the null sentinel.
    #[inline]
    pub fn is_null(self) -> bool {
        self == Self::NULL
    }
}

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Entity({})", self.0)
    }
}

/// # World Partition Chunk
///
/// ## 1. What Happens
/// `ChunkId` represents a discrete 2D spatial partition of the infinite continuous world.
///
/// ## 2. Why It Happens
/// An infinite world cannot be loaded into memory all at once. By partitioning space into
/// chunks, the engine can dynamically load, unload, and serialize regions of space
/// based on the camera's location and the density of organisms.
///
/// ## 3. How It Happens
/// Calculated by taking continuous world coordinates $(X, Y)$ and dividing by the configured
/// chunk size, then flooring the result to `i32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkId(pub i32, pub i32);

impl ChunkId {
    /// Constructs a [`ChunkId`] from separate x and y chunk coordinates.
    #[inline]
    pub fn new(x: i32, y: i32) -> Self {
        Self(x, y)
    }

    /// Returns the chunk coordinate as a [`glam::IVec2`].
    #[inline]
    pub fn as_ivec2(self) -> IVec2 {
        IVec2::new(self.0, self.1)
    }
}

impl std::fmt::Display for ChunkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Chunk({}, {})", self.0, self.1)
    }
}

/// # Deterministic Simulation Tick
///
/// ## 1. What Happens
/// `Tick` is a strictly monotonic counter representing the current discrete step of the
/// physics and biological simulation.
///
/// ## 2. Why It Happens
/// Artificial life experiments require perfect determinism. Relying on floating-point
/// delta-time (`dt`) accumulation leads to subtle floating-point drift across different
/// CPU architectures. A discrete integer tick guarantees that the simulation state at
/// `Tick(10_000)` is identical no matter the hardware.
///
/// ## 3. How It Happens
/// The `SimulationScheduler` executes the entire suite of engine systems. Once all systems
/// complete their passes for the current state, the global `Tick` is incremented by 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Tick(pub u64);

impl Tick {
    /// The zero tick — the state before any simulation step has run.
    pub const ZERO: Self = Self(0);

    /// Advances this tick by one, returning the next tick value.
    #[inline]
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// Returns the number of ticks elapsed since `earlier`.
    ///
    /// Returns `0` if `earlier` is after `self` (prevents underflow).
    #[inline]
    pub fn elapsed_since(self, earlier: Self) -> u64 {
        self.0.saturating_sub(earlier.0)
    }
}

impl std::fmt::Display for Tick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tick({})", self.0)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Simulation unit newtypes
// ────────────────────────────────────────────────────────────────────────────

/// A length measured in simulation length units (su).
///
/// Do **not** interpret this as metres or any real-world unit unless a
/// per-experiment conversion table has been explicitly defined.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SimLength(pub f32);

/// A mass measured in simulation mass units (smu).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SimMass(pub f32);

/// An energy quantity measured in simulation energy units (seu).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SimEnergy(pub f32);

/// A sub-tick time fraction used for rendering interpolation.
///
/// Valid range is `[0.0, 1.0)` where `0.0` is the start of the current tick
/// and values approaching `1.0` represent the state just before the next tick.
/// This value is **not** used in simulation logic — only in the render path.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SimTime(pub f32);

// ────────────────────────────────────────────────────────────────────────────
// Deterministic RNG
// ────────────────────────────────────────────────────────────────────────────

/// # The Single Seeded Source of Randomness
///
/// ## 1. What Happens
/// `SimRng` wraps a `ChaCha8Rng` instance. It is inserted into the ECS
/// `World` exactly once, as a [`bevy_ecs::prelude::Resource`], and is the one
/// shared generator every stochastic system draws from: mutation, crossover,
/// spawn placement, mate selection, and any future stochastic decision.
///
/// ## 2. Why It Happens
/// Reproducible research requires that identical seed + identical recorded
/// interventions produce an identical simulation trajectory. An unseeded,
/// platform/thread-dependent source of randomness (`rand::thread_rng()`)
/// breaks this guarantee silently — two runs of "the same" experiment
/// diverge from tick 0 with no way to tell why. Centralizing every draw
/// through one seeded generator closes that gap at its root.
///
/// ## 3. How It Happens
/// `SimRng::from_seed` is called once, at application startup, using the
/// seed recorded in `PhylonConfig::simulation::rng_seed`. Systems that need
/// randomness take `ResMut<SimRng>` as an ECS system parameter (or receive
/// `&mut ChaCha8Rng` passed down from a caller already holding the
/// resource) instead of reaching for `rand::thread_rng()`.
#[derive(Resource, Debug)]
pub struct SimRng(pub ChaCha8Rng);

impl SimRng {
    /// Constructs a `SimRng` seeded deterministically from a 64-bit seed.
    ///
    /// The same seed always produces the same sequence of draws on any
    /// platform — `ChaCha8Rng` is explicitly designed for cross-platform,
    /// cross-version bit-for-bit reproducibility, unlike
    /// `rand::thread_rng()`, whose output depends on OS entropy and thread
    /// state.
    #[inline]
    pub fn from_seed(seed: u64) -> Self {
        Self(ChaCha8Rng::seed_from_u64(seed))
    }
}

impl std::ops::Deref for SimRng {
    type Target = ChaCha8Rng;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for SimRng {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Tick rate
// ────────────────────────────────────────────────────────────────────────────

/// # The Single Source of Truth for the Simulation's Tick Duration
///
/// ## 1. What Happens
/// `TickRate` wraps the fixed per-tick delta-time (in seconds) derived from
/// `PhylonConfig::simulation::tick_rate`. It is inserted into the ECS
/// `World` exactly once, as a [`bevy_ecs::prelude::Resource`], and is the
/// one shared value every fixed-timestep calculation reads: the physics/
/// CTRNN integration math, the diffusion GPU dispatch, the windowed
/// render loop's tick-accumulator, and any tick-count-from-elapsed-time
/// bookkeeping (status bar, save/reset handlers).
///
/// ## 2. Why It Happens
/// Before this type existed, the same `0.016` (60 Hz) literal was
/// hand-copied into five separate call sites across three crates. Nothing
/// enforced that they agreed with each other or with
/// `PhylonConfig::simulation::tick_rate` — changing `tick_rate` in config
/// silently affected only the headless loop's pacing, while the windowed
/// loop and the physics integration math kept using their own hardcoded
/// copies. Centralizing the value here closes that gap the same way
/// [`SimRng`] closes the determinism gap.
///
/// ## 3. How It Happens
/// `TickRate::from_hz` is called once, at application startup, from
/// `PhylonConfig::simulation::tick_rate`. Every fixed-timestep call site
/// reads `dt()` from the shared resource instead of a local constant.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct TickRate(f32);

impl TickRate {
    /// Constructs a `TickRate` from a tick frequency in Hz (ticks per second).
    #[inline]
    pub fn from_hz(hz: u32) -> Self {
        Self(1.0 / hz as f32)
    }

    /// The fixed per-tick delta-time, in seconds.
    #[inline]
    pub fn dt(&self) -> f32 {
        self.0
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Error foundation
// ────────────────────────────────────────────────────────────────────────────

/// Marker trait implemented by all Phylon domain error types.
///
/// Every library crate in the workspace defines its own typed error enum
/// using `thiserror` and implements this trait. The trait enforces that all
/// errors are `Send + Sync + 'static` so they can safely cross thread
/// boundaries and be stored in [`PhylonResult`].
pub trait PhylonError: std::error::Error + Send + Sync + 'static {}

/// The canonical result type used by all Phylon public APIs.
///
/// The error variant is a trait object so callers can mix errors from
/// different subsystems without defining wrapper enums at every call site.
/// For domain-specific code, prefer returning the concrete typed error directly.
pub type PhylonResult<T> = std::result::Result<T, Box<dyn PhylonError>>;

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_id_null_sentinel() {
        assert!(EntityId::NULL.is_null());
        assert!(!EntityId(0).is_null());
    }

    #[test]
    fn tick_ordering() {
        let t0 = Tick(0);
        let t1 = Tick(1);
        let t100 = Tick(100);
        assert!(t0 < t1);
        assert!(t1 < t100);
        assert_eq!(t0.next(), t1);
    }

    #[test]
    fn tick_elapsed_since_no_underflow() {
        let early = Tick(10);
        let late = Tick(100);
        assert_eq!(late.elapsed_since(early), 90);
        // Reversed: saturates to zero
        assert_eq!(early.elapsed_since(late), 0);
    }

    #[test]
    fn chunk_id_roundtrip() {
        let c = ChunkId::new(-3, 7);
        assert_eq!(c.as_ivec2(), IVec2::new(-3, 7));
    }

    #[test]
    fn sim_unit_ordering() {
        let e1 = SimEnergy(1.0);
        let e2 = SimEnergy(2.0);
        assert!(e1 < e2);
    }

    #[test]
    fn sim_rng_same_seed_produces_same_sequence() {
        use rand::Rng;

        let mut a = SimRng::from_seed(42);
        let mut b = SimRng::from_seed(42);

        let draws_a: Vec<u32> = (0..16).map(|_| a.gen()).collect();
        let draws_b: Vec<u32> = (0..16).map(|_| b.gen()).collect();

        assert_eq!(draws_a, draws_b);
    }

    #[test]
    fn sim_rng_different_seeds_diverge() {
        use rand::Rng;

        let mut a = SimRng::from_seed(1);
        let mut b = SimRng::from_seed(2);

        let draws_a: Vec<u32> = (0..16).map(|_| a.gen()).collect();
        let draws_b: Vec<u32> = (0..16).map(|_| b.gen()).collect();

        assert_ne!(draws_a, draws_b);
    }

    #[test]
    fn tick_rate_from_hz_60_matches_expected_dt() {
        let rate = TickRate::from_hz(60);
        assert!((rate.dt() - 1.0 / 60.0).abs() < f32::EPSILON);
    }

    #[test]
    fn tick_rate_from_hz_30_is_double_the_dt_of_60() {
        let rate_30 = TickRate::from_hz(30);
        let rate_60 = TickRate::from_hz(60);
        assert!((rate_30.dt() - rate_60.dt() * 2.0).abs() < f32::EPSILON);
    }
}
