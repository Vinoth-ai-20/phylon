//! # Phylon GPU
//!
//! `wgpu` device and queue management, compute pipeline registry, staging
//! buffer pool, and GPU resource allocator.
//!
//! This crate is the boundary between the CPU-authoritative simulation and
//! the GPU accelerator. All GPU resources are created and managed here.
//! Simulation crates never touch `wgpu` directly — they call into `gpu`
//! through typed interfaces.
//!
//! ## Phase 0 scope
//!
//! GpuContext placeholder. Full wgpu initialisation and compute pipelines: Phase 3.

#![warn(missing_docs)]
#![warn(clippy::all)]

/// Muscle compute pipeline module.
pub mod muscle;

/// Diffusion compute pipeline module.
pub mod diffusion_pipeline;

/// Physics compute pipeline module for forces and PBD projection.
pub mod physics_pipeline;

/// Brain compute pipeline module for CTRNN integration.
pub mod brain_pipeline;

/// Errors from GPU resource management.
#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    /// No suitable GPU adapter was found on this system.
    #[error("no suitable GPU adapter found")]
    NoAdapter,

    /// Device creation failed.
    #[error("GPU device creation failed: {message}")]
    DeviceCreationFailed {
        /// Description of the failure.
        message: String,
    },
}

impl common::PhylonError for GpuError {}

/// Placeholder for the GPU context.
///
/// TODO(phase-3): Hold `wgpu::Device`, `wgpu::Queue`, and the compute
/// pipeline registry.
pub struct GpuContext;

impl GpuContext {
    /// Creates a placeholder GPU context (no real device is acquired yet).
    pub fn placeholder() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_context_placeholder_creates() {
        let _ctx = GpuContext::placeholder();
    }
}
