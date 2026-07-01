//! # Phylon Behavior
//!
//! Movement decisions, action selection, and locomotion output systems.
//!
//! Organisms receive sensory inputs, process them through their neural brain,
//! and emit motor commands. This crate converts neural output into physical
//! forces applied to the organism's particle nodes.
//!
//! ## Phase 0 scope
//!
//! Action type declaration. Implementation: Phase 3.

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

/// # Core Behavior Translation System
///
/// ## 1. What Happens
/// The `behavior_system` bridges the cognitive and physical domains. It reads the integrated
/// outputs from the GPU CTRNN neural networks and translates them into structural actuations
/// (muscle contractions) and chemical pheromone emissions, while applying environmental
/// temperature constraints.
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
#[allow(clippy::type_complexity)]
pub fn behavior_system(
    mut query: bevy_ecs::prelude::Query<(
        bevy_ecs::entity::Entity,
        &physics::ParticleNode,
        &sensing::SensoryState,
        Option<&mut brain::Brain>,
        Option<&MotorSystem>,
        Option<&mut diffusion::SignalEmitter>,
        Option<&mut metabolism::ChemicalEconomy>,
        Option<&metabolism::Age>,
    )>,
    mut springs: bevy_ecs::prelude::Query<&mut physics::Spring>,
    env: Option<bevy_ecs::prelude::Res<environment::EnvironmentManager>>,
    config: Option<bevy_ecs::prelude::Res<BehaviorConfig>>,
) {
    // Time step integration is now fully handled by the GPU compute pass

    for (
        _entity,
        node,
        sensory,
        mut brain_opt,
        motor_opt,
        mut emitter_opt,
        mut energy_opt,
        _age_opt,
    ) in query.iter_mut()
    {
        if let Some(brain) = brain_opt.as_mut() {
            // -1. Set inputs from sensory state
            brain.set_inputs(&sensory.inputs);

            // 0. Perform CPU integration step to allow CTRNNs to oscillate
            brain.step_cpu(0.016);

            // 1. Extract outputs
            let outputs = brain.get_outputs();

            // Calculate environmental efficiency based on local temperature
            let mut efficiency = 1.0;
            if let Some(env_res) = &env {
                let temp = env_res.get_temperature_at(node.position.x, node.position.y);
                let ideal_temp = 15.0; // Hardcoded for Phase 7 validation
                let divergence = (temp - ideal_temp).abs();

                // Efficiency drops linearly by 5% per degree off ideal.
                // At 20 degrees off (e.g. 35C or -5C), efficiency is 0.0 (paralyzed).
                efficiency = (1.0 - (divergence * 0.05)).clamp(0.0, 1.0);
            }

            // 2. Route outputs to effectors
            if let Some(motor) = motor_opt {
                for (i, &effector_entity) in motor.effectors.iter().enumerate() {
                    if let Ok(mut spring) = springs.get_mut(effector_entity) {
                        if i < outputs.len() {
                            let actuation = outputs[i];

                            // Apply environmental efficiency loss
                            let effective_actuation = actuation * efficiency;

                            // Map the [-1.0, 1.0] neural output to an actuation amplitude
                            // For Rotational or Elastic muscles, we can modulate rest_length or amplitude
                            spring.actuation_amplitude = effective_actuation * 8.0;

                            // For simple immediate swimming, we can just oscillate it here
                            // if we don't have a CPG built in. But the brain IS a CTRNN, so it should oscillate!
                            if spring.constraint_type == physics::ConstraintType::Elastic {
                                spring.rest_length = spring.base_length
                                    + (effective_actuation * spring.base_length * 0.5);
                            }

                            // Punish rigidity: if the muscle is locked at high actuation, drain a small amount of ATP
                            if actuation.abs() > 0.9 {
                                if let Some(ref mut chem) = energy_opt {
                                    chem.atp = (chem.atp - 0.05).max(0.0);
                                }
                            }
                        }
                    }
                }
            }

            // 3. Route to signal emitter if present
            let mut signal_output: f32 = 0.0;
            if let Some(motor) = motor_opt {
                if motor.effectors.len() < outputs.len() {
                    signal_output = outputs[motor.effectors.len()];
                }
            } else if !outputs.is_empty() {
                signal_output = outputs[0];
            }

            if let Some(emitter) = emitter_opt.as_mut() {
                // Ensure value is positive for emission strength
                let emission = signal_output.clamp(0.0, 1.0);
                emitter.value = emission;

                // Drain ATP
                if emission > 0.0 {
                    if let Some(chem) = energy_opt.as_mut() {
                        let cost_per_unit = config
                            .as_ref()
                            .map_or(0.01, |c| c.signal_energy_cost_per_unit);
                        let cost = emission * cost_per_unit;
                        chem.atp = (chem.atp - cost).max(0.0);
                    }
                }
            }
        }
    }
}

/// # Behavior Tracing System
///
/// Reads the neural outputs of each organism's brain and logs significant actions
/// (e.g. high actuation or strong signal emission) to the structured tracing output.
pub fn behavior_logging_system(
    query: bevy_ecs::prelude::Query<(
        bevy_ecs::entity::Entity,
        &brain::Brain,
        Option<&physics::ParticleNode>,
        Option<&diffusion::SignalEmitter>,
    )>,
) {
    for (entity, brain, node_opt, emitter_opt) in query.iter() {
        let outputs = brain.get_outputs();
        if outputs.is_empty() {
            continue;
        }

        // Check for strong physical actuation
        let mut max_actuation = 0.0_f32;
        for &out in &outputs {
            if out.abs() > max_actuation {
                max_actuation = out.abs();
            }
        }

        let emission = if let Some(emitter) = emitter_opt {
            emitter.value
        } else {
            0.0
        };

        // Only log if something interesting is happening
        if max_actuation > 0.8 || emission > 0.5 {
            let pos_x = node_opt.map_or(0.0, |n| n.position.x);
            let pos_y = node_opt.map_or(0.0, |n| n.position.y);

            tracing::info!(
                target: "behavior",
                entity_id = entity.to_bits(),
                max_actuation = max_actuation,
                emission = emission,
                pos_x = pos_x,
                pos_y = pos_y,
                "Significant behavioral action recorded"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motor_system_initialization() {
        let ms = MotorSystem { effectors: vec![] };
        assert!(ms.effectors.is_empty());
    }
}
