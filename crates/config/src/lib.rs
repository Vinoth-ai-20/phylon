//! Configuration loading and types for Phylon.

use common::PhylonError;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Errors that can occur during configuration loading.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to load configuration: {0}")]
    Load(#[from] config::ConfigError),
}

impl PhylonError for ConfigError {}

/// The physics integrator algorithm used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhysicsIntegrator {
    VerletEuler,
    SymplecticEuler,
}

/// Simulation-specific tuning and scaling settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationConfig {
    pub tick_rate: u32,
    pub rng_seed: u64,
    pub world_chunk_size: u32,
    pub toroidal_world: bool,
    pub max_active_chunks: usize,
    pub target_organism_count: u32,
    pub diffusion_step_size: f32,
    pub physics_integrator: PhysicsIntegrator,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            tick_rate: 60,
            rng_seed: 42,
            world_chunk_size: 256,
            toroidal_world: false,
            max_active_chunks: 512,
            target_organism_count: 100_000,
            diffusion_step_size: 0.1,
            physics_integrator: PhysicsIntegrator::SymplecticEuler,
        }
    }
}

/// Rendering and visual display settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderConfig {
    pub vsync: bool,
    pub draw_debug_overlays: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            vsync: true,
            draw_debug_overlays: false,
        }
    }
}

/// Research and data export configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchConfig {
    pub snapshot_interval_ticks: u64,
    pub database_path: String,
}

impl Default for ResearchConfig {
    fn default() -> Self {
        Self {
            snapshot_interval_ticks: 3600,
            database_path: "phylon_research.db".to_string(),
        }
    }
}

/// Root configuration holding all subsystems.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PhylonConfig {
    pub simulation: SimulationConfig,
    pub render: RenderConfig,
    pub research: ResearchConfig,
}

impl PhylonConfig {
    /// Load configuration from a file or fallback to defaults.
    pub fn load(path: Option<&Path>) -> Result<Self, ConfigError> {
        let mut builder =
            config::Config::builder().add_source(config::Config::try_from(&Self::default())?);

        if let Some(p) = path {
            builder = builder.add_source(config::File::from(p).required(false));
        }

        let cfg = builder.build()?;
        let res: PhylonConfig = cfg.try_deserialize()?;
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_loads() {
        let cfg = PhylonConfig::load(None).unwrap();
        assert_eq!(cfg.simulation.tick_rate, 60);
    }
}
