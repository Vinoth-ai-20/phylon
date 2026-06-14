use genetics::Genome;
use hecs::World;
use organisms::{Energy, Health};
use sensing::Observation;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Intention {
    pub target_velocity: common::Vec2,
}

impl Intention {
    pub fn new() -> Self {
        Self {
            target_velocity: common::Vec2::ZERO,
        }
    }
}

pub const INPUT_SIZE: usize = 12;
pub const HIDDEN_SIZE: usize = 8;
pub const OUTPUT_SIZE: usize = 3; // dx, dy, neuromodulator
pub const TOTAL_NEURONS: usize = INPUT_SIZE + HIDDEN_SIZE + OUTPUT_SIZE;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrainState {
    pub potentials: [f32; TOTAL_NEURONS],
}

impl Default for BrainState {
    fn default() -> Self {
        Self {
            potentials: [0.0; TOTAL_NEURONS],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearnedWeights {
    pub data: Vec<f32>,
}

pub fn process_brain(world: &mut World) {
    puffin::profile_function!();

    let dt = 1.0;
    let tau = 2.0;

    for (_entity, (obs, genome, state, learned_weights, intention, energy, health)) in world
        .query_mut::<(
            &Observation,
            &Genome,
            &mut BrainState,
            &mut LearnedWeights,
            &mut Intention,
            &Energy,
            &Health,
        )>()
    {
        if energy.0 <= 0.0 || health.0 <= 0.0 {
            intention.target_velocity = common::Vec2::ZERO;
            continue;
        }

        if learned_weights.data.is_empty() {
            learned_weights.data = genome.brain_weights.clone();
        }

        let num_weights = TOTAL_NEURONS * TOTAL_NEURONS;
        if learned_weights.data.len() < num_weights {
            learned_weights.data.resize(num_weights, 0.0);
        }

        // Set inputs
        for i in 0..INPUT_SIZE {
            state.potentials[i] = obs.data[i];
        }

        let mut next_potentials = state.potentials;

        #[allow(clippy::needless_range_loop)]
        for i in INPUT_SIZE..TOTAL_NEURONS {
            let mut excitation = 0.0;
            for j in 0..TOTAL_NEURONS {
                let weight = learned_weights.data[j * TOTAL_NEURONS + i];
                let output_j = (1.0 / (1.0 + (-state.potentials[j]).exp())) - 0.5; // tanh-like
                excitation += weight * output_j;
            }
            let dy = (-state.potentials[i] + excitation) / tau;
            next_potentials[i] += dy * dt;
        }

        state.potentials = next_potentials;

        let out_start = INPUT_SIZE + HIDDEN_SIZE;
        let out_x = (1.0 / (1.0 + (-state.potentials[out_start]).exp())) * 2.0 - 1.0;
        let out_y = (1.0 / (1.0 + (-state.potentials[out_start + 1]).exp())) * 2.0 - 1.0;
        let neuromodulator = 1.0 / (1.0 + (-state.potentials[out_start + 2]).exp());

        intention.target_velocity =
            common::Vec2::new(out_x * genome.max_speed, out_y * genome.max_speed);

        // Hebbian Update
        if neuromodulator > 0.1 {
            let learning_rate = 0.01 * neuromodulator;
            for i in INPUT_SIZE..TOTAL_NEURONS {
                let output_i = (1.0 / (1.0 + (-state.potentials[i]).exp())) - 0.5;
                for j in 0..TOTAL_NEURONS {
                    let output_j = (1.0 / (1.0 + (-state.potentials[j]).exp())) - 0.5;
                    let idx = j * TOTAL_NEURONS + i;
                    let delta_w = learning_rate * output_i * output_j;
                    learned_weights.data[idx] = (learned_weights.data[idx] + delta_w)
                        .clamp(-genome.max_weight, genome.max_weight);
                }
            }
        }
    }
}
