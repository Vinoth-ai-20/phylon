//! Energy management, ageing, respiration, starvation, and hunger systems.

#![warn(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

/// # Cellular Chemical Economy
///
/// ## 1. What Happens
/// The `ChemicalEconomy` component acts as the biological ledger for an organism, tracking the
/// internal storage of four primary resources: Glucose, Oxygen, Carbon Dioxide, and ATP.
///
/// ## 2. Why It Happens
/// Simple ALife simulations just use a single "Energy" scalar. Real biology uses a multi-step
/// conversion process. By splitting energy into stored mass (Glucose), volatile fuel (ATP), and
/// gas dependencies ($O_2$/$CO_2$), the engine naturally emerges distinct ecological niches:
/// anaerobic vs aerobic organisms, suffocation from algal blooms, and starvation.
///
/// ## 3. How It Happens
/// Every tick, the `metabolism_system` attempts to perform cellular respiration, converting
/// Glucose and Oxygen into ATP and Carbon Dioxide. If ATP drops to $0.0$, the organism dies.
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct ChemicalEconomy {
    /// Glucose: Raw fuel acquired by eating or photosynthesis.
    pub glucose: f32,
    /// Oxygen: Acquired from the environment.
    pub o2: f32,
    /// Carbon Dioxide: Waste product of respiration.
    pub co2: f32,
    /// ATP: Usable energy for survival and movement.
    pub atp: f32,

    /// Max Glucose capacity.
    pub max_glucose: f32,
    /// Max Oxygen capacity.
    pub max_o2: f32,
    /// Max Carbon Dioxide capacity.
    pub max_co2: f32,
    /// Max ATP capacity.
    pub max_atp: f32,
}

impl ChemicalEconomy {
    /// A small, per-body-segment resource pool (Phase 4, `PHASE4_ROADMAP.md`
    /// milestone P4-F2) — deliberately much smaller than an organism's own
    /// head-level pool (used organism-wide by `metabolism_system`,
    /// `reproduction`, and `ecology::foraging_system`, all unaffected by
    /// this milestone). These placeholder values are not tuned; they exist
    /// so a future intra-body transport pass (P4-F3) has real per-segment
    /// state to move resources between. `metabolism_system`'s query also
    /// requires `&Age`/`&Metabolism`, which only the head entity carries —
    /// so a segment with just this component is never picked up by it,
    /// confirmed by this crate's own existing test suite still passing
    /// unmodified after this milestone.
    pub fn segment_default() -> Self {
        Self {
            glucose: 100.0,
            o2: 50.0,
            co2: 0.0,
            atp: 100.0,
            max_glucose: 200.0,
            max_o2: 100.0,
            max_co2: 100.0,
            max_atp: 200.0,
        }
    }
}

/// Global planetary atmosphere.
#[derive(Resource, Debug, Clone)]
pub struct GlobalAtmosphere {
    /// Total planetary Oxygen.
    pub o2: f32,
    /// Total planetary Carbon Dioxide.
    pub co2: f32,
    /// Current normalized sunlight (0.0 to 1.0).
    pub sunlight: f32,
    /// Global ambient temperature.
    pub temp: f32,
    /// Absolute ticks elapsed. Used for the Day/Night cycle.
    pub ticks: u64,
    /// Seasonal phase in `[0, 1]`, following `sunlight`'s own convention:
    /// `1.0` is midsummer (peak), `0.0` is midwinter (trough). Modulates
    /// `sunlight`'s peak amplitude on a much slower cycle than `sunlight`
    /// itself oscillates on (see [`day_night_cycle_system`]'s doc comment
    /// for both periods). Also read by `ecology::food_spawner_system` to
    /// scale biome fertility seasonally.
    pub season: f32,
}

impl Default for GlobalAtmosphere {
    fn default() -> Self {
        Self {
            o2: 10_000_000.0, // Large starting pool to prevent immediate collapse
            co2: 400.0,
            sunlight: 1.0,
            temp: 22.0,
            ticks: 0,
            season: 1.0, // matches sunlight's tick-0 default of full brightness
        }
    }
}

/// Tracks the age of an organism.
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct Age {
    /// Number of ticks lived.
    pub ticks: u64,
    /// Maximum lifespan in ticks before senescence.
    pub max_lifespan: u64,
}

/// Defines the baseline metabolic cost per tick.
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct Metabolism {
    /// The abstract mass of the organism (sum of its nodes).
    pub mass: f32,
    /// The base cost multiplier per tick.
    pub base_rate: f32,
    /// Indicates if the organism is a Producer (autotroph).
    pub is_plant: bool,
}

