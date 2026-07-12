//! # Phylon Config
//!
//! Configuration loading, validation, and default values for the Phylon simulation.
//!
//! Configuration is stored in [RON](https://github.com/ron-rs/ron) (Rusty Object Notation)
//! files under the `data/` directory. The canonical entry point is
//! [`PhylonConfig::load`], which parses the file at the given path and falls
//! back to the compile-time defaults if the file is absent.
//!
//! ## Hierarchy
//!
//! ```text
//! PhylonConfig
//! ├── SimulationConfig  — tick rate, RNG seed, world topology
//! ├── RenderConfig      — window dimensions, vsync, overlay opacity
//! └── ResearchConfig    — headless/batch mode, dataset export, experiment ID
//! ```
//!
//! ## Usage
//!
//! ```rust,no_run
//! use config::PhylonConfig;
//! use std::path::Path;
//!
//! let cfg = PhylonConfig::load(Some(Path::new("data/default.ron")))
//!     .expect("Failed to load config");
//! println!("tick rate: {}", cfg.simulation.tick_rate);
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ────────────────────────────────────────────────────────────────────────────
// Error type
// ────────────────────────────────────────────────────────────────────────────

/// All errors that can occur while loading or validating [`PhylonConfig`].
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The config file could not be read from disk.
    #[error("failed to read config file '{path}': {source}")]
    IoError {
        /// Path that was attempted.
        path: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The config file was read but could not be parsed as valid RON.
    #[error("failed to parse config file '{path}': {source}")]
    ParseError {
        /// Path that was attempted.
        path: String,
        /// Underlying RON parse error.
        #[source]
        source: ron::error::SpannedError,
    },

    /// A configuration value failed validation constraints.
    #[error("invalid configuration value: {message}")]
    ValidationError {
        /// Human-readable description of the violated constraint.
        message: String,
    },
}

impl common::PhylonError for ConfigError {}

// ────────────────────────────────────────────────────────────────────────────
// Physics integrator enum
// ────────────────────────────────────────────────────────────────────────────

/// The numerical integration method used by the physics subsystem.
///
/// Both methods are explicit and first-order accurate, but Symplectic Euler
/// conserves energy better for oscillatory systems (spring networks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PhysicsIntegrator {
    /// Semi-implicit (symplectic) Euler: velocity updated first, then position.
    /// Recommended for soft-body spring networks — better energy conservation.
    #[default]
    SymplecticEuler,

    /// Velocity Verlet integration. Slightly more accurate than symplectic Euler
    /// for conservative forces at the cost of an extra force evaluation per tick.
    VelocityVerlet,
}

// ────────────────────────────────────────────────────────────────────────────
// SimulationConfig
// ────────────────────────────────────────────────────────────────────────────

/// # Core Simulation Configuration
///
/// ## 1. What Happens
/// `SimulationConfig` stores parameters that dictate the deterministic tick rate,
/// spatial topologies, and numerical bounds of the physics and diffusion systems.
///
/// ## 2. Why It Happens
/// Hardcoded "magic numbers" in engine code are brittle and prevent batch testing.
/// Extracting these to a loaded config allows researchers to run scripts that sweep
/// `tick_rate` or `world_chunk_size` without recompiling the Rust binary.
///
/// ## 3. How It Happens
/// Parsed from a `.ron` file and validated during startup. It is passed as a shared
/// reference to the ECS `World` or `SimulationScheduler`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationConfig {
    /// Target simulation ticks per second. Default: `60`.
    ///
    /// Changing this value between experiment runs breaks reproducibility.
    /// Always record this value in the experiment manifest.
    pub tick_rate: u32,

    /// The master RNG seed for this experiment.
    ///
    /// Every stochastic decision in the simulation derives from `ChaCha8Rng`
    /// seeded with this value. Recording this seed is sufficient to reproduce
    /// the entire CPU-authoritative simulation trajectory.
    pub rng_seed: u64,

    /// Width and height of each world chunk in simulation length units.
    /// Default: `256`.
    pub world_chunk_size: u32,

    /// If `true`, the world wraps at its boundaries (toroidal topology).
    /// Default: `false` (infinite expanding world).
    pub toroidal_world: bool,

    /// Maximum number of simultaneously active chunks. Default: `512`.
    pub max_active_chunks: usize,

    /// Desired steady-state organism count. The ecology system uses this
    /// target to calibrate resource availability. Default: `1_000`.
    pub target_organism_count: u32,

    /// Diffusion field update step size (fractional ticks). Default: `1.0`.
    ///
    /// Must satisfy the stability condition: `step_size ≤ 0.25 / diffusion_rate`.
    pub diffusion_step_size: f32,

    /// Physics integrator selection. Default: [`PhysicsIntegrator::SymplecticEuler`].
    pub physics_integrator: PhysicsIntegrator,

    /// Energy cost per unit of signal emission amplitude.
    #[serde(default = "default_signal_energy_cost")]
    pub signal_energy_cost_per_unit: f32,
}

