//! # Phylon Sensing
//!
//! All sensor modalities: vision, olfaction, hearing, tactile contact,
//! thermoreception, proprioception, electroreception, and magnetoreception.
//!
//! Sensors read from local field values and nearby entity positions. They
//! produce a flat float vector fed into the neural brain as input.
//!
//! ## Phase 0 scope
//!
//! Sensor modality enum. Implementation: Phase 4.

#![allow(missing_docs)]
#![warn(clippy::all)]

/// The sensory modalities available to organisms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SensorModality {
    Vision,
    Olfaction,
    Hearing,
    Touch,
    Thermoreception,
    Proprioception,
    Baroreception,
    Electroreception,
    Magnetoreception,
    Nociception,
}

/// A component holding the current sensory inputs for an organism.
#[derive(bevy_ecs::prelude::Component, Debug, Clone)]
pub struct SensoryState {
    /// Array of float inputs corresponding to the active sensor modalities.
    pub inputs: Vec<f32>,
}

impl SensoryState {
    /// Creates a new, empty sensory state.
    pub fn new(input_count: usize) -> Self {
        Self {
            inputs: vec![0.0; input_count],
        }
    }
}

/// System that gathers sensory data from the environment and biology.
pub fn sensing_system(
    mut query: bevy_ecs::prelude::Query<(
        &mut SensoryState,
        &physics::ParticleNode,
        Option<&metabolism::Energy>,
        Option<&metabolism::Age>,
    )>,
    cpu_field: Option<bevy_ecs::prelude::Res<diffusion::CpuFieldState>>,
) {
    for (mut state, node, energy_opt, age_opt) in query.iter_mut() {
        if state.inputs.is_empty() {
            continue;
        }

        let mut idx = 0;

        // 1. Chemical sensor (Olfaction) - reads diffusion field
        if let Some(field) = &cpu_field {
            // Very basic: read the exact cell concentration
            let val = field.sample(node.position);
            if idx < state.inputs.len() {
                state.inputs[idx] = val;
                idx += 1;
            }
        }

        // 2. Proprioception (Energy level)
        if let Some(energy) = energy_opt {
            if idx < state.inputs.len() {
                state.inputs[idx] = energy.current / energy.max.max(1.0);
                idx += 1;
            }
        }

        // 3. Proprioception (Age)
        if let Some(age) = age_opt {
            if idx < state.inputs.len() {
                state.inputs[idx] = age.ticks as f32 / age.max_lifespan.max(1) as f32;
            }
        }

        // Additional sensors could be added here
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensor_modality_is_copy() {
        let s = SensorModality::Vision;
        let _s2 = s;
    }
}
