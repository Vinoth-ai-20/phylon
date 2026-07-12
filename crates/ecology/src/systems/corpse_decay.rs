use crate::components::{Corpse, MineralPellet};
use bevy_ecs::prelude::*;

/// # Corpse Decomposition & Outgassing System
///
/// ## 1. What Happens
/// The `corpse_decay_system` manages the biological decay of organisms that have died. When a
/// `Corpse` decays, it steadily outgasses $CO_2$ back into the `GlobalAtmosphere` over a set
/// duration. Once fully decayed, it despawns and leaves behind a `MineralPellet`.
///
/// ## 2. Why It Happens
/// This resolves the "Carbon Leak" tragedy-of-the-commons crisis. If organisms consume $CO_2$
/// to grow but delete that mass from the simulation upon death, the atmosphere would eventually
/// run out of carbon, halting all photosynthetic life. The outgassing models the gradual
/// respiration of invisible decomposer microbes breaking down structural carbon.
///
/// ## 3. How It Happens
/// Each tick, the system iterates over all entities with a `Corpse` component. The decay timer
/// is decremented, and the atmospheric outgassing accumulation is calculated per tick as:
///
/// $$ \Delta CO_{2} = \text{corpse.energy\_value} \times 0.0001 $$
///
/// Upon timer exhaustion ($t = 0$), the corpse undergoes complete mineralization. 50% of
/// the remaining energy is spawned as a `MineralPellet`, a 10% $\Delta CO_2$ burst is released,
/// and the `Corpse` entity is safely despawned.
pub fn corpse_decay_system(
    mut commands: Commands,
    mut atmosphere: ResMut<metabolism::GlobalAtmosphere>,
    mut corpse_query: Query<(Entity, &mut Corpse)>,
    mut timed_effects: ResMut<events::TimedEffects>,
) {
    // Not biologically tuned — a placeholder duration shared with the
    // other short-lived floating-text effect durations in this crate.
    const DECOMPOSITION_EFFECT_DURATION_TICKS: u64 = 90;

    for (entity, mut corpse) in corpse_query.iter_mut() {
        if corpse.decay_timer > 0 {
            corpse.decay_timer -= 1;
            // Slowly release CO2 back into the atmosphere as the corpse
            // decays, rather than all at once on death — see this system's
            // "Why It Happens" above for the carbon-leak problem this avoids.
            atmosphere.co2 += corpse.energy_value * 0.0001;
        } else {
            // Decay into mineral
            commands.spawn(MineralPellet {
                position: corpse.position,
                energy_value: corpse.energy_value * 0.5, // 50% energy lost to environment if not eaten directly
            });
            // Final burst of CO2 upon complete decay
            atmosphere.co2 += corpse.energy_value * 0.1;

            timed_effects.spawn(
                corpse.position,
                events::TimedEffectKind::FloatingText {
                    text: "Decomposed".to_string(),
                    color: [0.5, 0.4, 0.3],
                },
                atmosphere.ticks,
                DECOMPOSITION_EFFECT_DURATION_TICKS,
            );

            if let Some(mut e) = commands.get_entity(entity) {
                e.despawn();
            }
        }
    }
}
