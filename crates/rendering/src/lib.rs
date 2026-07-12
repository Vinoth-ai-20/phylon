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
//! Organisms render as GPU-instanced, oriented capsule meshes (see
//! [`organism`]) — a hemisphere-capped cylinder per body-graph bone — shaded
//! with a physically-based (Cook-Torrance) lighting model and a real depth
//! buffer. This gives correct occlusion and lighting between organisms and
//! scales to large populations via instancing rather than one draw call per
//! organism. A diffusion-field heatmap ([`field`]) renders as a background
//! overlay, and an optional wireframe/structural [`debug`] view draws
//! camera-facing billboards depth-tested against the same scene depth.

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

/// Procedural capsule mesh generation.
pub mod capsule_mesh;

/// Mesh-based, GPU-instanced organism rendering module — draws every
/// organism/pellet bone as a lit, depth-correct capsule.
pub mod organism;
pub use organism::{CapsuleInstance, ClipPlane, OrganismRenderer};

/// Ray-vs-capsule picking — a real 3D ray cast against the exact capsule
/// primitives the renderer draws, used for entity selection.
pub mod picking;
pub use picking::ray_capsule_hit;

#[cfg(test)]
mod tests {
    // Tests for DebugRenderer will go here
}
