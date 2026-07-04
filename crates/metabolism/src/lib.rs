//! Energy management, ageing, respiration, starvation, and hunger systems.

#![warn(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::prelude::*;

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
#[derive(Component, Debug, Clone)]
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
}

impl Default for GlobalAtmosphere {
    fn default() -> Self {
        Self {
            o2: 10_000_000.0, // Large starting pool to prevent immediate collapse
            co2: 400.0,
            sunlight: 1.0,
            temp: 22.0,
            ticks: 0,
        }
    }
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
    /// Indicates if the organism is a Producer (autotroph).
    pub is_plant: bool,
}

/// Tracks physical damage and overall vitality.
#[derive(Component, Debug, Clone)]
pub struct Health {
    /// Current health points (e.g. 0.0 to 100.0).
    pub current: f32,
    /// Maximum health points.
    pub max: f32,
}

/// Tracks water levels for ecological rules.
#[derive(Component, Debug, Clone)]
pub struct Hydration {
    /// Current hydration level (0.0 to 1.0).
    pub level: f32,
    /// Rate of water loss per tick.
    pub loss_rate: f32,
}

/// Tracks body temperature for thermoregulation.
#[derive(Component, Debug, Clone)]
pub struct BodyTemperature {
    /// Current body temperature in degrees Celsius.
    pub current: f32,
    /// Ideal body temperature for optimal metabolic function.
    pub ideal: f32,
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
    for (entity, node, mut chem, mut age, metabolism) in query.iter_mut() {
        // Increment age
        age.ticks += 1;

        // 1. Gas Exchange (Organism <-> Atmosphere)
        // Instead of GlobalAtmosphere, we sample the local spatial grid.
        let local_o2 = if let Some(field) = &cpu_field {
            field.sample(node.position, 2)
        } else {
            1000.0 // fallback
        };
        let local_co2 = if let Some(field) = &cpu_field {
            field.sample(node.position, 3)
        } else {
            0.0 // fallback
        };

        // Organisms want to keep their O2 full and CO2 empty.
        let o2_needed = (chem.max_o2 - chem.o2).min(metabolism.mass * 2.0); // Max inhalation rate
        let o2_absorbed = o2_needed.min(local_o2);
        chem.o2 += o2_absorbed;
        atmosphere.o2 = (atmosphere.o2 - o2_absorbed).max(0.0);

        // Exhale CO2 into the shared planetary pool (in addition to the local
        // spatial grid emission handled in simulation.rs) — this is the
        // missing return path that closes the carbon cycle: photosynthesis
        // draws from `GlobalAtmosphere.co2`, so respiration must feed it
        // back or the pool only drains (see corpse_decay_system's outgassing
        // for the other return path).
        let co2_exhale = chem.co2.min(metabolism.mass * 2.0);
        chem.co2 -= co2_exhale;
        atmosphere.co2 += co2_exhale;

        // 2. Cellular Respiration (Glucose + O2 -> ATP + CO2)
        // How much ATP they want to generate to fill their tank
        let atp_needed = chem.max_atp - chem.atp;
        // Limit by available Glucose and O2 (let's say 1 Glucose + 2 O2 -> 5 ATP + 2 CO2)
        // Rate is limited by mass
        let max_reaction = (metabolism.mass * 1.0).min(atp_needed / 5.0);
        let actual_reaction = max_reaction
            .min(chem.glucose)
            .min(chem.o2 / 2.0)
            .min((chem.max_co2 - chem.co2) / 2.0);

        if actual_reaction > 0.0 {
            chem.glucose -= actual_reaction;
            chem.o2 -= actual_reaction * 2.0;
            chem.atp += actual_reaction * 5.0;
            chem.co2 += actual_reaction * 2.0;
        }

        // 3. Basal Metabolic Cost
        // Deduct ATP: superlinear scaling mass^1.2
        let mut active_base_rate = metabolism.base_rate;

        // Phase 2: Metabolic Dormancy (Night/Scarcity Mode)
        if metabolism.is_plant && (atmosphere.sunlight < 0.2 || local_co2 < 10.0) {
            // Sleep through the night or CO2 droughts without burning entire Glucose supply.
            active_base_rate *= 0.2;
        }

        let cost = active_base_rate * metabolism.mass.powf(1.2);
        chem.atp -= cost;

        // Check starvation / suffocation (ATP hit 0)
        if chem.atp <= 0.0 {
            commands.entity(entity).insert(Dead);
            continue;
        }

        // Check old age
        if age.ticks >= age.max_lifespan {
            commands.entity(entity).insert(Dead);
            continue;
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
pub fn day_night_cycle_system(mut atmosphere: ResMut<GlobalAtmosphere>) {
    atmosphere.ticks += 1;
    // frequency = 2 * PI / 3600
    let frequency = std::f32::consts::TAU / 3600.0;
    // ((t * frequency).cos() + 1.0) / 2.0 starts at exactly 1.0 at tick 0
    let t = atmosphere.ticks as f32;
    atmosphere.sunlight = ((t * frequency).cos() + 1.0) / 2.0;
}
