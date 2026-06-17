//! Heritable traits and mutation logic for Phylon organisms.

use rand::Rng;
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Diet {
    Herbivore,
    Carnivore,
    Omnivore,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ReproductionMode {
    Asexual,
    Facultative { sexual_threshold: f32 },
    Sexual,
}

/// The genetic code of an organism, determining its baseline traits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Genome {
    pub version: u8,
    pub diet: Diet,
    pub color: [f32; 3],
    pub max_speed: f32,
    pub metabolic_rate: f32,
    pub size: f32,
    pub vision_cone_angle: f32,
    pub vision_depth: f32,
    pub reproduction_mode: ReproductionMode,
    pub max_weight: f32,
    pub brain_weights: Vec<f32>,
    /// Number of body segments this organism expresses
    pub hox_count: u8, // 3 to 7, clamped
    /// HOX gene array — one entry per segment. Each u8 encodes the segment type
    pub hox_genes: [u8; 7],
    /// Segment size modifiers — how large each segment is
    pub hox_size_factors: [f32; 7],
    /// Appendage type per segment
    pub hox_appendages: [u8; 7],
    /// Appendage count per segment (1-4)
    pub hox_appendage_count: [u8; 7],
}

impl Default for Genome {
    fn default() -> Self {
        Self {
            version: 1,
            diet: Diet::Herbivore,
            color: [1.0, 1.0, 1.0],
            max_speed: 50.0,
            metabolic_rate: 1.0,
            size: 5.0,
            vision_cone_angle: std::f32::consts::PI / 2.0,
            vision_depth: 100.0,
            reproduction_mode: ReproductionMode::Asexual,
            max_weight: 10.0,
            brain_weights: Vec::new(),
            hox_count: 4,
            hox_genes: [5, 2, 2, 6, 0, 0, 0],
            hox_size_factors: [0.5, 0.9, 0.8, 0.4, 0.0, 0.0, 0.0],
            hox_appendages: [1, 1, 1, 1, 0, 0, 0],
            hox_appendage_count: [2, 4, 4, 2, 0, 0, 0],
        }
    }
}

impl Genome {
    pub fn default_for_diet(diet: Diet) -> Self {
        let mut base = Self {
            diet,
            ..Default::default()
        };
        match diet {
            Diet::Herbivore => {
                base.hox_count = 4;
                base.hox_genes = [5, 2, 2, 6, 0, 0, 0];
                base.hox_size_factors = [0.5, 0.9, 0.8, 0.4, 0.0, 0.0, 0.0];
                base.hox_appendages = [1, 1, 1, 1, 0, 0, 0];
                base.hox_appendage_count = [2, 4, 4, 2, 0, 0, 0];
            }
            Diet::Carnivore => {
                base.hox_count = 5;
                base.hox_genes = [5, 4, 1, 1, 6, 0, 0];
                base.hox_size_factors = [0.6, 0.3, 0.7, 0.6, 0.35, 0.0, 0.0];
                base.hox_appendages = [6, 0, 2, 2, 2, 0, 0];
                base.hox_appendage_count = [2, 0, 1, 1, 2, 0, 0];
            }
            Diet::Omnivore => {
                // Scavenger plan
                base.hox_count = 5;
                base.hox_genes = [5, 2, 0, 2, 0, 0, 0];
                base.hox_size_factors = [0.4, 0.7, 0.5, 0.6, 0.4, 0.0, 0.0];
                base.hox_appendages = [3, 3, 3, 3, 3, 0, 0];
                base.hox_appendage_count = [2, 3, 2, 3, 2, 0, 0];
            }
        }
        base
    }

