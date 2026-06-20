use bevy_ecs::system::Resource;
use noise::{NoiseFn, OpenSimplex};
use serde::{Deserialize, Serialize};

/// Biome classification for a coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Biome {
    TropicalRainforest,
    TemperateForest,
    Desert,
    Tundra,
    Grassland,
    Freshwater,
    CoastalMarine,
    DeepOcean,
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

    /// Evaluates a 2D tileable/continuous noise function using 4D mapping if toroidal.
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

    /// Returns the temperature in Celsius at the given coordinates.
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

    /// Resolves the Biome at a specific coordinate based on temperature and humidity.
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
