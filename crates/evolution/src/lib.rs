use genetics::Genome;
use organisms::SpeciesId;
use rustc_hash::FxHashMap;

pub fn init() {}

pub fn genetic_distance(g1: &Genome, g2: &Genome) -> f32 {
    let mut dist = 0.0;

    // Diet difference penalty
    if g1.diet != g2.diet {
        dist += 5.0;
    }

    // Reproduction mode difference penalty
    if std::mem::discriminant(&g1.reproduction_mode)
        != std::mem::discriminant(&g2.reproduction_mode)
    {
        dist += 2.0;
    }

    // Color distance
    let dr = g1.color[0] - g2.color[0];
    let dg = g1.color[1] - g2.color[1];
    let db = g1.color[2] - g2.color[2];
    dist += (dr * dr + dg * dg + db * db).sqrt() * 10.0;

    // Continuous traits
    dist += (g1.max_speed - g2.max_speed).abs() * 0.1;
    dist += (g1.metabolic_rate - g2.metabolic_rate).abs() * 2.0;
    dist += (g1.size - g2.size).abs() * 1.0;
    dist += (g1.vision_cone_angle - g2.vision_cone_angle).abs() * 5.0;
    dist += (g1.vision_depth - g2.vision_depth).abs() * 0.1;
    dist += (g1.max_weight - g2.max_weight).abs() * 0.5;

    // Brain weights
    let len = g1.brain_weights.len().min(g2.brain_weights.len());
    let mut brain_diff = 0.0;
    for i in 0..len {
        brain_diff += (g1.brain_weights[i] - g2.brain_weights[i]).abs();
    }
    brain_diff += (g1.brain_weights.len() as f32 - g2.brain_weights.len() as f32).abs() * 1.0;
    dist += brain_diff * 0.5;

    dist
}

#[derive(Debug, Clone, Default)]
pub struct SpeciesRegistry {
    pub species: FxHashMap<u32, Genome>,
    pub next_id: u32,
}

impl SpeciesRegistry {
    pub fn new() -> Self {
        Self {
            species: FxHashMap::default(),
            next_id: 1,
        }
    }

    pub fn assign_species(&mut self, genome: &Genome, threshold: f32) -> SpeciesId {
        for (&id, rep_genome) in &self.species {
            if genetic_distance(genome, rep_genome) < threshold {
                return SpeciesId(id);
            }
        }

        let new_id = self.next_id;
        self.next_id += 1;
        self.species.insert(new_id, genome.clone());
        SpeciesId(new_id)
    }
}