/// Tracks physical damage and overall vitality.
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct Health {
    /// Current health points (e.g. 0.0 to 100.0).
    pub current: f32,
    /// Maximum health points.
    pub max: f32,
}

/// Tracks water levels for ecological rules.
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct Hydration {
    /// Current hydration level (0.0 to 1.0).
    pub level: f32,
    /// Rate of water loss per tick.
    pub loss_rate: f32,
}

/// Tracks body temperature for thermoregulation.
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct BodyTemperature {
    /// Current body temperature in degrees Celsius.
    pub current: f32,
    /// Ideal body temperature for optimal metabolic function.
    pub ideal: f32,
}

/// Per-entity result of the parallel metabolism computation — pure data,
/// no shared state — produced by [`compute_metabolism`] and applied back
/// to the ECS by `metabolism_system` in a fixed, deterministic order. See
/// `metabolism_system`'s doc comment for why the parallel/sequential split
/// is shaped this way.
struct MetabolismResult {
    entity: Entity,
    new_glucose: f32,
    new_o2: f32,
    new_co2: f32,
    new_atp: f32,
    new_age_ticks: u64,
    /// Amount subtracted from `GlobalAtmosphere.o2` for this entity's
    /// inhalation (always ≥ 0).
    atmosphere_o2_consumed: f32,
    /// Amount added to `GlobalAtmosphere.co2` for this entity's exhalation
    /// (always ≥ 0).
    atmosphere_co2_exhaled: f32,
    should_die: bool,
}

/// Pure per-entity metabolism computation — reads only its own snapshot of
/// component state plus the two read-only environment values (`sunlight`,
/// the sampled local field), and touches no shared mutable state. Safe to
/// call from any thread; `metabolism_system` runs this via `rayon`'s
/// `par_iter`.
#[allow(clippy::too_many_arguments)]
fn compute_metabolism(
    entity: Entity,
    chem: &ChemicalEconomy,
    age_ticks: u64,
    max_lifespan: u64,
    metabolism: &Metabolism,
    local_o2: f32,
    local_co2: f32,
    sunlight: f32,
) -> MetabolismResult {
    let mut chem_glucose = chem.glucose;
    let mut chem_o2 = chem.o2;
    let mut chem_co2 = chem.co2;
    let mut chem_atp = chem.atp;
    let new_age_ticks = age_ticks + 1;

    // 1. Gas Exchange (Organism <-> Atmosphere)
    // Instead of GlobalAtmosphere, we sample the local spatial grid.
    let o2_needed = (chem.max_o2 - chem_o2).min(metabolism.mass * 2.0); // Max inhalation rate
    let o2_absorbed = o2_needed.min(local_o2);
    chem_o2 += o2_absorbed;

    // Exhale CO2 into the shared planetary pool (in addition to the local
    // spatial grid emission handled in simulation.rs) — this is the
    // missing return path that closes the carbon cycle: photosynthesis
    // draws from `GlobalAtmosphere.co2`, so respiration must feed it
    // back or the pool only drains (see corpse_decay_system's outgassing
    // for the other return path).
    let co2_exhale = chem_co2.min(metabolism.mass * 2.0);
    chem_co2 -= co2_exhale;

    // 2. Cellular Respiration (Glucose + O2 -> ATP + CO2)
    // How much ATP they want to generate to fill their tank
    let atp_needed = chem.max_atp - chem_atp;
    // Limit by available Glucose and O2 (let's say 1 Glucose + 2 O2 -> 5 ATP + 2 CO2)
    // Rate is limited by mass
    let max_reaction = (metabolism.mass * 1.0).min(atp_needed / 5.0);
    let actual_reaction = max_reaction
        .min(chem_glucose)
        .min(chem_o2 / 2.0)
        .min((chem.max_co2 - chem_co2) / 2.0);

    if actual_reaction > 0.0 {
        chem_glucose -= actual_reaction;
        chem_o2 -= actual_reaction * 2.0;
        chem_atp += actual_reaction * 5.0;
        chem_co2 += actual_reaction * 2.0;
    }

    // 3. Basal Metabolic Cost
    // Deduct ATP: superlinear scaling mass^1.2
    let mut active_base_rate = metabolism.base_rate;

    // Phase 2: Metabolic Dormancy (Night/Scarcity Mode)
    if metabolism.is_plant && (sunlight < 0.2 || local_co2 < 10.0) {
        // Sleep through the night or CO2 droughts without burning entire Glucose supply.
        active_base_rate *= 0.2;
    }

    let cost = active_base_rate * metabolism.mass.powf(1.2);
    chem_atp -= cost;

    // Check starvation/suffocation (ATP hit 0) or old age.
    let should_die = chem_atp <= 0.0 || new_age_ticks >= max_lifespan;

    MetabolismResult {
        entity,
        new_glucose: chem_glucose,
        new_o2: chem_o2,
        new_co2: chem_co2,
        new_atp: chem_atp,
        new_age_ticks,
        atmosphere_o2_consumed: o2_absorbed,
        atmosphere_co2_exhaled: co2_exhale,
        should_die,
    }
}

