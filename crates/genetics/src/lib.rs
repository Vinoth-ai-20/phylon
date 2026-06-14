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
    /// The maximum distance this organism can sense food.
    pub sense_radius: f32,
    /// Flattened weights and biases for the organism's neural network brain.
    pub brain_weights: Vec<f32>,
}

impl Default for Genome {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0],
            max_speed: 50.0,
            metabolic_rate: 1.0,
            size: 5.0,
            sense_radius: 100.0,
            brain_weights: Vec::new(),
        }
    }
}

impl Genome {
    /// Produces a mutated copy of this genome using Gaussian noise.
    pub fn mutate<R: Rng + ?Sized>(&self, rng: &mut R, mutation_rate: f32) -> Self {
        let normal = Normal::new(0.0, mutation_rate as f64).unwrap();

        let mutate_val = |val: f32, rng: &mut R| -> f32 { val + normal.sample(rng) as f32 };

        Self {
            color: [
                (mutate_val(self.color[0], rng)).clamp(0.0, 1.0),
                (mutate_val(self.color[1], rng)).clamp(0.0, 1.0),
                (mutate_val(self.color[2], rng)).clamp(0.0, 1.0),
            ],
            max_speed: (mutate_val(self.max_speed, rng)).clamp(10.0, 200.0),
            metabolic_rate: (mutate_val(self.metabolic_rate, rng)).clamp(0.1, 5.0),
            size: (mutate_val(self.size, rng)).clamp(2.0, 20.0),
            sense_radius: (mutate_val(self.sense_radius, rng)).clamp(20.0, 500.0),
            brain_weights: self
                .brain_weights
                .iter()
                .map(|&w| mutate_val(w, rng))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_deterministic_mutation() {
        let mut rng1 = ChaCha8Rng::seed_from_u64(42);
        let mut rng2 = ChaCha8Rng::seed_from_u64(42);

        let mut genome = Genome::default();
        genome.brain_weights = vec![0.1, 0.2, -0.5];

        let child1 = genome.mutate(&mut rng1, 0.1);
        let child2 = genome.mutate(&mut rng2, 0.1);

        assert_eq!(child1.color, child2.color);
        assert_eq!(child1.max_speed, child2.max_speed);
        assert_eq!(child1.sense_radius, child2.sense_radius);
        assert_eq!(child1.brain_weights, child2.brain_weights);

        // Ensure it actually changed from parent
        assert_ne!(genome.brain_weights, child1.brain_weights);
    }
}
