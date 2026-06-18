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
//! - Phase 7: Full pipeline — SDF organisms, trails, MRT overlays.
//!
//! ## Phase 0 scope
//!
//! Type stubs only. Implementation: Phase 1 (debug renderer).

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
pub use field::FieldRenderer;

/// SDF skin rendering module (capsule-SDF organic skin).
pub mod sdf_skin;
pub use sdf_skin::{SdfBoneInstance, SdfSkinRenderer};

#[cfg(test)]
mod tests {
    // Tests for DebugRenderer will go here
}
