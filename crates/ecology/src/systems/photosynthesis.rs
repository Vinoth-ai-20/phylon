use crate::components::Diet;
use bevy_ecs::prelude::*;

/// # Autotrophic Energy Generation System
///
/// ## 1. What Happens
/// The `photosynthesis_system` allows organisms with the `Diet::Producer` trait to passively
/// convert ambient `GlobalAtmosphere.sunlight` and `GlobalAtmosphere.co2` directly into
/// structural `Glucose` and respired $O_2$.
///
/// ## 2. Why It Happens
/// The food web must have a foundational energy source. In Earth's biosphere, this is solar
/// irradiance. This system injects new biomass into the economy. However, to prevent runaway
/// infinite growth, the conversion is strictly bottlenecked by the availability of atmospheric $CO_2$.
///
/// ## 3. How It Happens
/// Every tick, a Producer requests a carbon volume proportional to its mass ($M$) and the
/// available sunlight ($S$):
///
/// $$ CO_{2_{req}} = 4.0 \times M \times S $$
///
/// To prevent a "Carbon Leak" where plants delete carbon by over-eating when full, the requested
/// $CO_2$ is clamped to the available space in the organism's glucose tank:
///
/// $$ \Delta CO_2 = \min(CO_{2_{req}}, G_{max} - G_{current}, CO_{2_{atmosphere}}) $$
///
/// The $\Delta CO_2$ is subtracted from the atmosphere, and the organism's glucose and $O_2$
/// are incremented by the same amount (a 1:1 simplified stoichiometric ratio).
pub fn photosynthesis_system(
    mut atmosphere: ResMut<metabolism::GlobalAtmosphere>,
    mut query: Query<(
        &Diet,
        &metabolism::Metabolism,
        &mut metabolism::ChemicalEconomy,
    )>,
) {
    let sunlight = atmosphere.sunlight;

    for (diet, metabolism, mut chem) in query.iter_mut() {
        if *diet == Diet::Producer && chem.atp > 0.0 {
            // Plants consume CO2 and Sunlight to make Glucose and O2
            let mut co2_needed = 4.0 * metabolism.mass * sunlight;

            // Phase 3: Stop the Carbon Leak
            // Do not absorb CO2 if the Glucose tank is full, otherwise the carbon is deleted.
            let glucose_room = (chem.max_glucose - chem.glucose).max(0.0);
            co2_needed = co2_needed.min(glucose_room);

            let actual_co2 = atmosphere.co2.min(co2_needed);
            atmosphere.co2 -= actual_co2;

            // 1 CO2 -> 1 Glucose + 1 O2 (simplified). O2 output feeds back
            // into the shared atmosphere pool as well as the organism's own
            // tank, closing the loop with metabolism_system's O2 draw.
            chem.glucose = (chem.glucose + actual_co2).min(chem.max_glucose);
            chem.o2 = (chem.o2 + actual_co2).min(chem.max_o2);
            atmosphere.o2 += actual_co2;
        }
    }
}
