//! Reproduction strategies, birth events, offspring dispersal, and malformed offspring handling.

#![warn(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::prelude::*;
use common::Vec2;
use genetics::Genome;
use metabolism::Energy;

/// Reproduction mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReproductionMode {
    /// Clones itself.
    Asexual,
    /// Requires a mate.
    Sexual,
}

/// Component defining an organism's reproduction logic.
#[derive(Component, Debug, Clone)]
pub struct ReproductionStrategy {
    /// Energy required to trigger reproduction.
    pub energy_threshold: f32,
    /// Energy deducted from parent when reproducing.
    pub energy_cost: f32,
    /// Minimum ticks between reproduction events.
    pub cooldown_ticks: u64,
    /// Ticks since last reproduction.
    pub current_cooldown: u64,
    /// Mode of reproduction.
    pub mode: ReproductionMode,
    /// The genome of this organism (to pass to offspring).
    pub genome: Genome,
}

/// Event triggered when an organism successfully reproduces.
#[derive(Event, Debug, Clone)]
pub struct BirthRequest {
    /// The genome for the new child.
    pub genome: Genome,
    /// The position to spawn the child.
    pub position: Vec2,
}

/// System that handles asexual cloning.
pub fn reproduction_system(
    mut query: Query<(
        &mut Energy,
        &mut ReproductionStrategy,
        &physics::ParticleNode,
    )>,
    config: Res<ecology::EcologyConfig>,
    all_organisms: Query<(), With<ReproductionStrategy>>,
    mut birth_events: EventWriter<BirthRequest>,
) {
    let current_population = all_organisms.iter().count();
    let mut pending_births = 0;

    for (mut energy, mut strategy, node) in query.iter_mut() {
        if strategy.current_cooldown > 0 {
            strategy.current_cooldown -= 1;
            continue;
        }

        if strategy.mode == ReproductionMode::Asexual && energy.current >= strategy.energy_threshold
        {
            // Check hard cap
            if current_population + pending_births >= config.max_organisms {
                continue; // Can't reproduce due to cap
            }

            // Pay the cost
            energy.current -= strategy.energy_cost;
            strategy.current_cooldown = strategy.cooldown_ticks;

            // Offset the child's position slightly so they don't exactly overlap
            let mut offset_pos = node.position;
            offset_pos.x += (fastrand::f32() - 0.5) * 100.0;
            offset_pos.y += (fastrand::f32() - 0.5) * 100.0;

            // Emit birth request
            birth_events.send(BirthRequest {
                genome: strategy.genome.clone(),
                position: offset_pos,
            });

            pending_births += 1;
        }
    }
}