fn default_signal_energy_cost() -> f32 {
    0.01
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            tick_rate: 60,
            rng_seed: 0xDEAD_BEEF_CAFE_BABE,
            world_chunk_size: 256,
            toroidal_world: false,
            max_active_chunks: 512,
            target_organism_count: 1_000,
            diffusion_step_size: 1.0,
            physics_integrator: PhysicsIntegrator::default(),
            signal_energy_cost_per_unit: default_signal_energy_cost(),
        }
    }
}

impl SimulationConfig {
    /// Validates that all fields satisfy their invariants.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::ValidationError`] if any field is out of range.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.tick_rate == 0 {
            return Err(ConfigError::ValidationError {
                message: "tick_rate must be > 0".into(),
            });
        }
        if self.world_chunk_size == 0 {
            return Err(ConfigError::ValidationError {
                message: "world_chunk_size must be > 0".into(),
            });
        }
        if self.max_active_chunks == 0 {
            return Err(ConfigError::ValidationError {
                message: "max_active_chunks must be > 0".into(),
            });
        }
        if !(0.0..=1.0).contains(&self.diffusion_step_size) {
            return Err(ConfigError::ValidationError {
                message: format!(
                    "diffusion_step_size {} is outside valid range [0.0, 1.0]",
                    self.diffusion_step_size
                ),
            });
        }
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────────────
// RenderConfig
// ────────────────────────────────────────────────────────────────────────────

/// # Renderer Configuration
///
/// ## 1. What Happens
/// `RenderConfig` dictates the physical window dimensions, frame synchronisation,
/// and visual overlay settings for the wgpu presentation layer.
///
/// ## 2. Why It Happens
/// Allows end-users to customize their viewing experience (e.g., turning off vsync
/// to unlock frame rates, or adjusting UI opacities for better visibility of
/// underlying structures).
///
/// ## 3. How It Happens
/// This config configures the winit `WindowBuilder` before the graphics context
/// is instantiated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderConfig {
    /// Initial window width in physical pixels. Default: `1280`.
    pub window_width: u32,

    /// Initial window height in physical pixels. Default: `720`.
    pub window_height: u32,

    /// Window title string. Default: `"Phylon"`.
    pub window_title: String,

    /// Enable vertical synchronisation. Default: `true`.
    pub vsync: bool,

    /// Opacity of field overlay layers `[0.0, 1.0]`. Default: `0.5`.
    pub overlay_opacity: f32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            window_width: 1280,
            window_height: 720,
            window_title: "Phylon".into(),
            vsync: true,
            overlay_opacity: 0.5,
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// ResearchConfig
// ────────────────────────────────────────────────────────────────────────────

