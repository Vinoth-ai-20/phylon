//! # Phylon Environment
//!
//! Terrain heightmaps, biomes, climate zones, weather systems, and seasonal
//! cycles. The environment crate manages all non-biological world state that
//! modulates organism survival conditions.
//!
//! ## Phase 0 scope
//!
//! Biome and climate type enumerations. Implementation: Phase 4.

#![warn(missing_docs)]
#![warn(clippy::all)]

use serde::{Deserialize, Serialize};

/// Biome classification for a world chunk.
///
/// Biomes determine baseline temperature, humidity, sunlight intensity,
/// and soil composition for the chunks assigned to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Biome {
    /// Tropical rainforest — high humidity, high sunlight, dense canopy.
    TropicalRainforest,
    /// Temperate forest — moderate conditions, seasonal variation.
    TemperateForest,
    /// Desert — low humidity, extreme temperature range, sparse nutrients.
    Desert,
    /// Tundra — cold, short growing seasons, permafrost.
    Tundra,
    /// Grassland / savanna — high sunlight, moderate water availability.
    Grassland,
    /// Freshwater lake or river.
    Freshwater,
    /// Shallow marine / coastal zone.
    CoastalMarine,
    /// Deep ocean.
    DeepOcean,
    /// Volcanic / hydrothermal zone.
    Hydrothermal,
}

/// Climate zone classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClimateZone {
    /// Tropical belt — year-round warmth.
    Tropical,
    /// Subtropical — warm with dry seasons.
    Subtropical,
    /// Temperate — four distinct seasons.
    Temperate,
    /// Boreal / subarctic — long cold winters.
    Boreal,
    /// Polar — year-round cold, minimal sunlight.
    Polar,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn biome_is_copy() {
        let b = Biome::Desert;
        let _b2 = b;
    }

    #[test]
    fn climate_zone_is_copy() {
        let cz = ClimateZone::Temperate;
        let _cz2 = cz;
    }
}
