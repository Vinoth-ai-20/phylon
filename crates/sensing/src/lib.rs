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

/// # Ecological Sensory State
///
/// ## 1. What Happens
/// The `SensoryState` component stores the flattened float vector ($\mathbb{R}^N$) representing
/// the organism's current environmental perception.
///
/// ## 2. Why It Happens
/// Neural networks (like CTRNNs or neat-based brains) require normalized numeric arrays as
/// input. We decouple the biological sensors (eyes, noses) from the neural network topology.
/// The biology writes to this array, and the brain reads from it.
///
/// ## 3. How It Happens
/// During the `Sensing` phase, `sensing_system` iterates over active sensor modalities, computes
/// their values (e.g., sampling diffusion fields or raycasting for vision), and overwrites the
/// indices in `inputs`.
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

/// # Organism Visual Cortex
///
/// ## 1. What Happens
/// The `HeadVision` component defines the geometric capabilities of an organism's eyes.
///
/// ## 2. Why It Happens
/// Blind organisms stumble aimlessly. Vision allows targeted predation, foraging, and mating.
/// However, true raycasting is too expensive for thousands of agents. We use a lightweight
/// binned-cone heuristic instead.
///
/// ## 3. How It Happens
/// The system checks the angle to nearby entities. If the entity falls within the `fov` cone,
/// its inverse-distance is accumulated into three bins (Left, Center, Right). This gives the
/// neural network enough gradient information to turn toward or away from a target.
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

/// # Sensory Acquisition System
///
/// ## 1. What Happens
/// The `sensing_system` collects environmental data (Chemical fields, visual targets) and
/// internal data (ATP, Age) and populates the `SensoryState` array for every organism.
///
/// ## 2. Why It Happens
/// The brain needs a snapshot of the world to make decisions. By combining field sampling
/// (for gradients/pheromones), distance-based vision (for hunting/fleeing), and an internal
/// Pacemaker (for gait generation), we provide the necessary basis for complex behavior.
///
/// ## 3. How It Happens
/// For every organism with a `SensoryState`:
/// 1. Sample CPU diffusion fields (Olfaction, Signals, Hazards) at the node's position.
/// 2. Read internal components (`ChemicalEconomy::atp`, `Age::ticks`) for proprioception.
/// 3. Iterate over the spatial index (or all nodes/food/corpses) and bin them into the `HeadVision` FOV.
/// 4. Generate a sine wave `Pacemaker` signal ($\approx 2Hz$) to drive rhythmic locomotion.
#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn sensing_system(
    mut query: bevy_ecs::prelude::Query<(
        &mut SensoryState,
        &physics::ParticleNode,
        Option<&mut HeadVision>,
        Option<&metabolism::ChemicalEconomy>,
        Option<&metabolism::Age>,
        Option<&ecology::Diet>,
    )>,
    node_query: bevy_ecs::prelude::Query<&physics::ParticleNode>,
    diet_query: bevy_ecs::prelude::Query<(&physics::ParticleNode, &ecology::Diet)>,
    food_query: bevy_ecs::prelude::Query<&ecology::FoodPellet>,
    mineral_query: bevy_ecs::prelude::Query<&ecology::MineralPellet>,
    corpse_query: bevy_ecs::prelude::Query<&ecology::Corpse>,
    cpu_field: Option<bevy_ecs::prelude::Res<diffusion::CpuFieldState>>,
    cpu_signal_field: Option<bevy_ecs::prelude::Res<diffusion::CpuSignalFieldState>>,
    cpu_hazard_field: Option<bevy_ecs::prelude::Res<diffusion::CpuHazardFieldState>>,
    mut local_tick: bevy_ecs::prelude::Local<u64>,
) {
    let mut diet_map = std::collections::HashMap::new();
    for (node, diet) in diet_query.iter() {
        diet_map.insert(node.organism_id, diet.clone());
    }

    for (mut state, node, mut vision_opt, energy_opt, age_opt, diet_opt) in query.iter_mut() {
        if state.inputs.is_empty() {
            continue;
        }

        let mut idx = 0;

        // 1. Chemical sensor (Olfaction) - reads diffusion field
        if let Some(field) = &cpu_field {
            // Very basic: read the exact cell concentration
            let val = field.sample(node.position, 0);
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

        // 2. Proprioception (ATP level)
        if let Some(chem) = energy_opt {
            if idx < state.inputs.len() {
                state.inputs[idx] = chem.atp / chem.max_atp.max(1.0);
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

            let mut food_left = 0.0f32;
            let mut food_center = 0.0f32;
            let mut food_right = 0.0f32;

            let mut obs_left = 0.0f32;
            let mut obs_center = 0.0f32;
            let mut obs_right = 0.0f32;

            let mut process_vision_target = |target_pos: common::Vec2, is_food: bool| {
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
                    if is_food {
                        if angle < -third_fov {
                            food_left = food_left.max(strength);
                        } else if angle > third_fov {
                            food_right = food_right.max(strength);
                        } else {
                            food_center = food_center.max(strength);
                        }
                    } else {
                        if angle < -third_fov {
                            obs_left = obs_left.max(strength);
                        } else if angle > third_fov {
                            obs_right = obs_right.max(strength);
                        } else {
                            obs_center = obs_center.max(strength);
                        }
                    }
                }
            };

            // 1. See other organisms (mating, collision avoidance, predation)
            for other_node in node_query.iter() {
                let mut is_food = false;
                if let (Some(my_diet), Some(other_diet)) =
                    (diet_opt, diet_map.get(&other_node.organism_id))
                {
                    is_food = matches!(
                        (my_diet, other_diet),
                        (
                            ecology::Diet::Carnivore,
                            ecology::Diet::Herbivore | ecology::Diet::Omnivore
                        ) | (
                            ecology::Diet::Herbivore | ecology::Diet::Omnivore,
                            ecology::Diet::Producer
                        )
                    );
                }
                process_vision_target(other_node.position, is_food);
            }

            // 2. Diet-specific target vision
            if let Some(diet) = diet_opt {
                match diet {
                    ecology::Diet::Producer => {
                        for mineral in mineral_query.iter() {
                            process_vision_target(mineral.position, true);
                        }
                    }
                    ecology::Diet::Herbivore | ecology::Diet::Omnivore => {
                        for food in food_query.iter() {
                            process_vision_target(food.position, true);
                        }
                    }
                    ecology::Diet::Decomposer => {
                        for corpse in corpse_query.iter() {
                            process_vision_target(corpse.position, true);
                        }
                    }
                    ecology::Diet::Carnivore => {
                        // Carnivores look at other organisms which is already done above.
                    }
                }
            }

            if idx < state.inputs.len() {
                state.inputs[idx] = food_left - obs_left;
                idx += 1;
            }
            if idx < state.inputs.len() {
                state.inputs[idx] = food_center - obs_center;
                idx += 1;
            }
            if idx < state.inputs.len() {
                state.inputs[idx] = food_right - obs_right;
                idx += 1;
            }

            // 7. Internal Pacemaker (CPG)
            if idx < state.inputs.len() {
                // Since this runs once per tick, local_tick corresponds to elapsed ticks.
                // At 60 ticks/sec, * 0.2 gives ~2 Hz frequency.
                let pacemaker_signal = (*local_tick as f32 * 0.2).sin();
                state.inputs[idx] = pacemaker_signal;
            }
        }
    }

    // Advance the pacemaker tick globally once per frame
    *local_tick += 1;
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
