//! # Phylon GPU
//!
//! Compute pipeline modules — the boundary between the CPU-authoritative
//! simulation and the GPU accelerator, per the spec's GPU policy: this
//! crate holds only compute *pipelines* (dispatch + readback for physics,
//! diffusion, CTRNN brain integration, and muscle actuation), not a
//! device/queue-owning context of its own.
//!
//! ## Currently implemented
//!
//! - [`physics_pipeline`] — particle-spring force/constraint integration.
//! - [`diffusion_pipeline`] — chemical/atmospheric field diffusion.
//! - [`brain_pipeline`] — batched CTRNN neural integration.
//! - [`muscle`] — muscle/spring actuation.
//! - [`GpuError`] — the crate's typed error enum.
//!
//! Each pipeline module owns its own `dispatch`/`step` entry point, taking
//! an already-acquired `&wgpu::Device`/`&wgpu::Queue` from the caller
//! rather than owning them — device/queue lifecycle (surface
//! configuration, adapter selection, resize handling) lives in
//! `app::app::GpuContext` today, a separate type from anything in this
//! crate despite the similar name. Consolidating device/queue ownership
//! into this crate, if ever done, is a distinct future change, not
//! reflected here as a claim about current behavior.

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
