//! Energy management, ageing, respiration, starvation, and hunger systems.

#![warn(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::prelude::*;

/// Tracks the energy level of an organism.
#[derive(Component, Debug, Clone)]
pub struct Energy {
    /// Current energy level.
    pub current: f32,
    /// Maximum energy capacity.
    pub max: f32,
}

/// Tracks the age of an organism.
#[derive(Component, Debug, Clone)]
pub struct Age {
    /// Number of ticks lived.
    pub ticks: u64,
    /// Maximum lifespan in ticks before senescence.
    pub max_lifespan: u64,
}

/// Defines the baseline metabolic cost per tick.
#[derive(Component, Debug, Clone)]
pub struct Metabolism {
    /// The abstract mass of the organism (sum of its nodes).
    pub mass: f32,
    /// The base cost multiplier per tick.
    pub base_rate: f32,
}

/// System that deducts energy per tick based on mass and handles aging.
///
/// Note: Starvation and old age deaths emit a `DeathEvent` (handled in `app`).
/// For simplicity in this phase, we just flag them by despawning or marking them dead
/// directly via commands if we don't have access to the EventBus here.
/// Actually, to use `EventBus`, we would need it as a Resource. Since `events::EventBus`
/// isn't a Bevy Resource yet, we can either insert it as a Resource or just despawn
/// directly here. For Phylon architecture, let's use `commands.entity(entity).despawn_recursive()`
/// and we can publish the event by storing an `EventBus` resource.
pub fn metabolism_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Energy, &mut Age, &Metabolism)>,
) {
    for (entity, mut energy, mut age, metabolism) in query.iter_mut() {
        // Increment age
        age.ticks += 1;

        // Deduct energy: superlinear scaling mass^1.2
        let cost = metabolism.base_rate * metabolism.mass.powf(1.2);
        energy.current -= cost;

        // Check starvation
        if energy.current <= 0.0 {
            // Emitting event would be ideal, but despawning is the immediate ECS action.
            // In a full implementation, we push `events::DeathCause::Starvation` to the bus.
            commands.entity(entity).despawn();
            continue;
        }

        // Check old age
        if age.ticks >= age.max_lifespan {
            commands.entity(entity).despawn();
            continue;
        }
    }
}
