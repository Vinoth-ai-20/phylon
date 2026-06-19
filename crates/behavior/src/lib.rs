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

/// System that runs the CTRNN brain and maps output to muscles.
pub fn behavior_system(
    mut query: bevy_ecs::prelude::Query<(
        &sensing::SensoryState,
        Option<&mut brain::Brain>,
        Option<&MotorSystem>,
    )>,
    mut springs: bevy_ecs::prelude::Query<&mut physics::Spring>,
) {
    // Time step integration is now fully handled by the GPU compute pass

    for (_sensory, mut brain_opt, motor_opt) in query.iter_mut() {
        if let Some(brain) = brain_opt.as_mut() {
            // 1. Extract outputs (the integration happened globally on GPU)
            let outputs = brain.get_outputs();

            // 2. Route outputs to effectors
            if let Some(motor) = motor_opt {
                for (i, &effector_entity) in motor.effectors.iter().enumerate() {
                    if let Ok(mut spring) = springs.get_mut(effector_entity) {
                        if i < outputs.len() {
                            let actuation = outputs[i];
                            // Map the [-1.0, 1.0] neural output to an actuation amplitude
                            // For Rotational or Elastic muscles, we can modulate rest_length or amplitude
                            spring.actuation_amplitude = actuation * 2.0;

                            // For simple immediate swimming, we can just oscillate it here
                            // if we don't have a CPG built in. But the brain IS a CTRNN, so it should oscillate!
                            if spring.constraint_type == physics::ConstraintType::Elastic {
                                spring.rest_length =
                                    spring.base_length + (actuation * spring.base_length * 0.5);
                            } else if spring.constraint_type == physics::ConstraintType::Rotational
                            {
                                spring.actuation_phase = actuation * std::f32::consts::PI;
                                // torque angle
                            }
                        }
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
