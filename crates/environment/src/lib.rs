//! # Phylon Environment
//!
//! Procedural world generation: continuous temperature/humidity noise fields
//! and the discrete [`Biome`] classification derived from them.
//!
//! ## Purpose
//!
//! Organisms and the ecology system need a spatially-varying, deterministic
//! notion of "what kind of place is this" — how fertile the ground is, how
//! hot or cold, how wet — without simulating real climate. This crate
//! generates that world once per coordinate, cheaply and reproducibly, from
//! a seeded noise field rather than a hand-authored map.
//!
//! ## Architecture
//!
//! [`EnvironmentManager`] owns two independent `OpenSimplex` noise
//! generators (temperature, humidity), both seeded from the experiment's RNG
//! seed so that "the same seed" reproduces the same world layout, not just
//! the same organism behavior. [`EnvironmentManager::get_temperature_at`] and
//! [`EnvironmentManager::get_humidity_at`] sample those fields directly;
//! [`EnvironmentManager::get_biome_at`] combines both samples through a
//! simplified Whittaker biome-classification scheme to produce a discrete
//! [`Biome`], which in turn determines fertility via [`Biome::fertility`].
//!
//! ## Design decisions
//!
//! Noise sampling is seeded, not random-per-call, and the manager is
//! inserted into the ECS `World` as a `bevy_ecs` [`Resource`] so every system
//! that reads environmental conditions (ecology, spawn placement) sees the
//! same values for the same coordinate — the world layout itself is part of
//! the simulation's deterministic state, not just organism behavior.
//!
//! When the world is toroidal (wraps at its boundaries), naively sampling 2D
//! noise would produce a visible seam where the wrap occurs. Instead,
//! coordinates are projected onto two independent circles and sampled from a
//! 4D noise field (see the internal `eval_noise` helper's doc comment for the
//! derivation), which tiles seamlessly by construction.

#![warn(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::system::Resource;
use noise::{NoiseFn, OpenSimplex};
use serde::{Deserialize, Serialize};

/// Discrete biome classification for a world coordinate, derived from local
/// temperature and humidity via [`EnvironmentManager::get_biome_at`].
///
/// Each variant carries an implied fertility level (see [`Biome::fertility`])
/// that the ecology system uses to calibrate resource/food density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Biome {
    /// Hot, humid — highest fertility.
    TropicalRainforest,
    /// Moderate temperature, moderate-to-high humidity.
    TemperateForest,
    /// Hot, dry — low fertility.
    Desert,
    /// Below-freezing — lowest fertility.
    Tundra,
    /// Moderate temperature, low-to-moderate humidity.
    Grassland,
    /// Inland water body.
    Freshwater,
    /// Ocean-adjacent water, high fertility.
    CoastalMarine,
    /// Open ocean, away from coastal influence.
    DeepOcean,
    /// Geothermally active water (vents) — non-photosynthetic fertility source.
    Hydrothermal,
}

impl Biome {
    /// Helper to get the relative fertility multiplier of a biome (0.0 to 1.0+).
    pub fn fertility(&self) -> f32 {
        match self {
            Biome::TropicalRainforest => 1.5,
            Biome::TemperateForest => 1.0,
            Biome::Grassland => 0.8,
            Biome::CoastalMarine => 1.2,
            Biome::Freshwater => 1.0,
            Biome::Tundra => 0.2,
            Biome::Desert => 0.1,
            Biome::DeepOcean => 0.4,
            Biome::Hydrothermal => 0.8,
        }
    }
}

/// The global environment resource managing noise and climatic conditions.
#[derive(Resource)]
pub struct EnvironmentManager {
    toroidal: bool,
    width: f32,
    height: f32,
    noise_temp: OpenSimplex,
    noise_humid: OpenSimplex,
}

impl EnvironmentManager {
    /// Creates a new environment manager.
    pub fn new(seed: u64, toroidal: bool, width: f32, height: f32) -> Self {
        let seed32 = (seed & 0xFFFFFFFF) as u32;
        Self {
            toroidal,
            width,
            height,
            noise_temp: OpenSimplex::new(seed32),
            noise_humid: OpenSimplex::new(seed32.wrapping_add(0x1234567)),
        }
    }

    /// Returns the global world width.
    pub fn width(&self) -> f32 {
        self.width
    }

    /// Returns the global world height.
    pub fn height(&self) -> f32 {
        self.height
    }

