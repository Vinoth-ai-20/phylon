use bevy_ecs::prelude::*;
use rand::Rng;

use crate::Diet;

/// The stage of an organism's infection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InfectionState {
    /// Infected but not yet contagious or symptomatic.
    Incubating,
    /// Contagious and taking ongoing damage.
    Infectious,
    /// Immune — no longer susceptible, no longer contagious.
    Recovered,
}

/// # Pathogen Infection State
///
/// ## 1. What Happens
/// Tracks one organism's progress through a disease: incubation, active
/// infection, and (permanent) recovery/immunity.
///
/// ## 2. Why It Happens
/// A believable disease model needs more than "touch it, die" — real
/// pathogens have a latent period before symptoms, drain the host while
/// active, and provoke an immune response that can end the infection.
///
/// ## 3. How It Happens
/// `virulence` and `transmissibility` are **per-infection**, not global
/// constants: every time this infection spreads to a new host, both drift
/// by a small random amount (see [`DiseaseConfig::mutation_jitter`]) —
/// pathogen mutation, in the sense the spec asks for, without a full
/// separate genome representation.
#[derive(Component, Debug, Clone)]
pub struct Infection {
    /// Current stage of infection.
    pub state: InfectionState,
    /// Ticks spent in the current state.
    pub ticks_in_state: u32,
    /// ATP drained from the host per tick while [`InfectionState::Infectious`].
    pub virulence: f32,
    /// Per-candidate transmission probability per tick while contagious.
    pub transmissibility: f32,
}

/// # Per-Segment Infection Severity
///
/// A single body segment's own local infection severity (Phase 4,
/// `PHASE4_ROADMAP.md` milestone P4-F5) — extends the existing
/// organism-wide [`Infection`] (which stays the authority on
/// incubation/infectious/recovered state and ATP/health drain) with a
/// spatial dimension: how strongly *this* segment, specifically, is
/// affected right now. Attached to every non-head body segment, mirroring
/// `metabolism::ChemicalEconomy::segment_default()`'s per-segment pattern
/// (P4-F2).
#[derive(Component, Debug, Clone, Copy)]
pub struct SegmentInfection {
    /// Local severity, in `[0, 1]` — `0.0` is unaffected, `1.0` is maximal.
    pub severity: f32,
}

impl SegmentInfection {
    /// A freshly grown segment starts unaffected.
    pub fn healthy() -> Self {
        Self { severity: 0.0 }
    }
}

/// # Per-Segment Immune Resistance
///
/// A body segment's own local immune clearance rate (Phase 4, P4-F5) —
/// subtracted from [`SegmentInfection::severity`] each tick by
/// `organisms::immune::segment_infection_system`, modeling localized immune
/// defense independent of the organism-wide recovery roll
/// [`disease_progression_system`] already performs.
#[derive(Component, Debug, Clone, Copy)]
pub struct SegmentImmunity {
    /// Severity cleared per tick, in `[0, 1]`.
    pub resistance: f32,
}

impl SegmentImmunity {
    /// A placeholder baseline clearance rate — not biologically tuned, same
    /// status as `metabolism::ChemicalEconomy::segment_default()`'s pool
    /// sizes.
    pub fn baseline() -> Self {
        Self { resistance: 0.02 }
    }
}

/// Global tunables for disease spread and progression.
#[derive(Resource, Debug, Clone)]
pub struct DiseaseConfig {
    /// Radius within which an infectious organism can transmit to a
    /// susceptible one — modeled as direct proximity transmission via the
    /// existing spatial-grid infrastructure (see `disease_spread_system`'s
    /// doc comment for why this isn't a diffused concentration field).
    pub transmission_radius: f32,
    /// Ticks spent incubating before becoming infectious.
    pub incubation_ticks: u32,
    /// Multiplier applied to transmission probability when source and
    /// target have different `Diet`s — cross-species spillover is real but
    /// harder than same-species transmission.
    pub cross_diet_transmission_multiplier: f32,
    /// Per-tick probability an infectious organism recovers (becomes
    /// permanently immune) — the immune-response term.
    pub recovery_probability_per_tick: f32,
    /// Maximum +/- fractional change applied to `virulence`/`transmissibility`
    /// on each transmission event (pathogen mutation).
    pub mutation_jitter: f32,
    /// `virulence` a brand new (unmutated) infection starts with.
    pub initial_virulence: f32,
    /// `transmissibility` a brand new (unmutated) infection starts with.
    pub initial_transmissibility: f32,
    /// Per-tick probability an uninfected organism spontaneously contracts
    /// a fresh (unmutated) infection — stands in for an environmental
    /// pathogen reservoir, since nothing else seeds the first case.
    pub spontaneous_infection_probability: f32,
}

