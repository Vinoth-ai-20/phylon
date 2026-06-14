//! Metabolism logic for Phylon organisms.

use events::{DeathCause, EventBus, PhylonEvent};
use genetics::Genome;
use hecs::World;
use organisms::{Age, Energy, Health, Organism};
use physics::{Mass, Velocity};

/// Processes metabolism for all organisms.
/// Drains energy based on basal metabolic rate, velocity, and mass.
/// Increases age.
/// Drains health if energy is 0.
/// Fires DeathEvent if health reaches 0 or max age is exceeded.
pub fn process_metabolism(world: &mut World, events: &EventBus) {
    puffin::profile_function!();

    let mut deaths = Vec::new();

    for (entity, (_, energy, health, age, genome, mass, vel)) in world.query_mut::<(
        &Organism,
        &mut Energy,
        &mut Health,
        &mut Age,
        &Genome,
        &Mass,
        &Velocity,
    )>() {
        age.0 += 1;

        // Base metabolic cost + kinetic cost
        let speed_sq = vel.0.length_squared();
        let kinetic_cost = 0.5 * mass.0 * speed_sq * 0.0001; // tiny multiplier
        let sensory_cost = 0.0005 * genome.sense_radius; // cost for sensing
        let basal_cost = 0.05 * genome.metabolic_rate + sensory_cost;
        let total_cost = basal_cost + kinetic_cost;

        energy.0 -= total_cost;

        if energy.0 <= 0.0 {
            energy.0 = 0.0;
            // Starving
            health.current -= 1.0;
        } else if energy.0 > 20.0 {
            // Healing
            health.current = (health.current + 0.1).min(health.max);
        }

        let mut cause = None;
        if health.current <= 0.0 {
            cause = Some(DeathCause::Starvation);
        } else if age.0 > 10000 {
            // Hardcoded max age for now
            cause = Some(DeathCause::OldAge);
        }

        if let Some(reason) = cause {
            deaths.push((entity, reason));
        }
    }

    for (entity, reason) in deaths {
        events.publish(PhylonEvent::DeathEvent {
            id: common::EntityId(entity.to_bits().get()),
            reason,
        });
    }
}
