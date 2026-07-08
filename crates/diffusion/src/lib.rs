//! Continuous field simulation: chemical and atmospheric field diffusion via discrete Laplacian.

#![warn(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::prelude::*;
use common::Vec2;

/// # Global Diffusion Configuration
///
/// ## 1. What Happens
/// The `DiffusionConfig` holds the global thermodynamic constants governing the Partial Differential
/// Equations (PDEs) solved by the GPU. It defines the base diffusion rate ($D$) and the evaporation/decay
/// rate ($\lambda$).
///
/// ## 2. Why It Happens
/// In real environments, chemical signals and gases do not persist forever. They diffuse across
/// a gradient and chemically break down over time. Without a decay rate ($\lambda$), any emitted
/// pheromone would accumulate infinitely until the entire map saturated to maximum capacity, rendering
/// spatial navigation impossible.
///
/// ## 3. How It Happens
/// The GPU compute shader solves a variation of the Reaction-Diffusion equation. For a chemical
/// concentration $C$:
///
/// $$ \frac{\partial C}{\partial t} = D \nabla^2 C - \lambda C + E $$
///
/// Where $D$ is `diffusion_rate`, $\lambda$ is `decay_rate`, and $E$ is the external emission source.
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

/// # CPU Field Readback Buffer
///
/// ## 1. What Happens
/// `CpuFieldState` maintains a linear `Vec<f32>` representation of the continuous 2D spatial grid.
/// It provides methods to map continuous physical coordinates to discrete array indices to sample
/// environmental concentrations (O2, Pheromones).
///
/// ## 2. Why It Happens
/// The diffusion PDEs are computed entirely on the GPU for performance. However, biological
/// organisms running in the CPU ECS (like olfactory sensors or photosynthetic leaves) need to
/// sample their local environment to make decisions. The GPU buffer must be asynchronously copied
/// back to main system memory each tick to provide this spatial data to the CPU.
///
/// ## 3. How It Happens
/// The `sample` method maps a continuous world vector $\vec{P} = \langle x, y \rangle$ to a
/// discrete 1D array index $i$ on a grid of width $W$ and height $H$, with a cell resolution $R$:
///
/// $$ G_x = \lfloor \frac{P_x}{R} + \frac{W}{2} \rfloor $$
/// $$ G_y = \lfloor \frac{P_y}{R} + \frac{H}{2} \rfloor $$
///
/// Indexing into the multi-layered 1D array for a specific $Layer$:
///
/// $$ i = (Layer \times W \times H) + (G_y \times W + G_x) $$
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
    /// Samples the field at a given world position for a specific layer.
    /// Layer 0: Pheromones, 1: Energy, 2: O2, 3: CO2, 4: Morphogen (Phase 6,
    /// Epic D, D1b — see [`FieldLayer::Morphogen`]).
    pub fn sample(&self, pos: Vec2, layer: u32) -> f32 {
        let gx = (pos.x / 10.0) + (self.width as f32 / 2.0);
        let gy = (pos.y / 10.0) + (self.height as f32 / 2.0);

        let ix = gx.floor() as i32;
        let iy = gy.floor() as i32;

        if ix >= 0 && ix < self.width as i32 && iy >= 0 && iy < self.height as i32 {
            let layer_offset = (layer * self.width * self.height) as usize;
            let idx = layer_offset + (iy * self.width as i32 + ix) as usize;
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
    /// World position
    pub position: Vec2,
    /// Value emitted per tick (can be negative for absorption)
    pub value: f32,
    /// Radius of the emission
    pub radius: f32,
    /// Which field layer this emitter affects
    pub layer: FieldLayer,
}

/// Represents the type of layer an emitter affects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldLayer {
    /// Pheromones layer for ant-like communication.
    Pheromones = 0,
    /// Spatial energy layer for localized resources.
    Energy = 1,
    /// Spatial oxygen layer.
    O2 = 2,
    /// Spatial carbon dioxide layer.
    CO2 = 3,
    /// Phase 6, Epic D (D1b, ADR-D1-01): the inter-organism/environmental
    /// developmental-coupling layer — emitted into by developing organisms
    /// (proportional to their own intra-organism `MorphogenLevel`, see
    /// `organisms::morphogen_field`) and sampled by nearby developing
    /// organisms' own decode. This is the "5th layer" ADR-D1-01 calls for;
    /// the intra-organism signal itself (D1a) never touches the GPU.
    Morphogen = 4,
}

/// The latest state of the signal diffusion field read back from the GPU.
#[derive(Resource, Clone, Debug)]
pub struct CpuSignalFieldState {
    /// The 2D grid data (e.g., 256x256)
    pub data: Vec<f32>,
    /// The width of the grid
    pub width: u32,
    /// The height of the grid
    pub height: u32,
}

