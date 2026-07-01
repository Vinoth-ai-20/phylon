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
    atmosphere: Option<bevy_ecs::prelude::Res<metabolism::GlobalAtmosphere>>,
    env: Option<bevy_ecs::prelude::Res<environment::EnvironmentManager>>,
    mut local_tick: bevy_ecs::prelude::Local<u64>,
) {
    let mut diet_map = std::collections::HashMap::new();
    for (node, diet) in diet_query.iter() {
        diet_map.insert(node.organism_id, diet.clone());
    }

    for (mut state, node, mut vision_opt, energy_opt, age_opt, diet_opt) in query.iter_mut() {
        if state.inputs.len() < 15 {
            continue; // Safety fallback
        }

        for i in 0..state.inputs.len() {
            state.inputs[i] = -1.0; // Default to -1.0 (empty)
        }

        // --- INTERNAL STATE ---
        // [0] ATP
        if let Some(chem) = energy_opt {
            let normalized_atp = (chem.atp / chem.max_atp.max(1.0)).clamp(0.0, 1.0);
            state.inputs[0] = normalized_atp * 2.0 - 1.0;
        }
        // [1] Age
        if let Some(age) = age_opt {
            let normalized_age =
                (age.ticks as f32 / age.max_lifespan.max(1) as f32).clamp(0.0, 1.0);
            state.inputs[1] = normalized_age * 2.0 - 1.0;
        }
        // [2] Internal Pacemaker (~2Hz sine wave)
        state.inputs[2] = (*local_tick as f32 * 0.2).sin();

        // --- CHEMICAL SENSING ---
        // [3] O2 Gradient
        if let Some(field) = &cpu_field {
            let val = field.sample(node.position, 2); // channel 2 = O2
            state.inputs[3] = (val / 5000.0).clamp(0.0, 1.0) * 2.0 - 1.0;
        }
        // [4] CO2 Gradient
        if let Some(field) = &cpu_field {
            let val = field.sample(node.position, 3); // channel 3 = CO2
            state.inputs[4] = (val / 1000.0).clamp(0.0, 1.0) * 2.0 - 1.0;
        }
        // [5] Pheromones
        if let Some(field) = &cpu_signal_field {
            let val = field.sample(node.position);
            state.inputs[5] = val.clamp(0.0, 1.0) * 2.0 - 1.0;
        }

        // --- ENVIRONMENT ---
        // [6] Sunlight
        if let Some(atm) = &atmosphere {
            state.inputs[6] = atm.sunlight.clamp(0.0, 1.0) * 2.0 - 1.0;
        }
        // [7] Hazards
        if let Some(field) = &cpu_hazard_field {
            let val = field.sample(node.position);
            state.inputs[7] = val.clamp(0.0, 1.0) * 2.0 - 1.0;
        }
        // [8] World Boundary
        if let Some(e) = &env {
            let hw = e.width() / 2.0;
            let hh = e.height() / 2.0;
            let dist_x = hw - node.position.x.abs();
            let dist_y = hh - node.position.y.abs();
            let dist = dist_x.min(dist_y);
            let norm_dist = (dist / 150.0).clamp(0.0, 1.0);
            state.inputs[8] = norm_dist * 2.0 - 1.0;
        }

        // --- VISION ---
        if let Some(vision) = &mut vision_opt {
            if node.velocity.length_squared() > 0.01 {
                vision.last_forward = node.velocity.normalize();
            } else if vision.last_forward.length_squared() < 0.01 {
                vision.last_forward = common::Vec2::X;
            }
            let forward = vision.last_forward;

            let mut org_left = 0.0f32;
            let mut org_center = 0.0f32;
            let mut org_right = 0.0f32;

            let mut res_left = 0.0f32;
            let mut res_center = 0.0f32;
            let mut res_right = 0.0f32;

            let mut process_vision_target = |target_pos: common::Vec2, is_resource: bool| {
                let diff = target_pos - node.position;
                let dist = diff.length();
                if dist < vision.self_occlusion_radius || dist > vision.range {
                    return;
                }
                let dir = diff / dist;
                let angle = forward.angle_to(dir);
                let half_fov = vision.fov / 2.0;

                if angle >= -half_fov && angle <= half_fov {
                    let strength = 1.0 - (dist / vision.range);
                    let third_fov = half_fov / 1.5;

                    if is_resource {
                        if angle < -third_fov {
                            res_left = res_left.max(strength);
                        } else if angle > third_fov {
                            res_right = res_right.max(strength);
                        } else {
                            res_center = res_center.max(strength);
                        }
                    } else {
                        if angle < -third_fov {
                            org_left = org_left.max(strength);
                        } else if angle > third_fov {
                            org_right = org_right.max(strength);
                        } else {
                            org_center = org_center.max(strength);
                        }
                    }
                }
            };

            for other_node in node_query.iter() {
                if other_node.organism_id != node.organism_id {
                    process_vision_target(other_node.position, false);
                }
            }

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
                    ecology::Diet::Carnivore => {}
                }
            }

            state.inputs[9] = org_left * 2.0 - 1.0;
            state.inputs[10] = org_center * 2.0 - 1.0;
            state.inputs[11] = org_right * 2.0 - 1.0;

            state.inputs[12] = res_left * 2.0 - 1.0;
            state.inputs[13] = res_center * 2.0 - 1.0;
            state.inputs[14] = res_right * 2.0 - 1.0;
        }
    }

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
