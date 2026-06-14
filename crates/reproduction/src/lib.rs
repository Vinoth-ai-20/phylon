//! Reproduction logic for Phylon organisms.

use common::Vec2;
use events::{EventBus, PhylonEvent};
use genetics::Genome;
use hecs::World;
use organisms::{Energy, Organism};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Component indicating an organism cannot reproduce until the cooldown reaches 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReproductionCooldown(pub u32);

/// Processes asexual reproduction.
/// Iterates over organisms. If Energy > 100.0 and cooldown is 0, split energy, set cooldown, and fire BirthEvent.
pub fn process_reproduction(world: &mut World, events: &EventBus, rng_seed: u64, tick: u64) {
    let mut rng = ChaCha8Rng::seed_from_u64(rng_seed.wrapping_add(tick));

    for (entity, (_, energy, genome, cooldown, pos)) in world.query_mut::<(
        &Organism,
        &mut Energy,
        &Genome,
        &mut ReproductionCooldown,
        &physics::Position,
    )>() {
        if cooldown.0 > 0 {
            cooldown.0 -= 1;
            continue;
        }

        // Hardcoded threshold for now, could be dynamic or based on genome
        let threshold = 150.0;
        if energy.0 >= threshold {
            // Halve energy
            energy.0 /= 2.0;
            cooldown.0 = 100; // Cooldown ticks

            // Mutate genome
            let child_genome = genome.mutate(&mut rng, 0.1);

            // Spawn slightly offset
            let offset = Vec2::new(rng.gen_range(-5.0..5.0), rng.gen_range(-5.0..5.0));

            events.publish(PhylonEvent::BirthEvent {
                parent: Some(common::EntityId(entity.to_bits().get())),
                genome: child_genome,
                initial_energy: energy.0,
                position: pos.0 + offset,
            });
        }
    }
}
