use bevy_ecs::prelude::*;
use common::{Tick, Vec2};

/// Event triggered when a hazard is spawned.
#[derive(Event, Debug, Clone)]
pub struct HazardSpawned(pub Vec2);

/// Configuration for random hazards.
#[derive(Resource, Debug, Clone)]
pub struct CatastropheConfig {
    /// Probability per tick to spawn a new hazard.
    pub spawn_probability: f32,
    /// Hazard radius.
    pub hazard_radius: f32,
    /// Ticks the hazard stays in the "impending" state.
    pub impending_duration: u32,
    /// Ticks the hazard stays in the "active" state.
    pub active_duration: u32,
    /// Energy drain per tick when an organism is inside an active hazard.
    pub energy_drain_rate: f32,
}

impl Default for CatastropheConfig {
    fn default() -> Self {
        Self {
            spawn_probability: 0.0005, // 1 in 2000 ticks
            hazard_radius: 150.0,
            impending_duration: 300, // 5 seconds at 60Hz
            active_duration: 600,    // 10 seconds at 60Hz
            energy_drain_rate: 2.0,  // high drain
        }
    }
}

impl CatastropheConfig {
    /// Scales hazard frequency/severity by `level` (`1.0` = default
    /// difficulty) — the curriculum-learning hook
    /// `network::MarlCommand::SetDifficulty` drives. Always recomputed from
    /// [`CatastropheConfig::default`]'s baseline, never multiplied against
    /// the current live value, so repeated calls with the same `level` are
    /// idempotent instead of compounding — calling `set_difficulty(1.5)`
    /// twice in a row must not produce a harder difficulty than calling it
    /// once.
    pub fn set_difficulty(&mut self, level: f32) {
        let baseline = Self::default();
        let level = level.max(0.0);
        self.spawn_probability = baseline.spawn_probability * level;
        self.energy_drain_rate = baseline.energy_drain_rate * level;
    }
}

/// State of a specific hazard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HazardState {
    /// Hazard is warming up. Organisms can sense it but it doesn't drain energy yet.
    Impending {
        /// Tick when it started.
        start_tick: Tick,
    },
    /// Hazard is active and draining energy.
    Active {
        /// Tick when it became active.
        start_tick: Tick,
    },
}

/// Represents an ongoing localized catastrophe.
#[derive(Debug, Clone)]
pub struct LocalHazard {
    /// Center position of the hazard.
    pub center: Vec2,
    /// The current state of the hazard.
    pub state: HazardState,
}

/// Manager resource tracking all active hazards.
#[derive(Resource, Default, Debug)]
pub struct CatastropheManager {
    /// List of ongoing hazards.
    pub hazards: Vec<LocalHazard>,
}

impl CatastropheManager {
    /// Spawns a new hazard at the given location.
    pub fn spawn_hazard(&mut self, current_tick: Tick, center: Vec2) {
        self.hazards.push(LocalHazard {
            center,
            state: HazardState::Impending {
                start_tick: current_tick,
            },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_difficulty_one_matches_default() {
        let mut config = CatastropheConfig {
            spawn_probability: 999.0,
            energy_drain_rate: 999.0,
            ..CatastropheConfig::default()
        };
        config.set_difficulty(1.0);
        let baseline = CatastropheConfig::default();
        assert_eq!(config.spawn_probability, baseline.spawn_probability);
        assert_eq!(config.energy_drain_rate, baseline.energy_drain_rate);
    }

    #[test]
    fn set_difficulty_scales_proportionally() {
        let mut config = CatastropheConfig::default();
        config.set_difficulty(2.0);
        let baseline = CatastropheConfig::default();
        assert_eq!(config.spawn_probability, baseline.spawn_probability * 2.0);
        assert_eq!(config.energy_drain_rate, baseline.energy_drain_rate * 2.0);
    }

    #[test]
    fn set_difficulty_is_idempotent_not_compounding() {
        let mut config = CatastropheConfig::default();
        config.set_difficulty(1.5);
        let after_first = config.spawn_probability;
        config.set_difficulty(1.5);
        assert_eq!(config.spawn_probability, after_first);
    }

    #[test]
    fn set_difficulty_clamps_negative_to_zero() {
        let mut config = CatastropheConfig::default();
        config.set_difficulty(-5.0);
        assert_eq!(config.spawn_probability, 0.0);
        assert_eq!(config.energy_drain_rate, 0.0);
    }
}