    /// # Toroidal Noise Evaluation
    ///
    /// ## 1. What Happens
    /// The `eval_noise` function samples an `OpenSimplex` noise generator. If the environment is
    /// configured to be `toroidal` (wrapping boundaries), it projects the 2D Cartesian coordinates
    /// into a 4D hyperspace to ensure the noise field loops seamlessly without artifacts.
    ///
    /// ## 2. Why It Happens
    /// In ALife simulations, boundary conditions strongly affect population dynamics. Hard borders
    /// cause organisms to pile up in corners. A Torus (Pac-Man map) solves this, but standard 2D
    /// Perlin/Simplex noise does not tile seamlessly. To create continuous tiling, we must wrap both
    /// the $X$ and $Y$ dimensions into circles, which mathematically requires 4 dimensions.
    ///
    /// ## 3. How It Happens
    /// The 2D coordinates $(X, Y)$ on a grid of size $(W, H)$ are mapped to two independent angles
    /// $\theta_x, \theta_y$ in $[0, 2\pi]$. The evaluation is then sampled from a 4D noise space
    /// $N(x_1, y_1, x_2, y_2)$:
    ///
    /// $$ \theta_x = \frac{X}{W} 2\pi $$
    /// $$ \theta_y = \frac{Y}{H} 2\pi $$
    /// $$ R_x = \frac{W}{Scale \times 2\pi}, \quad R_y = \frac{H}{Scale \times 2\pi} $$
    ///
    /// $$ N_{val} = Noise_{4D}(R_x \cos\theta_x, R_x \sin\theta_x, R_y \cos\theta_y, R_y \sin\theta_y) $$
    fn eval_noise(&self, noise: &OpenSimplex, x: f32, y: f32, scale: f32) -> f64 {
        if self.toroidal {
            // Map [0, width] -> angle in [0, 2pi]
            // Map [0, height] -> angle in [0, 2pi]
            let x_rad = (x / self.width) * std::f32::consts::TAU;
            let y_rad = (y / self.height) * std::f32::consts::TAU;

            // To maintain a similar spatial frequency to standard 2D noise,
            // the radius of the torus in 4D space relates to the scale.
            let r_x = self.width / (scale * std::f32::consts::TAU);
            let r_y = self.height / (scale * std::f32::consts::TAU);

            // 4D torus coordinates
            let nx = (x_rad.cos() * r_x) as f64;
            let ny = (x_rad.sin() * r_x) as f64;
            let nz = (y_rad.cos() * r_y) as f64;
            let nw = (y_rad.sin() * r_y) as f64;

            noise.get([nx, ny, nz, nw])
        } else {
            // Standard 2D noise
            noise.get([(x / scale) as f64, (y / scale) as f64])
        }
    }

    /// # Environmental Domain Mapping (Temperature)
    ///
    /// ## 1. What Happens
    /// The `get_temperature_at` method samples the continuous noise field and maps it to a
    /// biologically relevant temperature scale (Celsius).
    ///
    /// ## 2. Why It Happens
    /// Simplex noise naturally outputs values in the domain $[-1.0, 1.0]$. For biological constraints
    /// (like thermodynamic efficiency in the `behavior` crate), we need a realistic temperature gradient
    /// ranging from freezing (Tundra) to extremely hot (Desert).
    ///
    /// ## 3. How It Happens
    /// The standard $[-1, 1]$ output is linearly mapped to $[-20^\circ C, +45^\circ C]$.
    /// Given a desired target range $[T_{min}, T_{max}]$, the transformation is:
    ///
    /// $$ Amplitude = \frac{T_{max} - T_{min}}{2.0} $$
    /// $$ Offset = T_{min} + Amplitude $$
    /// $$ T_{final} = Offset + (Noise_{val} \times Amplitude) $$
    pub fn get_temperature_at(&self, x: f32, y: f32) -> f32 {
        // Evaluate noise in [-1, 1], scale to [-20.0, 45.0] C
        let noise_val = self.eval_noise(&self.noise_temp, x, y, 400.0) as f32;
        12.5 + noise_val * 32.5
    }

    /// Returns the humidity [0.0, 1.0] at the given coordinates.
    pub fn get_humidity_at(&self, x: f32, y: f32) -> f32 {
        // Evaluate noise in [-1, 1], scale to [0.0, 1.0]
        let noise_val = self.eval_noise(&self.noise_humid, x, y, 350.0) as f32;
        (noise_val * 0.5) + 0.5
    }

    /// # Whittaker Biome Resolution
    ///
    /// ## 1. What Happens
    /// The `get_biome_at` function resolves a specific `Biome` enum variant based on the local
    /// temperature and humidity coordinates.
    ///
    /// ## 2. Why It Happens
    /// While temperature and humidity are continuous fields, ecological niches and fertility levels
    /// are often discrete states. To spawn appropriate food distributions (`EcologyConfig`) or
    /// apply localized evolutionary pressures, the engine must classify the continuum into known
    /// macroscopic biome types.
    ///
    /// ## 3. How It Happens
    /// The logic implements a simplified Robert Whittaker biome classification diagram. It evaluates
    /// a 2D piece-wise conditional tree mapping the coordinate $(T_{Celsius}, H_{Relative})$ to a
    /// discrete state space (e.g., $T > 15 \cap H > 0.7 \Rightarrow \text{TropicalRainforest}$).
    pub fn get_biome_at(&self, x: f32, y: f32) -> Biome {
        let temp = self.get_temperature_at(x, y);
        let hum = self.get_humidity_at(x, y);

        // Simple Whittaker-like biome resolution
        if temp < 0.0 {
            Biome::Tundra
        } else if temp < 15.0 {
            if hum > 0.6 {
                Biome::TemperateForest
            } else {
                Biome::Grassland
            }
        } else {
            // temp >= 15.0
            if hum > 0.7 {
                Biome::TropicalRainforest
            } else if hum > 0.3 {
                Biome::Grassland
            } else {
                Biome::Desert
            }
        }
    }
}
