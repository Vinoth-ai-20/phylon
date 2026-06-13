//! Common types, traits, and utilities for the Phylon simulation.
//!
//! Phase 0 implementation of base identifiers, physics units, math, and errors.

use std::error::Error;
pub use glam::{Vec2, Vec2Swizzles, IVec2};

/// Globally unique entity ID — must be unique across processes for future distributed sim
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EntityId(pub u64);

/// Coordinate identifying a chunk in the 2D grid
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ChunkId(pub i32, pub i32);

/// Monotonically increasing tick counter
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Tick(pub u64);

// Simulation Units

/// Simulation length unit (su)
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SimLength(pub f32);

/// Simulation mass unit (smu)
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SimMass(pub f32);

/// Simulation energy unit (seu)
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SimEnergy(pub f32);

/// Simulation time (fractional ticks for interpolation)
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SimTime(pub f32);

// Errors

/// Base error trait for all Phylon libraries
pub trait PhylonError: Error + Send + Sync + 'static {}

/// Universal Result alias
pub type PhylonResult<T> = std::result::Result<T, Box<dyn PhylonError>>;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tick_ordering() {
        assert!(Tick(1) < Tick(2));
    }
}