/// # Cellular Respiration and Aging System
///
/// ## 1. What Happens
/// The `metabolism_system` ticks the biological clock for all organisms. It handles gas exchange
/// with the local spatial PDE grid, converts Glucose to ATP via respiration, and deducts the basal
/// metabolic cost required to stay alive.
///
/// ## 2. Why It Happens
/// Thermodynamics dictates that organization requires energy. Without a continuous basal cost,
/// organisms could sit perfectly still forever. The super-linear scaling of mass to cost prevents
/// the evolution of infinitely large organisms, enforcing physical tradeoffs.
///
/// ## 3. How It Happens
/// The system executes 3 phases per organism:
/// 1. **Gas Exchange**: Samples $O_2$ from `CpuFieldState` and fills internal lungs.
/// 2. **Respiration**: Converts $1G + 2O_2 \to 5ATP + 2CO_2$, limited by mass-based reaction rates.
/// 3. **Basal Cost**: Deducts ATP using Kleiber's Law (super-linear metabolic scaling):
///
/// $$ ATP_{cost} = \text{base\_rate} \times M^{1.2} $$
///
/// If $ATP \le 0.0$ or $Age \ge \text{max\_lifespan}$, the entity is marked `Dead`.
///
/// ## Parallel/sequential split (determinism)
///
/// Each organism's own biology (steps 1-3 above) depends only on its own
/// components plus two read-only environment values (`sunlight`, the
/// sampled local field) — no organism's computation depends on any other
/// organism's result, so this half is computed in parallel via `rayon`
/// (`compute_metabolism`).
///
/// The one piece of genuinely shared state is `GlobalAtmosphere.o2`/`.co2`,
/// which every organism accumulates into. Floating-point addition is not
/// associative, so summing these contributions in whatever order threads
/// happen to finish would make the final atmosphere value (and everything
/// that reads it) depend on scheduling — a real determinism hazard, not a
/// hypothetical one. This system avoids it by collecting per-entity results
/// into a `Vec` that preserves the original query iteration order (`rayon`'s
/// indexed `map`/`collect` guarantees this regardless of which thread
/// computed which element), then applying every mutation — component
/// writeback, atmosphere accumulation, `Dead` marking — in a single
/// sequential pass over that ordered `Vec`. The result is bit-identical to
/// the pre-parallelization implementation for a given world state, whether
/// run with one thread or many (see the crate's `metabolism_is_deterministic_regardless_of_thread_count`
/// test).
pub fn metabolism_system(
    mut commands: Commands,
    mut atmosphere: ResMut<GlobalAtmosphere>,
    cpu_field: Option<Res<diffusion::CpuFieldState>>,
    mut query: Query<(
        Entity,
        &physics::ParticleNode,
        &mut ChemicalEconomy,
        &mut Age,
        &Metabolism,
    )>,
) {
    use rayon::prelude::*;

    let sunlight = atmosphere.sunlight;

    // Snapshot phase (sequential, cheap): gather each entity's own component
    // values plus its local field sample. Iteration order here is
    // `bevy_ecs`'s normal deterministic archetype order — the same order
    // the reduction phase below replays.
    let snapshots: Vec<_> = query
        .iter()
        .map(|(entity, node, chem, age, metabolism)| {
            let local_o2 = cpu_field
                .as_ref()
                .map_or(1000.0, |field| field.sample(node.position, 2));
            let local_co2 = cpu_field
                .as_ref()
                .map_or(0.0, |field| field.sample(node.position, 3));
            (
                entity,
                chem.clone(),
                age.ticks,
                age.max_lifespan,
                metabolism.clone(),
                local_o2,
                local_co2,
            )
        })
        .collect();

    // Parallel phase: pure per-entity computation, no shared state touched.
    let results: Vec<MetabolismResult> = snapshots
        .par_iter()
        .map(
            |(entity, chem, age_ticks, max_lifespan, metabolism, local_o2, local_co2)| {
                compute_metabolism(
                    *entity,
                    chem,
                    *age_ticks,
                    *max_lifespan,
                    metabolism,
                    *local_o2,
                    *local_co2,
                    sunlight,
                )
            },
        )
        .collect();

    // Sequential reduction phase: apply every result in the same fixed
    // order every time, regardless of thread count — see this function's
    // doc comment.
    for result in results {
        atmosphere.o2 = (atmosphere.o2 - result.atmosphere_o2_consumed).max(0.0);
        atmosphere.co2 += result.atmosphere_co2_exhaled;

        // Write the final tick's values back regardless of `should_die` —
        // matches the pre-parallelization behavior, which mutated `chem`/
        // `age` in place before checking whether the entity died this
        // tick, not conditionally on survival.
        if let Ok((_, _, mut chem, mut age, _)) = query.get_mut(result.entity) {
            chem.glucose = result.new_glucose;
            chem.o2 = result.new_o2;
            chem.co2 = result.new_co2;
            chem.atp = result.new_atp;
            age.ticks = result.new_age_ticks;
        }

        if result.should_die {
            commands.entity(result.entity).insert(Dead);
        }
    }
}

