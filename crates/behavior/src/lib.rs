//! # Phylon Behavior
//!
//! Movement decisions, action selection, and locomotion output systems.
//!
//! Organisms receive sensory inputs, process them through their neural brain,
//! and emit motor commands. This crate converts neural output into physical
//! forces applied to the organism's particle nodes. Also derives/regenerates
//! `Health` and `BehaviorState` from metabolic state each tick (see
//! [`physiological_state_update_system`]).

#![warn(missing_docs)]
#![warn(clippy::all)]

/// # Motor Output Mapping Component
///
/// ## 1. What Happens
/// The `MotorSystem` component stores an ordered list of ECS entities representing the physical
/// `Spring` effectors an organism can actuate.
///
/// ## 2. Why It Happens
/// A neural brain outputs an array of normalized scalar values. Without a routing table, the
/// organism has no topological awareness of which output neuron connects to which physical
/// muscle segment. This component serves as the peripheral nervous system routing.
///
/// ## 3. How It Happens
/// During the `behavior_system`, the outputs of the `Brain` component array are iterated
/// sequentially and zipped against the `effectors` array. Neural output $O_i$ maps directly to
/// `effectors[i]`.
#[derive(bevy_ecs::prelude::Component, Debug, Clone)]
pub struct MotorSystem {
    /// Ordered list of spring entities this organism can actuate.
    pub effectors: Vec<bevy_ecs::entity::Entity>,
}

/// # Biological Behavior Configuration
///
/// ## 1. What Happens
/// The `BehaviorConfig` is a global ECS resource that defines the thermodynamic and metabolic
/// constants for behavioral outputs (e.g., pheromone signaling costs).
///
/// ## 2. Why It Happens
/// In theoretical biology, there is no "free lunch". Every action (moving, signaling, mating)
/// requires ATP. If emitting a pheromone signal costs zero energy, organisms will rapidly evolve
/// to scream globally at 100% volume at all times. Introducing an energy cost creates a selective
/// pressure for efficient, localized communication.
///
/// ## 3. How It Happens
/// It defines `signal_energy_cost_per_unit`. During emission, the cost subtracted from the
/// `ChemicalEconomy` is linearly proportional to the output amplitude $A$:
///
/// $$ \Delta ATP = - (A \times \text{signal\_cost}) $$
#[derive(bevy_ecs::prelude::Resource, Debug, Clone)]
pub struct BehaviorConfig {
    /// Energy cost per unit of signal emission amplitude.
    pub signal_energy_cost_per_unit: f32,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            signal_energy_cost_per_unit: 0.01,
        }
    }
}

/// The high-level behavioral state of the organism.
#[derive(bevy_ecs::prelude::Component, Debug, Clone, PartialEq, Eq, Default)]
pub enum BehaviorState {
    /// Organism is inactive or resting.
    #[default]
    Idle,
    /// Organism is actively seeking food or resources.
    Foraging,
    /// Organism is actively hunting prey.
    Hunting,
    /// Organism is fleeing from a hazard or predator.
    Fleeing,
    /// Organism is seeking a mate.
    Mating,
    /// Organism is sleeping to conserve energy.
    Sleeping,
}

/// The current goal/target the behavior is trying to satisfy.
#[derive(bevy_ecs::prelude::Component, Debug, Clone)]
pub struct CurrentGoal {
    /// A human-readable description of the current goal (e.g. "Seeking Glucose").
    pub description: String,
    /// An optional target entity.
    pub target_entity: Option<bevy_ecs::entity::Entity>,
}

/// One organism's read-only inputs to [`compute_behavior`] — captured by
/// value so the computation can run on any thread with no live ECS access.
/// See `behavior_system`'s doc comment for why the system is split this way.
struct OrganismSnapshot {
    entity: bevy_ecs::entity::Entity,
    brain_outputs: Option<Vec<f32>>,
    /// `(spring_entity, base_length, constraint_type)` for each effector,
    /// in the same order as `MotorSystem::effectors` — read once, up front,
    /// during the sequential snapshot phase (see doc comment).
    effectors: Vec<(bevy_ecs::entity::Entity, f32, physics::ConstraintType)>,
    has_emitter: bool,
    has_energy: bool,
    env_temp: Option<f32>,
}

