//! Heritable traits and mutation logic for Phylon organisms.

use rand::Rng;
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};

/// The genetic code of an organism, determining its baseline traits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Genome {
    /// R, G, B color values in range [0.0, 1.0].
    pub color: [f32; 3],
    /// The maximum speed the organism can attain.
    pub max_speed: f32,
    /// A multiplier applied to the basal metabolic rate. 
    /// Higher values consume energy faster.
    pub metabolic_rate: f32,
    /// The radius of the organism.
    pub size: f32,
}

impl Default for Genome {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0],
            max_speed: 50.0,
            metabolic_rate: 1.0,
            size: 5.0,
        }
    }
}

impl Genome {
    /// Produces a mutated copy of this genome using Gaussian noise.
    pub fn mutate<R: Rng + ?Sized>(&self, rng: &mut R, mutation_rate: f32) -> Self {
        let normal = Normal::new(0.0, mutation_rate as f64).unwrap();

        let mutate_val = |val: f32, rng: &mut R| -> f32 {
            val + normal.sample(rng) as f32
        };

        Self {
            color: [
                (mutate_val(self.color[0], rng)).clamp(0.0, 1.0),
                (mutate_val(self.color[1], rng)).clamp(0.0, 1.0),
                (mutate_val(self.color[2], rng)).clamp(0.0, 1.0),
            ],
            max_speed: (mutate_val(self.max_speed, rng)).clamp(10.0, 200.0),
            metabolic_rate: (mutate_val(self.metabolic_rate, rng)).clamp(0.1, 5.0),
            size: (mutate_val(self.size, rng)).clamp(2.0, 20.0),
        }
    }
}