/// Marker component for dead organisms. App logic should catch this to clean up the physical body.
#[derive(Component)]
pub struct Dead;

/// # Atmospheric Homeostasis
///
/// ## 1. What Happens
/// Slowly drifts `GlobalAtmosphere.co2`/`o2` toward their starting-baseline levels each tick.
///
/// ## 2. Why It Happens
/// Respiration and photosynthesis exchange gas through the shared pool, but a transient
/// imbalance — e.g. a population crash leaving no respirators to replenish `co2` — could
/// otherwise drain it to zero and permanently stall photosynthesis with no organisms left to
/// recover it. This models a minimal planetary buffer (geological/oceanic carbon reservoirs)
/// standing in for processes this simulation doesn't otherwise model.
///
/// ## 3. How It Happens
/// Each tick, both gases move a small fraction of the way toward their baseline:
///
/// $$ X_{new} = X + (X_{baseline} - X) \times \text{drift\_rate} $$
pub fn atmosphere_homeostasis_system(mut atmosphere: ResMut<GlobalAtmosphere>) {
    const CO2_BASELINE: f32 = 400.0;
    const O2_BASELINE: f32 = 10_000_000.0;
    const DRIFT_RATE: f32 = 0.005;

    atmosphere.co2 += (CO2_BASELINE - atmosphere.co2) * DRIFT_RATE;
    atmosphere.o2 += (O2_BASELINE - atmosphere.o2) * DRIFT_RATE;
}

