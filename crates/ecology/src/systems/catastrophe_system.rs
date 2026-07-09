use crate::components::Corpse;
use bevy_ecs::prelude::*;
use common::Vec2;

/// System that manages catastrophes, updates the hazard field, and drains energy from organisms in active hazards.
///
/// Phase 6, Epic A (re-audit finding, folded into this milestone rather than
/// deferred): this system previously used `Local<u64>` for `local_tick`, the
/// same anti-pattern SX-1a's diagnostic already named and fixed elsewhere —
/// the live app drives every system via `run_system_once` (a fresh
/// `SystemState` per call), so a `Local<u64>` silently reset to `0` on every
/// single tick, meaning `tick` here was **always `Tick(1)`**. Since hazard
/// lifecycle transitions are computed as `elapsed = tick - start_tick`, and
/// both sides of that subtraction were always `Tick(1)`, `elapsed` was
/// always `0` — hazards spawned into `Impending` state and then **never
/// transitioned to `Active` and never expired**, regardless of
/// `impending_duration`/`active_duration`. Fixed by reading
/// `metabolism::GlobalAtmosphere::ticks` (already the canonical live tick
/// counter this exact bug class was fixed with at SX-7a), which
/// `metabolism::day_night_cycle_system` increments earlier in the same
/// tick's system order (confirmed via `crates/app/src/simulation.rs`) — so
/// no new resource was introduced, just reuse of the one that already
/// exists.
#[allow(clippy::too_many_arguments)]
pub fn catastrophe_system(
    mut manager: ResMut<crate::catastrophe::CatastropheManager>,
    config: Res<crate::catastrophe::CatastropheConfig>,
    mut hazard_field: ResMut<diffusion::CpuHazardFieldState>,
    env: Res<environment::EnvironmentManager>,
    atmosphere: Res<metabolism::GlobalAtmosphere>,
    mut rng: ResMut<common::SimRng>,
    mut hazard_events: EventWriter<crate::catastrophe::HazardSpawned>,
    mut organisms: Query<(
        &mut metabolism::ChemicalEconomy,
        &physics::ParticleNode,
        Option<&mut Corpse>,
    )>,
) {
    use rand::Rng;

    let tick = common::Tick(atmosphere.ticks);

    // Spawn random hazards
    if rng.gen::<f32>() < config.spawn_probability {
        let x = (rng.gen::<f32>() - 0.5) * env.width();
        let y = (rng.gen::<f32>() - 0.5) * env.height();
        manager.spawn_hazard(tick, Vec2::new(x, y));
        hazard_events.send(crate::catastrophe::HazardSpawned(Vec2::new(x, y)));
    }

    hazard_field.clear();

    let mut active_hazards = Vec::new();

    // Update hazards and splat to field
    manager.hazards.retain_mut(|hazard| {
        match hazard.state {
            crate::catastrophe::HazardState::Impending { start_tick } => {
                let elapsed = tick.0.saturating_sub(start_tick.0);
                if elapsed >= config.impending_duration as u64 {
                    hazard.state = crate::catastrophe::HazardState::Active { start_tick: tick };
                    // splat with active severity
                    hazard_field.splat(hazard.center, config.hazard_radius, 1.0);
                    active_hazards.push((hazard.center, config.hazard_radius));
                } else {
                    // Splat impending severity (grows over time)
                    let severity = elapsed as f32 / config.impending_duration as f32;
                    hazard_field.splat(hazard.center, config.hazard_radius, severity);
                }
                true
            }
            crate::catastrophe::HazardState::Active { start_tick } => {
                let elapsed = tick.0.saturating_sub(start_tick.0);
                if elapsed >= config.active_duration as u64 {
                    false // Remove hazard
                } else {
                    hazard_field.splat(hazard.center, config.hazard_radius, 1.0);
                    active_hazards.push((hazard.center, config.hazard_radius));
                    true
                }
            }
        }
    });

    // Apply energy drain to organisms in active hazards
    for (mut chem, node, mut corpse_opt) in organisms.iter_mut() {
        let mut in_hazard = false;
        for (center, radius) in &active_hazards {
            if node.position.distance(*center) <= *radius {
                in_hazard = true;
                break;
            }
        }

        if in_hazard {
            chem.atp = (chem.atp - config.energy_drain_rate).max(0.0);

            // If they died from catastrophe, maybe accelerate decay if they are already a corpse
            if let Some(corpse) = corpse_opt.as_mut() {
                corpse.energy_value = (corpse.energy_value - config.energy_drain_rate).max(0.0);
            }
        }
    }
}
