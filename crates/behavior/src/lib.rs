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

/// The motor system component holding references to actuatable muscles.
#[derive(bevy_ecs::prelude::Component, Debug, Clone)]
pub struct MotorSystem {
    /// Ordered list of spring entities this organism can actuate.
    pub effectors: Vec<bevy_ecs::entity::Entity>,
}

/// Configuration parameters for the behavior system.
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

/// System that runs the CTRNN brain and maps output to muscles.
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
        entity,
        node,
        _sensory,
        mut brain_opt,
        motor_opt,
        mut emitter_opt,
        mut energy_opt,
        age_opt,
    ) in query.iter_mut()
    {
        if let Some(brain) = brain_opt.as_mut() {
            // 1. Extract outputs (the integration happened globally on GPU)
            let outputs = brain.get_outputs();

            if let Some(age) = age_opt {
                if age.ticks == 50 || age.ticks == 450 {
                    println!(
                        "[DIAGNOSTIC] Age {}: Entity {} outputs = {:?}",
                        age.ticks,
                        entity.index(),
                        outputs
                    );
                }
            }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motor_system_initialization() {
        let ms = MotorSystem { effectors: vec![] };
        assert!(ms.effectors.is_empty());
    }
}