impl Default for CpuSignalFieldState {
    fn default() -> Self {
        Self {
            data: vec![0.0; 256 * 256],
            width: 256,
            height: 256,
        }
    }
}

impl CpuSignalFieldState {
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

/// # Biological Signal Emitter
///
/// ## 1. What Happens
/// The `SignalEmitter` component allows an organism to inject mass or concentration values
/// into the spatial diffusion fields.
///
/// ## 2. Why It Happens
/// Communication in primitive ALife (like ants) is often stigmergic—organisms modify their
/// spatial environment to leave persistent signals rather than communicating directly via sound
/// or sight. The emitter acts as the biological gland injecting the chemical.
///
/// ## 3. How It Happens
/// Each simulation tick, the `behavior_system` extracts an output scalar from the CTRNN
/// neural network. This scalar $[0, 1]$ sets the `value` field. During the GPU dispatch, all
/// active emitters are packed into a `GpuEmitter` uniform array. The shader then integrates
/// this mass into the local discrete cell $C$:
///
/// $$ C_{t+1} = C_{t} + (\text{value} \times DT) $$
#[derive(Component, Clone, Debug)]
pub struct SignalEmitter {
    /// The value to add per tick. Typically driven by a brain output node.
    pub value: f32,
    /// The radius of emission (world space).
    pub radius: f32,
}

impl Default for SignalEmitter {
    fn default() -> Self {
        Self {
            value: 0.0,
            radius: 10.0,
        }
    }
}

/// The latest state of the hazard diffusion field read back from the GPU.
#[derive(Resource, Clone, Debug)]
pub struct CpuHazardFieldState {
    /// The 2D grid data (e.g., 256x256)
    pub data: Vec<f32>,
    /// The width of the grid
    pub width: u32,
    /// The height of the grid
    pub height: u32,
}

impl Default for CpuHazardFieldState {
    fn default() -> Self {
        Self {
            data: vec![0.0; 256 * 256],
            width: 256,
            height: 256,
        }
    }
}

impl CpuHazardFieldState {
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

    /// Splats a radial value onto the grid.
    pub fn splat(&mut self, pos: Vec2, radius: f32, max_val: f32) {
        let gx = (pos.x / 10.0) + (self.width as f32 / 2.0);
        let gy = (pos.y / 10.0) + (self.height as f32 / 2.0);

        let r_cells = (radius / 10.0).ceil() as i32;

        for dy in -r_cells..=r_cells {
            for dx in -r_cells..=r_cells {
                let ix = gx as i32 + dx;
                let iy = gy as i32 + dy;

                if ix >= 0 && ix < self.width as i32 && iy >= 0 && iy < self.height as i32 {
                    let d = ((dx as f32).powi(2) + (dy as f32).powi(2)).sqrt();
                    if d <= r_cells as f32 {
                        let intensity = (1.0 - (d / r_cells as f32)).max(0.0) * max_val;
                        let idx = (iy * self.width as i32 + ix) as usize;
                        if idx < self.data.len() {
                            self.data[idx] = self.data[idx].max(intensity);
                        }
                    }
                }
            }
        }
    }

    /// Clears the hazard field.
    pub fn clear(&mut self) {
        self.data.fill(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Phase 6, Epic D (D1b)'s own named testing requirement: the new
    /// `FieldLayer::Morphogen` (index 4) must round-trip independently of
    /// the other 4 layers — no cross-channel bleed. `CpuFieldState::sample`'s
    /// layer offset is `layer * width * height`; this proves that offset
    /// actually isolates layer 4's data from layers 0-3, not just that the
    /// arithmetic looks right on paper.
    #[test]
    fn cpu_field_state_samples_the_morphogen_layer_independently_of_the_other_4_layers() {
        let width = 4u32;
        let height = 4u32;
        let layer_size = (width * height) as usize;
        let mut data = vec![0.0f32; layer_size * 5];

        // Distinct, recognizable values per layer at the same grid cell.
        for layer in 0..5u32 {
            let idx = (layer as usize) * layer_size; // cell (0, 0) of each layer
            data[idx] = (layer + 1) as f32 * 10.0;
        }

        let field = CpuFieldState {
            data,
            width,
            height,
        };

        // World position that maps to grid cell (0, 0) given `sample`'s own
        // `pos / 10.0 + dimension / 2.0` convention.
        let pos = Vec2::new(-(width as f32 / 2.0) * 10.0, -(height as f32 / 2.0) * 10.0);

        assert_eq!(field.sample(pos, FieldLayer::Pheromones as u32), 10.0);
        assert_eq!(field.sample(pos, FieldLayer::Energy as u32), 20.0);
        assert_eq!(field.sample(pos, FieldLayer::O2 as u32), 30.0);
        assert_eq!(field.sample(pos, FieldLayer::CO2 as u32), 40.0);
        assert_eq!(field.sample(pos, FieldLayer::Morphogen as u32), 50.0);
    }
}