/// One organism's computed result — pure data, applied back to the ECS by
/// `behavior_system` in a second, sequential pass.
struct OrganismResult {
    entity: bevy_ecs::entity::Entity,
    /// `(spring_entity, new_actuation_amplitude, new_rest_length_if_elastic)`.
    spring_updates: Vec<(bevy_ecs::entity::Entity, f32, Option<f32>)>,
    /// Total ATP to subtract (rigidity punishment across all springs, plus
    /// signal-emission cost) — see this function's doc comment on why
    /// combining every subtraction into one delta, clamped once, is
    /// equivalent to the original step-by-step clamped subtraction.
    atp_delta: f32,
    /// `Some(new_value)` if the organism has both a brain and a
    /// [`diffusion::SignalEmitter`].
    emitter_value: Option<f32>,
}

/// Pure per-organism behavior computation — reads only `snap` and the
/// read-only environment/config values, touches no shared mutable state
/// (crucially, no `Query`). Safe to call from any thread; `behavior_system`
/// runs this via `rayon`'s `par_iter`.
///
/// Combines every ATP subtraction (per-spring rigidity punishment, plus
/// signal-emission cost) into a single `atp_delta`, applied with one
/// `max(0.0)` clamp by the caller, rather than the original's repeated
/// `chem.atp = (chem.atp - d).max(0.0)` per subtraction. These are
/// equivalent: repeated clamped subtraction of non-negative amounts with no
/// intervening addition is exactly `max(0, initial - sum_of_amounts)` — once
/// the value hits zero it stays zero either way, and before that point both
/// formulations track the same running total.
fn compute_behavior(
    snap: &OrganismSnapshot,
    efficiency_ideal_temp: f32,
    signal_cost_per_unit: f32,
) -> OrganismResult {
    let mut spring_updates = Vec::new();
    let mut atp_delta = 0.0f32;
    let mut emitter_value = None;

    if let Some(outputs) = &snap.brain_outputs {
        // Calculate environmental efficiency based on local temperature.
        let efficiency = snap.env_temp.map_or(1.0, |temp| {
            let divergence = (temp - efficiency_ideal_temp).abs();
            // Efficiency drops linearly by 5% per degree off ideal. At 20
            // degrees off (e.g. 35C or -5C), efficiency is 0.0 (paralyzed).
            (1.0 - (divergence * 0.05)).clamp(0.0, 1.0)
        });

        // 2. Route outputs to effectors
        for (i, &(spring_entity, base_length, constraint_type)) in snap.effectors.iter().enumerate()
        {
            if i >= outputs.len() {
                continue;
            }
            let actuation = outputs[i];
            let effective_actuation = actuation * efficiency;
            let new_amplitude = effective_actuation * 8.0;
            let new_rest_length = if constraint_type == physics::ConstraintType::Elastic {
                Some(base_length + (effective_actuation * base_length * 0.5))
            } else {
                None
            };
            spring_updates.push((spring_entity, new_amplitude, new_rest_length));

            // Punish rigidity: if the muscle is locked at high actuation,
            // drain a small amount of ATP.
            if actuation.abs() > 0.9 && snap.has_energy {
                atp_delta += 0.05;
            }
        }

        // 3. Route to signal emitter if present
        let mut signal_output: f32 = 0.0;
        if !snap.effectors.is_empty() {
            if snap.effectors.len() < outputs.len() {
                signal_output = outputs[snap.effectors.len()];
            }
        } else if !outputs.is_empty() {
            signal_output = outputs[0];
        }

        if snap.has_emitter {
            let emission = signal_output.clamp(0.0, 1.0);
            emitter_value = Some(emission);

            if emission > 0.0 && snap.has_energy {
                atp_delta += emission * signal_cost_per_unit;
            }
        }
    }

    OrganismResult {
        entity: snap.entity,
        spring_updates,
        atp_delta,
        emitter_value,
    }
}

