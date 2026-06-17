//! # Phylon Diffusion
//!
//! Continuous field simulation for all chemical and atmospheric fields.
//!
//! Fields are stored as flat 2D arrays tiled to the chunk grid. Each tick,
//! the diffusion operator applies a discrete Laplacian (explicit Euler) to
//! spread field values across the grid. The GPU compute version of this
//! system is introduced in Phase 3.
//!
//! ## Fields simulated
//!
//! - Chemical: O₂, CO₂, nutrients, pheromones, toxins, disease load
//! - Physical: temperature, sound pressure, bioluminescence
//!
//! ## Phase 0 scope
//!
//! Field type enumeration and placeholder diffusion system. Full CPU
//! implementation: Phase 2. GPU compute: Phase 3.

#![warn(missing_docs)]
#![warn(clippy::all)]

use common::ChunkId;

/// Identifies a diffusion field by type.
///
/// Mirrors [`events::FieldType`] but defined here so the diffusion crate
/// has no dependency on `events`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldKind {
    /// Atmospheric oxygen concentration.
    Oxygen,
    /// Atmospheric carbon dioxide concentration.
    CarbonDioxide,
    /// Nutrient / food density.
    Nutrient,
    /// Pheromone chemical signal.
    Pheromone,
    /// Thermal energy.
    Temperature,
    /// Toxin concentration.
    Toxin,
    /// Pathogen / disease concentration.
    Disease,
    /// Bioluminescent emission intensity.
    Bioluminescence,
    /// Sound pressure wave amplitude.
    SoundPressure,
}

/// Errors produced by the diffusion subsystem.
#[derive(Debug, thiserror::Error)]
pub enum DiffusionError {
    /// A diffusion step was requested for an inactive chunk.
    #[error("chunk {0} is not active — cannot diffuse")]
    InactiveChunk(ChunkId),
}

impl common::PhylonError for DiffusionError {}

/// Placeholder diffusion system.
///
/// TODO(phase-2): Implement CPU-side discrete Laplacian diffusion for each
/// FieldKind across all active chunks.
/// TODO(phase-3): Add GPU compute shader dispatch via the `gpu` crate.
pub struct DiffusionSystem;

impl DiffusionSystem {
    /// Creates a new diffusion system.
    pub fn new() -> Self {
        Self
    }
}

impl Default for DiffusionSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_kind_is_copy() {
        let fk = FieldKind::Oxygen;
        let _fk2 = fk; // copy semantics
    }

    #[test]
    fn diffusion_system_creates() {
        let _sys = DiffusionSystem::new();
    }
}