impl Default for DiseaseConfig {
    fn default() -> Self {
        Self {
            transmission_radius: 60.0,
            incubation_ticks: 200,
            cross_diet_transmission_multiplier: 0.3,
            recovery_probability_per_tick: 0.002,
            mutation_jitter: 0.05,
            initial_virulence: 5.0,
            initial_transmissibility: 0.1,
            spontaneous_infection_probability: 0.00002,
        }
    }
}

/// Draws a jittered copy of `value`, clamped to non-negative, using the
/// caller-supplied `rng` — never a fresh unseeded source (see
/// `common::SimRng`'s doc comment).
fn jitter(value: f32, max_jitter: f32, rng: &mut impl rand::Rng) -> f32 {
    let delta = rng.gen_range(-max_jitter..max_jitter);
    (value * (1.0 + delta)).max(0.0)
}

/// # Disease Progression System
///
/// ## 1. What Happens
/// Advances every infected organism's [`Infection`] state machine one tick:
/// incubation countdown, ATP/health drain while infectious, and a
/// per-tick immune-response roll toward recovery.
///
/// ## 2. Why It Happens
/// Progression is separated from spread ([`disease_spread_system`]) so each
/// stays a simple, single-purpose query — matching this crate's existing
/// pattern of one specialized system per ecological process (photosynthesis,
/// decay, foraging).
///
/// ## 3. How It Happens
/// Drains `ChemicalEconomy.atp` directly — the currency `metabolism_system`'s
/// existing death check (`atp <= 0.0`) already reads — rather than inventing
/// a parallel death pathway through the otherwise-inert `Health` component.
/// `Health` is drained too, purely for display/inspection, since nothing
/// else currently reads it.
pub fn disease_progression_system(
    config: Res<DiseaseConfig>,
    mut sim_rng: ResMut<common::SimRng>,
    mut query: Query<(
        &mut Infection,
        &mut metabolism::ChemicalEconomy,
        Option<&mut metabolism::Health>,
    )>,
) {
    for (mut infection, mut chem, health) in query.iter_mut() {
        infection.ticks_in_state += 1;

        match infection.state {
            InfectionState::Incubating => {
                if infection.ticks_in_state >= config.incubation_ticks {
                    infection.state = InfectionState::Infectious;
                    infection.ticks_in_state = 0;
                }
            }
            InfectionState::Infectious => {
                chem.atp = (chem.atp - infection.virulence).max(0.0);
                if let Some(mut health) = health {
                    health.current = (health.current - infection.virulence).max(0.0);
                }
                if sim_rng.gen::<f32>() < config.recovery_probability_per_tick {
                    infection.state = InfectionState::Recovered;
                    infection.ticks_in_state = 0;
                }
            }
            InfectionState::Recovered => {}
        }
    }
}

/// Susceptible-candidate query filter: has no [`Infection`] yet but does
/// have a [`metabolism::ChemicalEconomy`] (i.e. is a living organism, not a
/// food pellet or other non-organism entity that happens to have a `Diet`).
type SusceptibleFilter = (Without<Infection>, With<metabolism::ChemicalEconomy>);

