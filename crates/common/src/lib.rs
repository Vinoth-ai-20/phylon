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

#![warn(missing_docs)]
#![warn(clippy::all)]

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

/// Globally unique entity identifier.
///
/// The 64-bit space is wide enough to accommodate distributed simulation
/// scenarios where multiple processes mint IDs independently (Phase 12).
/// IDs are minted by a monotonic atomic counter seeded from the experiment
/// manifest so they are unique and reproducible per-experiment.
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

/// Identifies a spatial chunk in the world grid.
///
/// The world is divided into fixed-size chunks addressed by a signed integer
/// pair. The origin chunk `(0, 0)` is the spawn-zone for the default scenario.
/// Negative coordinates extend in the West / South directions.
///
/// Chunk IDs are designed for future distributed ownership: a separate process
/// can be assigned a contiguous rectangle of chunk IDs.
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

/// The canonical, monotonically increasing simulation time counter.
///
/// One [`Tick`] corresponds to one fixed-timestep simulation update. Rendering
/// may interpolate *between* ticks using a fractional [`SimTime`] value, but
/// all simulation state transitions happen on tick boundaries.
///
/// Ticks are ordered and comparable so they can be used as event timestamps,
/// snapshot labels, and replay indices.
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
}