/// # Research & Automation Configuration
///
/// ## 1. What Happens
/// `ResearchConfig` controls headless operation, experiment metadata, and
/// optional network connections for MARL.
///
/// ## 2. Why It Happens
/// A typical desktop game expects a human sitting at a screen. Phylon is an Alife
/// research tool—sometimes it needs to run on a Linux cluster for 48 hours with
/// no GPU, spitting out data to SQLite. This config enables those environments.
///
/// Note: autosave is currently manual-only (triggered via
/// `MenuAction::SaveState`) — there is no `autosave_interval_ticks` field or
/// periodic-autosave system. Adding one is real feature work (a save-path
/// and rotation policy, error handling, a tick-loop hook) rather than a
/// config knob, so it is intentionally left out until a concrete need for
/// periodic autosave arises, rather than exposing a field that silently does
/// nothing.
///
/// ## 3. How It Happens
/// The `app` crate checks `headless`. If true, it skips initializing `winit` and
/// `wgpu`, locking into a fast-forward tight loop that only sleeps if `realtime_lock`
/// is enabled.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchConfig {
    /// Human-readable experiment identifier. Recorded in the manifest.
    pub experiment_id: String,

    /// If `true`, the simulation runs without a window (headless mode).
    /// Default: `false`.
    pub headless: bool,

    /// If `true` while in `headless` mode, the simulation speed is capped to match
    /// the real-time `tick_rate`. If `false`, it runs as fast as possible (uncapped).
    /// Default: `false`.
    pub realtime_lock: bool,

    /// Maximum number of ticks to simulate before halting (0 = unlimited).
    pub max_ticks: u64,

    /// Optional port for the headless MARL WebSocket server. If `Some`, the
    /// server is started and the simulation acts as an RL environment.
    pub network_port: Option<u16>,

    /// When non-empty, `main` runs one headless experiment per seed listed
    /// here (via `app::batch::run_batch`) instead of the normal
    /// single-run/windowed flow, writing a per-run report plus one
    /// aggregate batch summary to `data/experiments/`. Default: empty (off).
    ///
    /// `#[serde(default)]` so `.ron` config files written before this field
    /// existed keep loading instead of failing with a missing-field error.
    #[serde(default)]
    pub batch_seeds: Vec<u64>,

    /// When `Some`, `main` loads the `.phylon-replay` bundle at this path
    /// and plays it back (via `app::replay::run_replay`) instead of the
    /// normal single-run/windowed/batch flow. Default: `None` (off).
    #[serde(default)]
    pub replay_path: Option<String>,

    /// Playback speed multiplier used when `replay_path` is set and
    /// `replay_realtime_pacing` is `true`. Default: `1.0`.
    #[serde(default = "default_replay_speed")]
    pub replay_speed: f32,

    /// When `true`, replay playback sleeps between ticks to match
    /// `replay_speed`× real time; when `false` (default), replay runs ticks
    /// back to back as fast as possible, matching headless/batch mode.
    #[serde(default)]
    pub replay_realtime_pacing: bool,

    /// When `Some`, a `.rhai` scenario script (see `plugins::PluginEngine`)
    /// runs once right after startup, in every run mode — letting a
    /// researcher author initial conditions without recompiling. Default:
    /// `None` (off).
    #[serde(default)]
    pub scenario_path: Option<String>,

    /// When `Some` and running headless, a `.rhai` script runs every
    /// `periodic_script_interval_ticks` ticks — scripted mid-run
    /// interventions (e.g. "release a hazard every 10,000 ticks") without
    /// recompiling. Default: `None` (off).
    #[serde(default)]
    pub periodic_script_path: Option<String>,

    /// How often (in ticks) `periodic_script_path` runs. Default: `600`
    /// (~10s at 60 Hz).
    #[serde(default = "default_periodic_script_interval")]
    pub periodic_script_interval_ticks: u64,
}

fn default_periodic_script_interval() -> u64 {
    600
}

fn default_replay_speed() -> f32 {
    1.0
}