/// # Core Behavior Translation System
///
/// ## 1. What Happens
/// The `behavior_system` bridges the cognitive and physical domains. It reads the integrated
/// outputs from each organism's CTRNN (continuous-time recurrent neural network — see the
/// `brain` crate for what that means and how it's integrated) and translates them into
/// structural actuations (muscle contractions) and chemical pheromone emissions, while applying
/// environmental temperature constraints.
///
/// ## 2. Why It Happens
/// Physical movement in a simulated fluid/environment requires forces. The neural network cannot
/// push itself; it can only tense "muscles" (Hookean springs). Furthermore, organism biology is
/// heavily influenced by thermodynamics. Ectothermic (cold-blooded) organisms suffer severe
/// kinematic paralysis outside their ideal temperature ranges.
///
/// ## 3. How It Happens
/// The system executes in three sequential phases per organism:
///
/// **Phase A: Temperature Efficiency**
/// A linear drop-off efficiency multiplier $\eta$ is calculated based on the divergence from the
/// ideal temperature $T_{ideal}$ ($15^\circ C$):
///
/// $$ \eta = \max\left(0, 1.0 - 0.05 \times |T_{local} - T_{ideal}|\right) $$
///
/// **Phase B: Spring Actuation**
/// Neural outputs $O_i \in [-1, 1]$ are routed to the associated `Spring`. The target resting
/// length $L_{rest}$ is modulated by the temperature-penalized actuation:
///
/// $$ Actuation = O_i \times \eta $$
/// $$ L_{rest} = L_{base} + \left(Actuation \times L_{base} \times 0.5\right) $$
///
/// To penalize rigid tetanic contractions, holding a muscle at extreme actuation ($|O_i| > 0.9$)
/// drains additional ATP.
///
/// **Phase C: Signal Emission**
/// Excess neural outputs not routed to muscles are clamped to $[0, 1]$ and mapped to the
/// `SignalEmitter` component, draining ATP proportionally to the emission strength.
///
/// ## Parallel/sequential split (determinism)
///
/// As with `metabolism::metabolism_system` and `sensing::sensing_system`,
/// the actual per-organism computation (Phases A-C above) is pure
/// — it depends only on that organism's own brain outputs, its effectors'
/// current spring state, and read-only environment/config values — so it's
/// computed in parallel via `compute_behavior`. The one piece of
/// cross-entity structure is that spring entities are looked up by ID
/// (`MotorSystem::effectors`) rather than held directly; different
/// organisms' effectors are disjoint spring entities in practice, but nothing
/// in the type system proves that, so — unlike metabolism's shared-resource
/// accumulation — this system's parallel phase touches no `Query` at all:
/// every spring's current `base_length`/`constraint_type` is read into the
/// snapshot *before* going parallel, and every mutation (`Spring::
/// actuation_amplitude`/`rest_length`, `ChemicalEconomy::atp`,
/// `SignalEmitter::value`) is applied in a single sequential pass afterward.
#[allow(clippy::type_complexity)]
pub fn behavior_system(
    mut query: bevy_ecs::prelude::Query<(
        bevy_ecs::entity::Entity,
        &physics::ParticleNode,
        &sensing::SensoryState,
        Option<&brain::Brain>,
        Option<&MotorSystem>,
        Option<&mut diffusion::SignalEmitter>,
        Option<&mut metabolism::ChemicalEconomy>,
        Option<&metabolism::Age>,
    )>,
    mut springs: bevy_ecs::prelude::Query<&mut physics::Spring>,
    env: Option<bevy_ecs::prelude::Res<environment::EnvironmentManager>>,
    config: Option<bevy_ecs::prelude::Res<BehaviorConfig>>,
) {
    use rayon::prelude::*;

    // Time step integration is fully handled by the GPU compute pass.
    const IDEAL_TEMP: f32 = 15.0; // Not biologically tuned to any specific organism or biome
    let signal_cost_per_unit = config
        .as_ref()
        .map_or(0.01, |c| c.signal_energy_cost_per_unit);

    // Snapshot phase (sequential): each organism's own state, plus a
    // read-only lookup of its effector springs' *current* base_length/
    // constraint_type (never mutated here — only read, to feed the pure
    // computation). Different organisms' effectors are disjoint spring
    // entities in practice, so this sequential read is trivially safe
    // regardless; the mutation happens later, in the writeback phase.
    let snapshots: Vec<OrganismSnapshot> = query
        .iter()
        .map(
            |(entity, node, _sensory, brain_opt, motor_opt, emitter_opt, energy_opt, _age_opt)| {
                let effectors = motor_opt
                    .map(|motor| {
                        motor
                            .effectors
                            .iter()
                            .filter_map(|&e| {
                                springs
                                    .get(e)
                                    .ok()
                                    .map(|s| (e, s.base_length, s.constraint_type))
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                OrganismSnapshot {
                    entity,
                    brain_outputs: brain_opt.map(|b| b.get_outputs()),
                    effectors,
                    has_emitter: emitter_opt.is_some(),
                    has_energy: energy_opt.is_some(),
                    env_temp: env
                        .as_ref()
                        .map(|e| e.get_temperature_at(node.position.x, node.position.y)),
                }
            },
        )
        .collect();

    // Parallel phase: pure per-organism computation, no shared state touched.
    let results: Vec<OrganismResult> = snapshots
        .par_iter()
        .map(|snap| compute_behavior(snap, IDEAL_TEMP, signal_cost_per_unit))
        .collect();

    // Sequential writeback phase.
    for result in results {
        for (spring_entity, amplitude, new_rest_length) in result.spring_updates {
            if let Ok(mut spring) = springs.get_mut(spring_entity) {
                spring.actuation_amplitude = amplitude;
                if let Some(rest_length) = new_rest_length {
                    spring.rest_length = rest_length;
                }
            }
        }

        if result.emitter_value.is_none() && result.atp_delta <= 0.0 {
            continue;
        }
        if let Ok((_, _, _, _, _, mut emitter_opt, mut energy_opt, _)) =
            query.get_mut(result.entity)
        {
            if let (Some(emitter), Some(value)) = (emitter_opt.as_mut(), result.emitter_value) {
                emitter.value = value;
            }
            if result.atp_delta > 0.0 {
                if let Some(chem) = energy_opt.as_mut() {
                    chem.atp = (chem.atp - result.atp_delta).max(0.0);
                }
            }
        }
    }
}

/// # Physiological State Update System
///
/// ## 1. What Happens
/// Each tick, `Hydration` decreases by `loss_rate`, `BodyTemperature` moves toward the
/// local environment temperature, and `BehaviorState` / `CurrentGoal` are set based on
/// the current metabolic reading. This ensures the Inspector always shows live data.
///
/// ## 2. Why It Happens
/// `BehaviorState` is a top-level observable: a researcher can tell at a glance whether
/// an organism is foraging, fleeing, or sleeping. Without a dedicated update pass it would
/// always read `Idle` — the default set at spawn.
///
/// ## 3. How It Happens
/// Metabolic urgency is evaluated in priority order:
/// 1. Very low ATP (< 10 %) → **Fleeing** (stress response / looking for energy)
/// 2. Low glucose (< 20 %) → **Foraging**
/// 3. Low hydration (< 0.2) → **Foraging** (seeking water)
/// 4. Otherwise → **Idle**
///
/// Also regenerates/drains `Health` from the same ATP reading: surplus ATP
/// (> 50 %) slowly heals injury, critical ATP (< 10 %, the same threshold
/// that triggers Fleeing) causes starvation damage — the first time
/// `Health` moves in the "recovery" direction rather than only ever
/// draining (see `ecology::disease_progression_system`, which drains it
/// for infected organisms).
#[allow(clippy::type_complexity)]
pub fn physiological_state_update_system(
    mut query: bevy_ecs::prelude::Query<(
        bevy_ecs::entity::Entity,
        &physics::ParticleNode,
        Option<&mut metabolism::Hydration>,
        Option<&mut metabolism::BodyTemperature>,
        Option<&metabolism::ChemicalEconomy>,
        Option<&mut BehaviorState>,
        Option<&mut CurrentGoal>,
        Option<&mut metabolism::Health>,
    )>,
    env: Option<bevy_ecs::prelude::Res<environment::EnvironmentManager>>,
) {
    for (_entity, node, hydration_opt, temp_opt, chem_opt, state_opt, goal_opt, health_opt) in
        query.iter_mut()
    {
        // 1. Tick Hydration
        if let Some(mut hydration) = hydration_opt {
            hydration.level = (hydration.level - hydration.loss_rate).clamp(0.0, 1.0);
        }

        // 2. Move BodyTemperature toward environment temperature
        if let Some(mut body_temp) = temp_opt {
            let env_temp = env
                .as_ref()
                .map(|e| e.get_temperature_at(node.position.x, node.position.y))
                .unwrap_or(22.0);
            // Lerp 2 % per tick toward the environment temperature
            body_temp.current += (env_temp - body_temp.current) * 0.02;
        }

        // 3a. Health regeneration/drain from ATP surplus/deficit — computed
        // before the `chem_opt` move below, so it applies independently of
        // whether `BehaviorState`/`CurrentGoal` are present on this entity.
        if let (Some(chem), Some(mut health)) = (chem_opt.as_ref(), health_opt) {
            let atp_fraction = if chem.max_atp > 0.0 {
                chem.atp / chem.max_atp
            } else {
                0.0
            };
            const HEALTH_REGEN_PER_TICK: f32 = 0.05;
            const STARVATION_DAMAGE_PER_TICK: f32 = 0.1;
            if atp_fraction > 0.5 {
                health.current = (health.current + HEALTH_REGEN_PER_TICK).min(health.max);
            } else if atp_fraction < 0.10 {
                health.current = (health.current - STARVATION_DAMAGE_PER_TICK).max(0.0);
            }
        }

        // 3b. Derive BehaviorState from metabolic urgency
        if let (Some(mut bstate), Some(mut goal), Some(chem)) = (state_opt, goal_opt, chem_opt) {
            let atp_fraction = if chem.max_atp > 0.0 {
                chem.atp / chem.max_atp
            } else {
                0.0
            };
            let glucose_fraction = if chem.max_glucose > 0.0 {
                chem.glucose / chem.max_glucose
            } else {
                0.0
            };

            if atp_fraction < 0.10 {
                *bstate = BehaviorState::Fleeing;
                goal.description = "Critical ATP – seeking energy".to_string();
                goal.target_entity = None;
            } else if glucose_fraction < 0.20 {
                *bstate = BehaviorState::Foraging;
                goal.description = "Low glucose – foraging".to_string();
                goal.target_entity = None;
            } else {
                *bstate = BehaviorState::Idle;
                goal.description = "Nominal".to_string();
                goal.target_entity = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;

    #[test]
    fn motor_system_initialization() {
        let ms = MotorSystem { effectors: vec![] };
        assert!(ms.effectors.is_empty());
    }

    /// (spring actuation_amplitude, spring rest_length, atp, emitter value)
    /// for one organism after a `behavior_system` tick.
    type OrganismOutcome = (f32, f32, f32, f32);

    fn build_world_with_organisms(n: u32) -> World {
        let mut world = World::new();
        world.insert_resource(BehaviorConfig::default());

        for i in 0..n {
            // Node states vary per organism so brain outputs (and hence
            // actuation) actually differ, including some triggering the
            // rigidity-punishment branch (|output| > 0.9).
            let state = -1.2 + (i as f32 * 0.05);
            let node = brain::CtrnnNode {
                state,
                time_constant: 1.0,
                bias: 0.0,
                activation: 1, // Tanh — squashes into (-1, 1)
                first_synapse: 0,
                synapse_count: 0,
            };
            let brain = brain::Brain::new(brain::BrainId(i as u64), vec![node], vec![], 0, 1);

            let spring_entity = world
                .spawn(physics::Spring {
                    node_a: bevy_ecs::entity::Entity::PLACEHOLDER,
                    node_b: bevy_ecs::entity::Entity::PLACEHOLDER,
                    constraint_type: physics::ConstraintType::Elastic,
                    rest_length: 20.0,
                    base_length: 20.0,
                    stiffness: 5.0,
                    damping: 0.3,
                    actuation_amplitude: 0.0,
                    actuation_phase: 0.0,
                    breaking_strain: 5.0,
                    is_fin: 0,
                })
                .id();

            world.spawn((
                physics::ParticleNode::new(common::Vec3::new(i as f32 * 10.0, 0.0, 0.0), 1.0, 0, i),
                sensing::SensoryState::new(1),
                brain,
                MotorSystem {
                    effectors: vec![spring_entity],
                },
                diffusion::SignalEmitter::default(),
                metabolism::ChemicalEconomy {
                    glucose: 500.0,
                    o2: 300.0,
                    co2: 50.0,
                    atp: 400.0,
                    max_glucose: 1000.0,
                    max_o2: 1000.0,
                    max_co2: 1000.0,
                    max_atp: 1000.0,
                },
            ));
        }
        world
    }

    fn run_behavior_with_thread_count(
        n_threads: usize,
        organism_count: u32,
    ) -> Vec<OrganismOutcome> {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(n_threads)
            .build()
            .unwrap();
        let mut world = build_world_with_organisms(organism_count);

        pool.install(|| {
            world.run_system_once(behavior_system);
        });

        let mut query = world.query::<(
            &physics::ParticleNode,
            &MotorSystem,
            &metabolism::ChemicalEconomy,
            &diffusion::SignalEmitter,
        )>();
        let mut springs = world.query::<&physics::Spring>();

        let mut results: Vec<_> = query
            .iter(&world)
            .map(|(node, motor, chem, emitter)| {
                let spring = springs.get(&world, motor.effectors[0]).unwrap();
                (
                    node.organism_id,
                    (
                        spring.actuation_amplitude,
                        spring.rest_length,
                        chem.atp,
                        emitter.value,
                    ),
                )
            })
            .collect();
        results.sort_by_key(|(id, _)| *id);
        results.into_iter().map(|(_, r)| r).collect()
    }

    #[test]
    fn behavior_is_deterministic_regardless_of_thread_count() {
        let results_1 = run_behavior_with_thread_count(1, 150);
        let results_8 = run_behavior_with_thread_count(8, 150);
        assert_eq!(
            results_1, results_8,
            "spring/atp/emitter state diverged between 1 and 8 threads"
        );
    }

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
    fn health_regenerates_when_atp_surplus() {
        let mut world = World::new();
        let e = world
            .spawn((
                physics::ParticleNode::new(common::Vec3::new(0.0, 0.0, 0.0), 1.0, 0, 1),
                sample_chem(80.0), // 80% ATP, above the 50% regen threshold
                metabolism::Health {
                    current: 50.0,
                    max: 100.0,
                },
            ))
            .id();

        world.run_system_once(physiological_state_update_system);

        assert!(world.get::<metabolism::Health>(e).unwrap().current > 50.0);
    }

    #[test]
    fn health_drains_on_critical_atp() {
        let mut world = World::new();
        let e = world
            .spawn((
                physics::ParticleNode::new(common::Vec3::new(0.0, 0.0, 0.0), 1.0, 0, 1),
                sample_chem(5.0), // 5% ATP, below the 10% starvation threshold
                metabolism::Health {
                    current: 50.0,
                    max: 100.0,
                },
            ))
            .id();

        world.run_system_once(physiological_state_update_system);

        assert!(world.get::<metabolism::Health>(e).unwrap().current < 50.0);
    }

    #[test]
    fn health_never_exceeds_max_or_drops_below_zero() {
        let mut world = World::new();
        let full = world
            .spawn((
                physics::ParticleNode::new(common::Vec3::new(0.0, 0.0, 0.0), 1.0, 0, 1),
                sample_chem(100.0),
                metabolism::Health {
                    current: 100.0,
                    max: 100.0,
                },
            ))
            .id();
        let empty = world
            .spawn((
                physics::ParticleNode::new(common::Vec3::new(0.0, 0.0, 0.0), 1.0, 0, 2),
                sample_chem(0.0),
                metabolism::Health {
                    current: 0.0,
                    max: 100.0,
                },
            ))
            .id();

        for _ in 0..10 {
            world.run_system_once(physiological_state_update_system);
        }

        assert_eq!(
            world.get::<metabolism::Health>(full).unwrap().current,
            100.0
        );
        assert_eq!(world.get::<metabolism::Health>(empty).unwrap().current, 0.0);
    }
}
