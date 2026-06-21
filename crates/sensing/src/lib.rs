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
    Signal,
    Hazard,
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

/// A component attached to the Head node of an organism, defining its vision capabilities.
#[derive(bevy_ecs::prelude::Component, Debug, Clone)]
pub struct HeadVision {
    /// Maximum distance the organism can see.
    pub range: f32,
    /// Field of view angle in radians.
    pub fov: f32,
    /// Last known forward direction (used when velocity is near zero).
    pub last_forward: common::Vec2,
    /// Distance within which nodes are ignored (self-occlusion heuristic).
    pub self_occlusion_radius: f32,
}

/// System that gathers sensory data from the environment and biology.
#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn sensing_system(
    mut query: bevy_ecs::prelude::Query<(
        &mut SensoryState,
        &physics::ParticleNode,
        Option<&mut HeadVision>,
        Option<&metabolism::Energy>,
        Option<&metabolism::Age>,
        Option<&ecology::Diet>,
    )>,
    node_query: bevy_ecs::prelude::Query<&physics::ParticleNode>,
    food_query: bevy_ecs::prelude::Query<&ecology::FoodPellet>,
    mineral_query: bevy_ecs::prelude::Query<&ecology::MineralPellet>,
    corpse_query: bevy_ecs::prelude::Query<&ecology::Corpse>,
    cpu_field: Option<bevy_ecs::prelude::Res<diffusion::CpuFieldState>>,
    cpu_signal_field: Option<bevy_ecs::prelude::Res<diffusion::CpuSignalFieldState>>,
    cpu_hazard_field: Option<bevy_ecs::prelude::Res<diffusion::CpuHazardFieldState>>,
) {
    for (mut state, node, mut vision_opt, energy_opt, age_opt, diet_opt) in query.iter_mut() {
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

        // 1.5. Signal sensor - reads emergent signal field
        if let Some(field) = &cpu_signal_field {
            let val = field.sample(node.position);
            if idx < state.inputs.len() {
                state.inputs[idx] = val;
                idx += 1;
            }
        }

        // 1.6. Hazard sensor - reads "impending doom" field
        if let Some(field) = &cpu_hazard_field {
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
                idx += 1;
            }
        }

        // 4, 5, 6. Vision (Left, Center, Right bins)
        if let Some(vision) = &mut vision_opt {
            // Update forward direction based on velocity
            if node.velocity.length_squared() > 0.01 {
                vision.last_forward = node.velocity.normalize();
            } else if vision.last_forward.length_squared() < 0.01 {
                vision.last_forward = common::Vec2::X; // Fallback
            }
            let forward = vision.last_forward;

            let mut left_val = 0.0f32;
            let mut center_val = 0.0f32;
            let mut right_val = 0.0f32;

            let mut process_vision_target = |target_pos: common::Vec2| {
                let diff = target_pos - node.position;
                let dist = diff.length();

                // Ignore self (heuristic), very close nodes, or nodes beyond range
                if dist < vision.self_occlusion_radius || dist > vision.range {
                    return;
                }

                let dir = diff / dist;
                // Angle between forward and dir
                let angle = forward.angle_to(dir);

                // If within FOV
                let half_fov = vision.fov / 2.0;
                if angle >= -half_fov && angle <= half_fov {
                    // Vision strength is inverse to distance
                    let strength = 1.0 - (dist / vision.range);

                    let third_fov = half_fov / 1.5; // Divide FOV into 3 bins
                    if angle < -third_fov {
                        left_val = left_val.max(strength);
                    } else if angle > third_fov {
                        right_val = right_val.max(strength);
                    } else {
                        center_val = center_val.max(strength);
                    }
                }
            };

            // 1. See other organisms (mating, collision avoidance, predation)
            for other_node in node_query.iter() {
                process_vision_target(other_node.position);
            }

            // 2. Diet-specific target vision
            if let Some(diet) = diet_opt {
                match diet {
                    ecology::Diet::Producer => {
                        for mineral in mineral_query.iter() {
                            process_vision_target(mineral.position);
                        }
                    }
                    ecology::Diet::Herbivore | ecology::Diet::Omnivore => {
                        for food in food_query.iter() {
                            process_vision_target(food.position);
                        }
                    }
                    ecology::Diet::Decomposer => {
                        for corpse in corpse_query.iter() {
                            process_vision_target(corpse.position);
                        }
                    }
                    ecology::Diet::Carnivore => {
                        // Carnivores look at other organisms which is already done above.
                    }
                }
            }

            if idx < state.inputs.len() {
                state.inputs[idx] = left_val;
                idx += 1;
            }
            if idx < state.inputs.len() {
                state.inputs[idx] = center_val;
                idx += 1;
            }
            if idx < state.inputs.len() {
                state.inputs[idx] = right_val;
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