    pub fn mutate<R: Rng + ?Sized>(&self, rng: &mut R, mutation_rate: f32) -> Self {
        let normal = Normal::new(0.0, mutation_rate as f64).unwrap();
        let mutate_val = |val: f32, rng: &mut R| -> f32 { val + normal.sample(rng) as f32 };

        let mut new_diet = self.diet;
        if rng.gen_bool(0.01) {
            new_diet = match rng.gen_range(0..3) {
                0 => Diet::Herbivore,
                1 => Diet::Carnivore,
                _ => Diet::Omnivore,
            };
        }

        let mut new_reproduction_mode = self.reproduction_mode.clone();
        if rng.gen_bool(mutation_rate.into()) {
            match new_reproduction_mode {
                ReproductionMode::Asexual => {
                    if rng.gen_bool(0.1) {
                        new_reproduction_mode = ReproductionMode::Facultative {
                            sexual_threshold: rng.gen_range(0.1..0.9),
                        };
                    }
                }
                ReproductionMode::Facultative {
                    ref mut sexual_threshold,
                } => {
                    if rng.gen_bool(0.1) {
                        new_reproduction_mode = if rng.gen_bool(0.5) {
                            ReproductionMode::Asexual
                        } else {
                            ReproductionMode::Sexual
                        };
                    } else {
                        *sexual_threshold =
                            (*sexual_threshold + rng.gen_range(-0.1..0.1)).clamp(0.0, 1.0);
                    }
                }
                ReproductionMode::Sexual => {
                    if rng.gen_bool(0.1) {
                        new_reproduction_mode = ReproductionMode::Facultative {
                            sexual_threshold: rng.gen_range(0.1..0.9),
                        };
                    }
                }
            }
        }

        let mut new_hox_count = self.hox_count;
        if rng.gen::<f32>() < 0.005 {
            new_hox_count = (new_hox_count as i8 + rng.gen_range(-1..=1)).clamp(3, 7) as u8;
        }

        let mut new_hox_genes = self.hox_genes;
        for gene in new_hox_genes.iter_mut().take(new_hox_count as usize) {
            if rng.gen::<f32>() < 0.02 {
                *gene = rng.gen_range(0..=6);
            }
        }

        let mut new_hox_size_factors = self.hox_size_factors;
        for factor in new_hox_size_factors.iter_mut().take(new_hox_count as usize) {
            if rng.gen::<f32>() < 0.04 {
                *factor = (*factor + rng.gen::<f32>() * 0.1 - 0.05).clamp(0.1, 1.0);
            }
        }

        let mut new_hox_appendages = self.hox_appendages;
        for appendage in new_hox_appendages.iter_mut().take(new_hox_count as usize) {
            if rng.gen::<f32>() < 0.015 {
                *appendage = rng.gen_range(0..=6);
            }
        }

        Self {
            version: self.version,
            diet: new_diet,
            color: [
                (mutate_val(self.color[0], rng)).clamp(0.0, 1.0),
                (mutate_val(self.color[1], rng)).clamp(0.0, 1.0),
                (mutate_val(self.color[2], rng)).clamp(0.0, 1.0),
            ],
            max_speed: (mutate_val(self.max_speed, rng)).clamp(10.0, 200.0),
            metabolic_rate: (mutate_val(self.metabolic_rate, rng)).clamp(0.1, 5.0),
            size: (mutate_val(self.size, rng)).clamp(2.0, 20.0),
            vision_cone_angle: (mutate_val(self.vision_cone_angle, rng))
                .clamp(0.1, std::f32::consts::PI),
            vision_depth: (mutate_val(self.vision_depth, rng)).clamp(20.0, 500.0),
            reproduction_mode: new_reproduction_mode,
            max_weight: (mutate_val(self.max_weight, rng)).clamp(1.0, 50.0),
            brain_weights: self
                .brain_weights
                .iter()
                .map(|&w| mutate_val(w, rng))
                .collect(),
            hox_count: new_hox_count,
            hox_genes: new_hox_genes,
            hox_size_factors: new_hox_size_factors,
            hox_appendages: new_hox_appendages,
            hox_appendage_count: self.hox_appendage_count,
        }
    }

    pub fn crossover<R: Rng + ?Sized>(
        &self,
        other: &Self,
        rng: &mut R,
        mutation_rate: f32,
    ) -> Self {
        let pick = |a: f32, b: f32, rng: &mut R| -> f32 {
            if rng.gen_bool(0.5) {
                a
            } else {
                b
            }
        };
        let avg = |a: f32, b: f32| -> f32 { (a + b) / 2.0 };

        let new_diet = if rng.gen_bool(0.5) {
            self.diet
        } else {
            other.diet
        };
        let new_reproduction_mode = if rng.gen_bool(0.5) {
            self.reproduction_mode.clone()
        } else {
            other.reproduction_mode.clone()
        };

        let mut new_brain = self.brain_weights.clone();
        if self.brain_weights.len() == other.brain_weights.len() && !self.brain_weights.is_empty() {
            let split = rng.gen_range(0..self.brain_weights.len());
            new_brain[split..self.brain_weights.len()]
                .copy_from_slice(&other.brain_weights[split..]);
        }

        let hox_parent = if rng.gen_bool(0.5) { self } else { other };

        let combined = Self {
            version: self.version,
            diet: new_diet,
            color: [
                avg(self.color[0], other.color[0]),
                avg(self.color[1], other.color[1]),
                avg(self.color[2], other.color[2]),
            ],
            max_speed: avg(self.max_speed, other.max_speed),
            metabolic_rate: avg(self.metabolic_rate, other.metabolic_rate),
            size: avg(self.size, other.size),
            vision_cone_angle: pick(self.vision_cone_angle, other.vision_cone_angle, rng),
            vision_depth: pick(self.vision_depth, other.vision_depth, rng),
            reproduction_mode: new_reproduction_mode,
            max_weight: avg(self.max_weight, other.max_weight),
            brain_weights: new_brain,
            hox_count: hox_parent.hox_count,
            hox_genes: hox_parent.hox_genes,
            hox_size_factors: hox_parent.hox_size_factors,
            hox_appendages: hox_parent.hox_appendages,
            hox_appendage_count: hox_parent.hox_appendage_count,
        };

        combined.mutate(rng, mutation_rate)
    }
}
