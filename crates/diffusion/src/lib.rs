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
    pub base_decay_rate: f32,
    /// Global time for diurnal modulation
    pub global_time: f32,
    /// The current calculated decay rate (modulated by time)
    pub decay_rate: f32,
}

impl Default for DiffusionConfig {
    fn default() -> Self {
        Self {
            diffusion_rate: 0.1,
            base_decay_rate: 0.005,
            global_time: 0.0,
            decay_rate: 0.005,
        }
    }
}

/// The latest state of the diffusion field read back from the GPU.
#[derive(Resource, Clone, Debug)]
pub struct CpuFieldState {
    /// The 2D grid data (e.g., 256x256)
    pub data: Vec<f32>,
    /// The width of the grid
    pub width: u32,
    /// The height of the grid
    pub height: u32,
}

impl Default for CpuFieldState {
    fn default() -> Self {
        Self {
            data: vec![0.0; 256 * 256],
            width: 256,
            height: 256,
        }
    }
}

impl CpuFieldState {
    /// Samples the field at a given world position.
    pub fn sample(&self, pos: Vec2) -> f32 {
        let gx = (pos.x / 10.0) + (self.width as f32 / 2.0);
        let gy = (pos.y / 10.0) + (self.height as f32 / 2.0);

        let ix = gx.floor() as i32;
        let iy = gy.floor() as i32;

        if ix >= 0 && ix < self.width as i32 && iy >= 0 && iy < self.height as i32 {
            let idx = (iy * self.width as i32 + ix) as usize;
            if idx < self.data.len() {
                return self.data[idx];
            }
        }
        0.0
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
