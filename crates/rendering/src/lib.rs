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

/// Placeholder for the renderer.
///
/// TODO(phase-1): Implement debug renderer with wgpu clear + entity dot pass.
/// TODO(phase-7): Implement full wgpu render pipeline.
pub struct Renderer;

impl Renderer {
    /// Creates a placeholder renderer.
    pub fn placeholder() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_placeholder_creates() {
        let _r = Renderer::placeholder();
    }
}
