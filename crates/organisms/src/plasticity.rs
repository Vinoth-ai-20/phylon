use bevy_ecs::prelude::{Query, Res};

/// # Neuromodulator Update System
///
/// ## 1. What Happens
/// Updates every organism's [`brain::Neuromodulators`] channels from its
/// current [`metabolism::ChemicalEconomy`] reading.
///
/// ## 2. Why It Happens
/// Dopamine/serotonin/noradrenaline need a real physiological signal to
/// track — ATP level and its recent trend stand in for reward and stress
/// without inventing a separate emotion system. This keeps neuromodulators
/// grounded in state the simulation already tracks.
///
/// ## 3. How It Happens
/// Runs once per tick, before [`hebbian_plasticity_system`], so this tick's
/// dopamine reading is available to gate this tick's Hebbian update.
pub fn neuromodulator_system(
    mut query: Query<(&metabolism::ChemicalEconomy, &mut brain::Neuromodulators)>,
) {
    for (chem, mut neuro) in query.iter_mut() {
        neuro.update(chem.atp, chem.max_atp);
    }
}

/// # Hebbian Plasticity & Pruning System
///
/// ## 1. What Happens
/// Applies one Hebbian weight-update step to every plastic organism's brain
/// (see [`brain::Brain::apply_hebbian_update`]), with the base rate scaled
/// by that organism's [`brain::Neuromodulators::dopamine`]. Every
/// `prune_interval_ticks` (gated on [`metabolism::GlobalAtmosphere::ticks`],
/// the simulation's global tick counter), it also prunes synapses that have
/// decayed below `prune_threshold` (see [`brain::Brain::prune_weak_synapses`]).
///
/// ## 2. Why It Happens
/// Intra-lifetime learning needs to run *after* this tick's CTRNN node
/// states are back from the GPU (see `crates/app/src/simulation.rs`'s
/// `resolve_pending_brain`), since the Hebbian rule reads node activity as
/// its pre/post-synaptic signal. Pruning runs on a slow cadence rather than
/// every tick — synapse counts don't change meaningfully tick-to-tick, and
/// rebuilding GPU-gather offsets on every organism every tick would be
/// wasted work.
///
/// ## 3. How It Happens
/// A single query over `(Brain, Neuromodulators)` — no cross-entity shared
/// state, so each organism's update is fully independent (unlike
/// `metabolism_system`'s `GlobalAtmosphere` accumulation).
pub fn hebbian_plasticity_system(
    config: Res<brain::PlasticityConfig>,
    atmosphere: Res<metabolism::GlobalAtmosphere>,
    mut query: Query<(&mut brain::Brain, &brain::Neuromodulators)>,
) {
    let should_prune = config.prune_interval_ticks > 0
        && atmosphere.ticks.is_multiple_of(config.prune_interval_ticks);

    for (mut brain, neuro) in query.iter_mut() {
        let effective_rate = config.hebbian_rate * (1.0 + config.dopamine_gain * neuro.dopamine);
        brain.apply_hebbian_update(effective_rate, config.weight_decay, config.max_weight);

        if should_prune {
            brain.prune_weak_synapses(config.prune_threshold);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;

    fn sample_brain(weight: f32) -> brain::Brain {
        brain::Brain::new(
            brain::BrainId(0),
            vec![
                brain::CtrnnNode {
                    state: 1.0,
                    time_constant: 1.0,
                    bias: 0.0,
                    activation: 7,
                    first_synapse: 0,
                    synapse_count: 0,
                },
                brain::CtrnnNode {
                    state: 1.0,
                    time_constant: 1.0,
                    bias: 0.0,
                    activation: 7,
                    first_synapse: 0,
                    synapse_count: 0,
                },
            ],
            vec![brain::CtrnnSynapse {
                source: 0,
                target: 1,
                weight,
                _padding: 0,
            }],
            1,
            1,
        )
    }

    #[test]
    fn neuromodulator_system_updates_dopamine_on_atp_gain() {
        let mut world = World::new();
        world.spawn((
            metabolism::ChemicalEconomy {
                glucose: 0.0,
                o2: 0.0,
                co2: 0.0,
                atp: 60.0,
                max_glucose: 0.0,
                max_o2: 0.0,
                max_co2: 0.0,
                max_atp: 100.0,
            },
            brain::Neuromodulators::new(50.0),
        ));

        world.run_system_once(neuromodulator_system);

        let mut query = world.query::<&brain::Neuromodulators>();
        let neuro = query.single(&world);
        assert!(neuro.dopamine > 0.0);
    }

    #[test]
    fn hebbian_plasticity_system_scales_rate_by_dopamine() {
        let mut world = World::new();
        world.insert_resource(brain::PlasticityConfig {
            hebbian_rate: 0.1,
            weight_decay: 0.0,
            max_weight: 8.0,
            prune_threshold: 0.0,
            prune_interval_ticks: 0,
            dopamine_gain: 1.0,
        });
        world.insert_resource(metabolism::GlobalAtmosphere::default());

        let mut high_dopamine_neuro = brain::Neuromodulators::new(0.0);
        high_dopamine_neuro.update(100.0, 100.0);

        world.spawn((sample_brain(0.0), brain::Neuromodulators::new(0.0)));
        world.spawn((sample_brain(0.0), high_dopamine_neuro));

        world.run_system_once(hebbian_plasticity_system);

        let mut query = world.query::<&brain::Brain>();
        let weights: Vec<f32> = query.iter(&world).map(|b| b.synapses[0].weight).collect();
        // Both weights moved (pre*post > 0 for both), but the
        // high-dopamine brain's effective rate is larger, so it should
        // have moved further from zero.
        assert!(weights[1].abs() > weights[0].abs());
    }

    #[test]
    fn hebbian_plasticity_system_prunes_on_interval() {
        let mut world = World::new();
        world.insert_resource(brain::PlasticityConfig {
            hebbian_rate: 0.0,
            weight_decay: 0.0,
            max_weight: 8.0,
            prune_threshold: 0.1,
            prune_interval_ticks: 10,
            dopamine_gain: 0.0,
        });
        world.insert_resource(metabolism::GlobalAtmosphere {
            ticks: 20,
            ..metabolism::GlobalAtmosphere::default()
        });

        world.spawn((sample_brain(0.01), brain::Neuromodulators::new(0.0)));

        world.run_system_once(hebbian_plasticity_system);

        let mut query = world.query::<&brain::Brain>();
        let b = query.single(&world);
        assert!(b.synapses.is_empty());
    }

    #[test]
    fn hebbian_plasticity_system_skips_pruning_off_interval() {
        let mut world = World::new();
        world.insert_resource(brain::PlasticityConfig {
            hebbian_rate: 0.0,
            weight_decay: 0.0,
            max_weight: 8.0,
            prune_threshold: 0.1,
            prune_interval_ticks: 10,
            dopamine_gain: 0.0,
        });
        world.insert_resource(metabolism::GlobalAtmosphere {
            ticks: 21,
            ..metabolism::GlobalAtmosphere::default()
        });

        world.spawn((sample_brain(0.01), brain::Neuromodulators::new(0.0)));

        world.run_system_once(hebbian_plasticity_system);

        let mut query = world.query::<&brain::Brain>();
        let b = query.single(&world);
        assert_eq!(b.synapses.len(), 1);
    }
}