/// # Disease Spread System
///
/// ## 1. What Happens
/// Every tick, each infectious organism may transmit its infection to
/// nearby susceptible organisms (no [`Infection`] component yet), and every
/// susceptible organism has a small chance of spontaneous infection from an
/// implicit environmental reservoir.
///
/// ## 2. Why It Happens
/// The spec asks for "disease spread via concentration field," but Phylon's
/// diffusion fields are GPU-resident PDEs (`crates/gpu/src/diffusion_step.wgsl`)
/// with no CPU-side per-organism read/write path for a new field layer
/// without touching the GPU shader and its bind-group layout — real but
/// avoidable complexity for a first implementation. Modeling transmission as
/// direct proximity via the existing `spatial::UniformGrid` infrastructure
/// (the same pattern `foraging_system` already uses for predation) gets the
/// same emergent behavior — local outbreaks, distance-limited spread — at a
/// fraction of the risk; a true diffused concentration field is a possible
/// future refinement, not required for this milestone.
///
/// ## 3. How It Happens
/// Broad-phase candidates come from a per-tick `spatial::UniformGrid` keyed
/// on organism position. `cross_diet_transmission_multiplier` reduces
/// (never increases) transmission probability across `Diet` boundaries —
/// cross-species spillover is possible but harder. A successful transmission
/// **jitters** the new infection's `virulence`/`transmissibility` away from
/// the source's — pathogen mutation, drifting the strain each time it jumps
/// hosts, without a full separate genome representation.
pub fn disease_spread_system(
    mut commands: Commands,
    config: Res<DiseaseConfig>,
    mut sim_rng: ResMut<common::SimRng>,
    infectious_query: Query<(Entity, &Diet, &physics::ParticleNode, &Infection)>,
    susceptible_query: Query<(Entity, &Diet, &physics::ParticleNode), SusceptibleFilter>,
) {
    let mut grid = spatial::UniformGrid::new(50.0).unwrap();
    for (entity, _diet, node) in susceptible_query.iter() {
        let _ = grid.insert(entity, node.position);
    }

    let mut newly_infected: Vec<(Entity, f32, f32)> = Vec::new();
    let mut already_targeted: std::collections::HashSet<Entity> = std::collections::HashSet::new();

    for (_source_entity, source_diet, source_node, infection) in infectious_query.iter() {
        if infection.state != InfectionState::Infectious {
            continue;
        }
        for candidate in grid.query_radius(source_node.position, config.transmission_radius) {
            if already_targeted.contains(&candidate) {
                continue;
            }
            let Ok((target_entity, target_diet, _target_node)) = susceptible_query.get(candidate)
            else {
                continue;
            };

            let cross_species_penalty = if source_diet == target_diet {
                1.0
            } else {
                config.cross_diet_transmission_multiplier
            };
            let probability = infection.transmissibility * cross_species_penalty;

            if sim_rng.gen::<f32>() < probability {
                let virulence = jitter(infection.virulence, config.mutation_jitter, &mut sim_rng.0);
                let transmissibility = jitter(
                    infection.transmissibility,
                    config.mutation_jitter,
                    &mut sim_rng.0,
                );
                newly_infected.push((target_entity, virulence, transmissibility));
                already_targeted.insert(target_entity);
            }
        }
    }

    for (entity, virulence, transmissibility) in newly_infected {
        commands.entity(entity).insert(Infection {
            state: InfectionState::Incubating,
            ticks_in_state: 0,
            virulence,
            transmissibility,
        });
    }

    // Spontaneous infection: a background environmental reservoir, since
    // nothing else seeds the very first case in a fresh ecosystem.
    for (entity, _diet, _node) in susceptible_query.iter() {
        if already_targeted.contains(&entity) {
            continue;
        }
        if sim_rng.gen::<f32>() < config.spontaneous_infection_probability {
            commands.entity(entity).insert(Infection {
                state: InfectionState::Incubating,
                ticks_in_state: 0,
                virulence: config.initial_virulence,
                transmissibility: config.initial_transmissibility,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;

    fn sample_chem(atp: f32) -> metabolism::ChemicalEconomy {
        metabolism::ChemicalEconomy {
            glucose: 0.0,
            o2: 0.0,
            co2: 0.0,
            atp,
            max_glucose: 0.0,
            max_o2: 0.0,
            max_co2: 0.0,
            max_atp: 100.0,
        }
    }

    #[test]
    fn jitter_never_goes_negative() {
        use rand::SeedableRng;
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(1);
        for _ in 0..1000 {
            assert!(jitter(0.01, 0.9, &mut rng) >= 0.0);
        }
    }

    #[test]
    fn progression_transitions_incubating_to_infectious_after_incubation_period() {
        let mut world = World::new();
        world.insert_resource(DiseaseConfig {
            incubation_ticks: 3,
            ..DiseaseConfig::default()
        });
        world.insert_resource(common::SimRng::from_seed(1));
        let e = world
            .spawn((
                Infection {
                    state: InfectionState::Incubating,
                    ticks_in_state: 0,
                    virulence: 1.0,
                    transmissibility: 0.0,
                },
                sample_chem(50.0),
            ))
            .id();

        for _ in 0..3 {
            world.run_system_once(disease_progression_system);
        }

        let infection = world.get::<Infection>(e).unwrap();
        assert_eq!(infection.state, InfectionState::Infectious);
    }

    #[test]
    fn progression_drains_atp_while_infectious() {
        let mut world = World::new();
        world.insert_resource(DiseaseConfig {
            recovery_probability_per_tick: 0.0,
            ..DiseaseConfig::default()
        });
        world.insert_resource(common::SimRng::from_seed(1));
        let e = world
            .spawn((
                Infection {
                    state: InfectionState::Infectious,
                    ticks_in_state: 0,
                    virulence: 5.0,
                    transmissibility: 0.0,
                },
                sample_chem(50.0),
            ))
            .id();

        world.run_system_once(disease_progression_system);

        assert_eq!(
            world.get::<metabolism::ChemicalEconomy>(e).unwrap().atp,
            45.0
        );
    }

    #[test]
    fn progression_recovers_with_certainty_when_probability_is_one() {
        let mut world = World::new();
        world.insert_resource(DiseaseConfig {
            recovery_probability_per_tick: 1.0,
            ..DiseaseConfig::default()
        });
        world.insert_resource(common::SimRng::from_seed(1));
        let e = world
            .spawn((
                Infection {
                    state: InfectionState::Infectious,
                    ticks_in_state: 0,
                    virulence: 1.0,
                    transmissibility: 0.0,
                },
                sample_chem(50.0),
            ))
            .id();

        world.run_system_once(disease_progression_system);

        assert_eq!(
            world.get::<Infection>(e).unwrap().state,
            InfectionState::Recovered
        );
    }

    #[test]
    fn recovered_state_never_progresses_further() {
        let mut world = World::new();
        world.insert_resource(DiseaseConfig::default());
        world.insert_resource(common::SimRng::from_seed(1));
        let e = world
            .spawn((
                Infection {
                    state: InfectionState::Recovered,
                    ticks_in_state: 0,
                    virulence: 5.0,
                    transmissibility: 0.0,
                },
                sample_chem(50.0),
            ))
            .id();

        for _ in 0..10 {
            world.run_system_once(disease_progression_system);
        }

        assert_eq!(
            world.get::<metabolism::ChemicalEconomy>(e).unwrap().atp,
            50.0
        );
        assert_eq!(
            world.get::<Infection>(e).unwrap().state,
            InfectionState::Recovered
        );
    }

    #[test]
    fn spread_infects_susceptible_organism_in_range_with_certain_transmissibility() {
        let mut world = World::new();
        world.insert_resource(DiseaseConfig {
            transmission_radius: 100.0,
            cross_diet_transmission_multiplier: 1.0,
            spontaneous_infection_probability: 0.0,
            ..DiseaseConfig::default()
        });
        world.insert_resource(common::SimRng::from_seed(1));

        world.spawn((
            Diet::Herbivore,
            physics::ParticleNode::new(common::Vec2::new(0.0, 0.0), 1.0, 0, 1),
            Infection {
                state: InfectionState::Infectious,
                ticks_in_state: 0,
                virulence: 1.0,
                transmissibility: 1.0,
            },
        ));
        let target = world
            .spawn((
                Diet::Herbivore,
                physics::ParticleNode::new(common::Vec2::new(10.0, 0.0), 1.0, 0, 2),
                sample_chem(50.0),
            ))
            .id();

        world.run_system_once(disease_spread_system);

        assert!(world.get::<Infection>(target).is_some());
    }

    #[test]
    fn spread_never_infects_beyond_transmission_radius() {
        let mut world = World::new();
        world.insert_resource(DiseaseConfig {
            transmission_radius: 10.0,
            cross_diet_transmission_multiplier: 1.0,
            spontaneous_infection_probability: 0.0,
            ..DiseaseConfig::default()
        });
        world.insert_resource(common::SimRng::from_seed(1));

        world.spawn((
            Diet::Herbivore,
            physics::ParticleNode::new(common::Vec2::new(0.0, 0.0), 1.0, 0, 1),
            Infection {
                state: InfectionState::Infectious,
                ticks_in_state: 0,
                virulence: 1.0,
                transmissibility: 1.0,
            },
        ));
        let target = world
            .spawn((
                Diet::Herbivore,
                physics::ParticleNode::new(common::Vec2::new(500.0, 0.0), 1.0, 0, 2),
                sample_chem(50.0),
            ))
            .id();

        world.run_system_once(disease_spread_system);

        assert!(world.get::<Infection>(target).is_none());
    }

    #[test]
    fn spread_cross_diet_multiplier_of_zero_blocks_spillover() {
        let mut world = World::new();
        world.insert_resource(DiseaseConfig {
            transmission_radius: 100.0,
            cross_diet_transmission_multiplier: 0.0,
            spontaneous_infection_probability: 0.0,
            ..DiseaseConfig::default()
        });
        world.insert_resource(common::SimRng::from_seed(1));

        world.spawn((
            Diet::Herbivore,
            physics::ParticleNode::new(common::Vec2::new(0.0, 0.0), 1.0, 0, 1),
            Infection {
                state: InfectionState::Infectious,
                ticks_in_state: 0,
                virulence: 1.0,
                transmissibility: 1.0,
            },
        ));
        let target = world
            .spawn((
                Diet::Carnivore,
                physics::ParticleNode::new(common::Vec2::new(10.0, 0.0), 1.0, 0, 2),
                sample_chem(50.0),
            ))
            .id();

        world.run_system_once(disease_spread_system);

        assert!(world.get::<Infection>(target).is_none());
    }

    #[test]
    fn spread_is_deterministic_for_same_seed() {
        let build = |seed: u64| {
            let mut world = World::new();
            world.insert_resource(DiseaseConfig {
                transmission_radius: 100.0,
                ..DiseaseConfig::default()
            });
            world.insert_resource(common::SimRng::from_seed(seed));
            world.spawn((
                Diet::Herbivore,
                physics::ParticleNode::new(common::Vec2::new(0.0, 0.0), 1.0, 0, 1),
                Infection {
                    state: InfectionState::Infectious,
                    ticks_in_state: 0,
                    virulence: 1.0,
                    transmissibility: 0.5,
                },
            ));
            for i in 0..20 {
                world.spawn((
                    Diet::Herbivore,
                    physics::ParticleNode::new(
                        common::Vec2::new(i as f32 * 2.0, 0.0),
                        1.0,
                        0,
                        i + 2,
                    ),
                    sample_chem(50.0),
                ));
            }
            world
        };

        let mut w1 = build(42);
        let mut w2 = build(42);
        w1.run_system_once(disease_spread_system);
        w2.run_system_once(disease_spread_system);

        let mut q1 = w1.query::<&Infection>();
        let mut q2 = w2.query::<&Infection>();
        let count1 = q1.iter(&w1).count();
        let count2 = q2.iter(&w2).count();
        assert_eq!(count1, count2);
    }
}