impl Default for ResearchConfig {
    fn default() -> Self {
        Self {
            experiment_id: "default-experiment".into(),
            headless: false,
            realtime_lock: false,
            max_ticks: 0,
            network_port: None,
            batch_seeds: Vec::new(),
            replay_path: None,
            replay_speed: default_replay_speed(),
            replay_realtime_pacing: false,
            scenario_path: None,
            periodic_script_path: None,
            periodic_script_interval_ticks: default_periodic_script_interval(),
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// PhylonConfig (root)
// ────────────────────────────────────────────────────────────────────────────

/// # Root Runtime Configuration
///
/// ## 1. What Happens
/// `PhylonConfig` is the root container struct parsing the RON file and storing
/// the parsed nested config modules (Simulation, Render, Research).
///
/// ## 2. Why It Happens
/// Centralizing all configuration into a single struct guarantees that any subsystem
/// has an unambiguous source of truth for its parameters.
///
/// ## 3. How It Happens
/// The `load()` method deserializes a text string into this struct using `ron`.
/// It then calls validation routines to ensure constraints (e.g., tick_rate > 0).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PhylonConfig {
    /// Core simulation parameters (tick rate, RNG seed, world topology).
    pub simulation: SimulationConfig,

    /// Windowing and rendering parameters.
    pub render: RenderConfig,

    /// Research and data-collection parameters.
    pub research: ResearchConfig,
}

impl PhylonConfig {
    /// Loads the configuration from a `.ron` file.
    ///
    /// If `path` is `None` or the file does not exist, the compile-time
    /// [`Default`] values are returned instead. If the file exists but cannot
    /// be parsed or fails validation, an error is returned.
    ///
    /// # Errors
    ///
    /// - [`ConfigError::IoError`] — file exists but cannot be read.
    /// - [`ConfigError::ParseError`] — file contains invalid RON.
    /// - [`ConfigError::ValidationError`] — parsed config violates constraints.
    pub fn load(path: Option<&Path>) -> Result<Self, ConfigError> {
        let Some(p) = path else {
            return Ok(Self::default());
        };

        if !p.exists() {
            return Ok(Self::default());
        }

        let text = std::fs::read_to_string(p).map_err(|e| ConfigError::IoError {
            path: p.to_string_lossy().into_owned(),
            source: e,
        })?;

        let cfg: Self = ron::from_str(&text).map_err(|e| ConfigError::ParseError {
            path: p.to_string_lossy().into_owned(),
            source: e,
        })?;

        cfg.simulation.validate()?;

        Ok(cfg)
    }

    /// Returns the fixed-timestep duration for one simulation tick.
    ///
    /// Convenience method so callers do not have to perform the division
    /// themselves, avoiding repeated magic-number arithmetic.
    #[inline]
    pub fn tick_duration(&self) -> std::time::Duration {
        std::time::Duration::from_secs_f64(1.0 / f64::from(self.simulation.tick_rate))
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let cfg = PhylonConfig::default();
        cfg.simulation
            .validate()
            .expect("default config must be valid");
    }

    #[test]
    fn tick_duration_60hz() {
        let cfg = PhylonConfig::default();
        let dur = cfg.tick_duration();
        // 1/60 second ≈ 16.666 ms
        let expected_ms = 1000.0 / 60.0;
        let actual_ms = dur.as_secs_f64() * 1000.0;
        assert!((actual_ms - expected_ms).abs() < 0.01, "got {actual_ms} ms");
    }

    #[test]
    fn zero_tick_rate_fails_validation() {
        let cfg = SimulationConfig {
            tick_rate: 0,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn invalid_diffusion_step_fails_validation() {
        let cfg = SimulationConfig {
            diffusion_step_size: 1.5,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn load_absent_path_returns_default() {
        let cfg = PhylonConfig::load(Some(Path::new("does_not_exist.ron")))
            .expect("absent file should return default");
        assert_eq!(cfg.simulation.tick_rate, 60);
    }

    #[test]
    fn load_none_returns_default() {
        let cfg = PhylonConfig::load(None).expect("None path should return default");
        assert_eq!(cfg.simulation.tick_rate, 60);
    }

    /// Proves the real, checked-in `data/default.ron` file parses cleanly
    /// with the current `PhylonConfig` shape — not just the in-memory
    /// `Default` impl the other tests above exercise. A schema change that
    /// forgets to update the checked-in file (or introduces a required
    /// field with no `#[serde(default)]`) fails here even if every other
    /// test passes.
    #[test]
    fn real_default_ron_file_still_loads() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/default.ron");
        let cfg = PhylonConfig::load(Some(&path)).expect("data/default.ron should parse");
        assert_eq!(cfg.research.experiment_id, "default");
    }
}
