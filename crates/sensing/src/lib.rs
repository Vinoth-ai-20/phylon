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

#![warn(missing_docs)]
#![warn(clippy::all)]

/// The sensory modalities available to organisms.
///
/// Each modality produces one or more floating-point input values for the
/// neural brain. The total input vector length depends on which modalities
/// are enabled in the organism's genome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SensorModality {
    /// Directional raycast vision with configurable cone and range.
    Vision,
    /// Olfactory sampling from pheromone / scent fields.
    Olfaction,
    /// Directional hearing from the sound pressure field.
    Hearing,
    /// Tactile contact detection.
    Touch,
    /// Temperature field reading.
    Thermoreception,
    /// Internal body state: energy, velocity, acceleration.
    Proprioception,
    /// Pressure / depth sensing.
    Baroreception,
    /// Electric field sensing (aquatic organisms).
    Electroreception,
    /// Geomagnetic field sensing.
    Magnetoreception,
    /// Pain signal from tissue damage.
    Nociception,
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
