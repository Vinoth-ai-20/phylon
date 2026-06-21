//! Energy management, ageing, respiration, starvation, and hunger systems.

#![warn(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::prelude::*;

/// Tracks the chemical economy of an organism.
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
    /// Absolute ticks elapsed. Used for the Day/Night cycle.
    pub ticks: u64,
}

impl Default for GlobalAtmosphere {
    fn default() -> Self {
        Self {
            o2: 10_000_000.0, // Large starting pool to prevent immediate collapse
            co2: 5_000_000.0,
            sunlight: 1.0,
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
    mut atmosphere: ResMut<GlobalAtmosphere>,
    mut query: Query<(Entity, &mut ChemicalEconomy, &mut Age, &Metabolism)>,
) {
    for (entity, mut chem, mut age, metabolism) in query.iter_mut() {
        // Increment age
        age.ticks += 1;

        // 1. Gas Exchange (Organism <-> Atmosphere)
        // Organisms want to keep their O2 full and CO2 empty.
        let o2_needed = (chem.max_o2 - chem.o2).min(metabolism.mass * 2.0); // Max inhalation rate
        if atmosphere.o2 >= o2_needed {
            atmosphere.o2 -= o2_needed;
            chem.o2 += o2_needed;
        } else {
            chem.o2 += atmosphere.o2;
            atmosphere.o2 = 0.0;
        }

        // Exhale CO2
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
        let cost = metabolism.base_rate * metabolism.mass.powf(1.2);
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
