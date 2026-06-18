//! Continuous field simulation: chemical and atmospheric field diffusion via discrete Laplacian.

#![warn(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::prelude::*;
use common::Vec2;

/// Configuration for the global diffusion field.
#[derive(Resource, Clone, Debug)]
pub struct DiffusionConfig {
    /// Diffusion rate (D in the PDE)
    pub diffusion_rate: f32,
    /// Decay rate (λ in the PDE)
    pub decay_rate: f32,
}

impl Default for DiffusionConfig {
    fn default() -> Self {
        Self {
            diffusion_rate: 0.1,
            decay_rate: 0.005,
        }
    }
}

/// A spatial emitter that adds a quantity to the field per tick.
#[derive(Component, Clone, Debug)]
pub struct Emitter {
    /// World position of the emitter
    pub position: Vec2,
    /// The value to add per tick
    pub value: f32,
    /// The radius of emission (world space)
    pub radius: f32,
}
