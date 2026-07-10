//! # Phylon Rendering
//!
//! `wgpu` render pipeline, scene graph, debug renderer, and visual overlays.
//!
//! The rendering crate translates CPU-authoritative simulation state into
//! GPU draw calls. It depends on the `gpu` crate for device access and the
//! `world` crate for entity positions and visual parameters.
//!
//! ## Rendering strategy
//!
//! - Phase 1: Minimal debug renderer — coloured dot per entity + field texture.
//! - Phase 7: 2-pass SDF metaball organism skin, trails, MRT overlays.
//! - Phase 8 (ADR-P8-03): mesh-based capsule instancing with a real depth
//!   buffer and PBR shading, replacing the SDF metaball technique.

#![warn(missing_docs)]
#![warn(clippy::all)]

/// Errors from the rendering subsystem.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// The wgpu surface was lost and must be reconfigured.
    #[error("render surface lost")]
    SurfaceLost,

    /// A pipeline configuration is invalid.
    #[error("invalid render pipeline: {message}")]
    InvalidPipeline {
        /// Description of the problem.
        message: String,
    },
}

impl common::PhylonError for RenderError {}

/// Debug rendering module.
pub mod debug;
pub use debug::{DebugInstance, DebugRenderer};

/// Field overlay rendering module.
pub mod field;
pub use field::{FieldConfig, FieldRenderer, GpuSplat, SplatComputePipeline, SplatConfig};

/// Procedural capsule mesh generation (Phase 8, ADR-P8-03).
pub mod capsule_mesh;

/// Mesh-based capsule organism rendering module (Phase 8, ADR-P8-03) —
/// replaces the retired `SdfSkinRenderer` (2-pass SDF metaball technique;
/// see git history prior to this epic for that implementation).
pub mod organism;
pub use organism::{CapsuleInstance, OrganismRenderer};

#[cfg(test)]
mod tests {
    // Tests for DebugRenderer will go here
}