/// Evaluates the day/night cycle using a shifted cosine wave.
/// A full cycle takes exactly 60 seconds (3600 ticks at 60 Hz).
/// We start at High Noon (1.0) using a cosine function, as requested by the user.
///
/// Layered on top is a **seasonal** cycle (`atmosphere.season`) 10x longer
/// (36,000 ticks — 10 minutes at 60 Hz), standing in for a "year" made of
/// many day/night cycles: it dampens (never zeroes) the day/night cycle's
/// peak sunlight during its winter half, so summer days are brighter than
/// winter days on top of the existing day/night swing.
///
/// Also derives ambient `temp` from `sunlight` — more sun means a warmer
/// atmosphere. `temp` eases toward that sunlight-driven target each tick
/// (`THERMAL_LAG`) rather than snapping straight to it, modeling thermal
/// inertia (air/land don't heat or cool instantly) so temperature is a
/// smoothed, physically-motivated function of daylight instead of a static
/// unused constant.
pub fn day_night_cycle_system(mut atmosphere: ResMut<GlobalAtmosphere>) {
    atmosphere.ticks += 1;
    let t = atmosphere.ticks as f32;

    // Season: same shifted-cosine shape as the day/night cycle below, just
    // an order of magnitude slower.
    const SEASON_PERIOD_TICKS: f32 = 36_000.0;
    let season_frequency = std::f32::consts::TAU / SEASON_PERIOD_TICKS;
    atmosphere.season = ((t * season_frequency).cos() + 1.0) / 2.0;
    // Winter dampens peak sunlight to 50%, never all the way to darkness —
    // nights stay dark year-round; only the *daytime* peak shifts.
    let season_amplitude = 0.5 + 0.5 * atmosphere.season;

    // frequency = 2 * PI / 3600
    let frequency = std::f32::consts::TAU / 3600.0;
    // ((t * frequency).cos() + 1.0) / 2.0 starts at exactly 1.0 at tick 0
    atmosphere.sunlight = season_amplitude * ((t * frequency).cos() + 1.0) / 2.0;

    const TEMP_MIN: f32 = 12.0; // night-time low, °C
    const TEMP_MAX: f32 = 30.0; // midday high, °C
    const THERMAL_LAG: f32 = 0.02; // fraction of the gap closed per tick
    let target_temp = TEMP_MIN + (TEMP_MAX - TEMP_MIN) * atmosphere.sunlight;
    atmosphere.temp += (target_temp - atmosphere.temp) * THERMAL_LAG;
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;

    #[test]
    fn segment_default_is_smaller_than_a_typical_organism_pool() {
        // Not a tuning assertion — just confirms the documented intent
        // (a genuinely small per-segment pool, not an organism-sized one)
        // holds, so a future accidental "make it organism-sized" edit
        // would fail loudly.
        let segment = ChemicalEconomy::segment_default();
        assert!(segment.max_glucose < 1000.0);
        assert!(segment.max_atp < 1000.0);
        assert!(segment.glucose <= segment.max_glucose);
        assert!(segment.atp <= segment.max_atp);
    }

    #[test]
    fn metabolism_system_ignores_a_segment_missing_age_and_metabolism() {
        // Phase 4, P4-F2: a plain `ChemicalEconomy` (no `Age`/`Metabolism`)
        // — exactly what a non-head body segment now carries — must not be
        // picked up by `metabolism_system`'s query, so per-segment pools
        // introduced by this milestone can never accidentally trigger
        // organism-level death/ageing logic.
        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(GlobalAtmosphere::default());
        world.spawn((
            physics::ParticleNode::new(common::Vec2::new(0.0, 0.0), 1.0, 1, 0),
            ChemicalEconomy::segment_default(),
        ));
        // Must not panic on a query mismatch; ticking should simply skip it.
        world.run_system_once(metabolism_system);
        assert_eq!(
            world
                .query::<&ChemicalEconomy>()
                .iter(&world)
                .next()
                .unwrap()
                .atp,
            ChemicalEconomy::segment_default().atp,
            "a segment-only entity's economy must be left untouched by metabolism_system"
        );
    }

    fn build_world_with_organisms(n: u32) -> bevy_ecs::world::World {
        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(GlobalAtmosphere::default());
        for i in 0..n {
            world.spawn((
                physics::ParticleNode::new(common::Vec2::new(i as f32 * 3.0, 0.0), 1.0, 0, i),
                ChemicalEconomy {
                    glucose: 500.0 + i as f32,
                    o2: 300.0,
                    co2: 50.0,
                    atp: 400.0,
                    max_glucose: 1000.0,
                    max_o2: 1000.0,
                    max_co2: 1000.0,
                    max_atp: 1000.0,
                },
                Age {
                    ticks: 0,
                    max_lifespan: 10_000,
                },
                Metabolism {
                    mass: 5.0 + (i as f32 * 0.1),
                    base_rate: 0.01,
                    is_plant: i % 3 == 0,
                },
            ));
        }
        world
    }

    /// (glucose, o2, co2, atp, age_ticks) for one organism after a
    /// `metabolism_system` tick.
    type OrganismOutcome = (f32, f32, f32, f32, u64);

    /// Runs `metabolism_system` once, inside a `rayon` thread pool with a
    /// caller-chosen thread count, and returns the resulting atmosphere
    /// gases plus every organism's final chemical-economy/age state (sorted
    /// by entity index, so the comparison below doesn't depend on
    /// `bevy_ecs`'s own entity storage order).
    fn run_metabolism_with_thread_count(
        n_threads: usize,
        organism_count: u32,
    ) -> (f32, f32, Vec<OrganismOutcome>) {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(n_threads)
            .build()
            .unwrap();
        let mut world = build_world_with_organisms(organism_count);

        pool.install(|| {
            world.run_system_once(metabolism_system);
        });

        let atmosphere = world.resource::<GlobalAtmosphere>();
        let (o2, co2) = (atmosphere.o2, atmosphere.co2);

        let mut query = world.query::<(&physics::ParticleNode, &ChemicalEconomy, &Age)>();
        let mut results: Vec<_> = query
            .iter(&world)
            .map(|(node, chem, age)| {
                (
                    node.organism_id,
                    (chem.glucose, chem.o2, chem.co2, chem.atp, age.ticks),
                )
            })
            .collect();
        results.sort_by_key(|(id, _)| *id);

        (o2, co2, results.into_iter().map(|(_, r)| r).collect())
    }

    #[test]
    fn metabolism_is_deterministic_regardless_of_thread_count() {
        // 200 organisms is enough to actually spread across multiple rayon
        // worker threads, not just fit in one batch.
        let (o2_1, co2_1, results_1) = run_metabolism_with_thread_count(1, 200);
        let (o2_8, co2_8, results_8) = run_metabolism_with_thread_count(8, 200);

        assert_eq!(
            o2_1, o2_8,
            "GlobalAtmosphere.o2 diverged between 1 and 8 threads"
        );
        assert_eq!(
            co2_1, co2_8,
            "GlobalAtmosphere.co2 diverged between 1 and 8 threads"
        );
        assert_eq!(
            results_1, results_8,
            "per-organism chemical economy/age diverged between 1 and 8 threads"
        );
    }

    #[test]
    fn compute_metabolism_marks_death_on_atp_depletion() {
        let chem = ChemicalEconomy {
            glucose: 0.0,
            o2: 0.0,
            co2: 0.0,
            atp: 0.001,
            max_glucose: 1000.0,
            max_o2: 1000.0,
            max_co2: 1000.0,
            max_atp: 1000.0,
        };
        let metabolism = Metabolism {
            mass: 10.0,
            base_rate: 1.0,
            is_plant: false,
        };
        let result = compute_metabolism(
            Entity::from_raw(0),
            &chem,
            0,
            10_000,
            &metabolism,
            0.0,
            0.0,
            1.0,
        );
        assert!(result.should_die);
    }

    #[test]
    fn compute_metabolism_marks_death_on_old_age() {
        let chem = ChemicalEconomy {
            glucose: 1000.0,
            o2: 1000.0,
            co2: 0.0,
            atp: 1000.0,
            max_glucose: 1000.0,
            max_o2: 1000.0,
            max_co2: 1000.0,
            max_atp: 1000.0,
        };
        let metabolism = Metabolism {
            mass: 1.0,
            base_rate: 0.001,
            is_plant: false,
        };
        let result = compute_metabolism(
            Entity::from_raw(0),
            &chem,
            999,
            1000,
            &metabolism,
            1000.0,
            0.0,
            1.0,
        );
        assert!(result.should_die);
        assert_eq!(result.new_age_ticks, 1000);
    }

    #[test]
    fn season_dampens_sunlight_peak_in_winter_but_never_reaches_night_darkness() {
        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(GlobalAtmosphere::default());

        // Run to the seasonal trough (roughly half the season period) and
        // check we land near the day/night cycle's own peak-sunlight tick
        // (a multiple of 3600) so we're comparing "winter noon" against
        // "summer noon", not two arbitrary phases of the day.
        let mut world_summer = bevy_ecs::world::World::new();
        world_summer.insert_resource(GlobalAtmosphere::default());
        for _ in 0..3600 {
            world_summer.run_system_once(day_night_cycle_system);
        }
        let summer_noon_sunlight = world_summer.resource::<GlobalAtmosphere>().sunlight;

        for _ in 0..18000 {
            // half of the 36,000-tick season period
            world.run_system_once(day_night_cycle_system);
        }
        for _ in 0..3600 {
            // advance to the next day/night noon from this point
            world.run_system_once(day_night_cycle_system);
        }
        let winter_noon_sunlight = world.resource::<GlobalAtmosphere>().sunlight;

        assert!(
            winter_noon_sunlight < summer_noon_sunlight,
            "winter noon ({winter_noon_sunlight}) should be dimmer than summer noon ({summer_noon_sunlight})"
        );
        assert!(
            winter_noon_sunlight > 0.0,
            "winter noon should never be fully dark"
        );
    }

    #[test]
    fn season_and_sunlight_stay_in_valid_range_over_a_full_cycle() {
        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(GlobalAtmosphere::default());
        for _ in 0..36_000 {
            world.run_system_once(day_night_cycle_system);
            let atmosphere = world.resource::<GlobalAtmosphere>();
            assert!((0.0..=1.0).contains(&atmosphere.season));
            assert!((0.0..=1.0).contains(&atmosphere.sunlight));
        }
    }
}
