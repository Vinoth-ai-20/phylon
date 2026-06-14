use genetics::Genome;
use hecs::World;
use ndarray::{ArrayView1, ArrayView2};
use sensing::Observation;

pub const INPUT_SIZE: usize = 4;
pub const HIDDEN_SIZE: usize = 8;
pub const OUTPUT_SIZE: usize = 2;
pub const BRAIN_WEIGHTS_COUNT: usize =
    (INPUT_SIZE * HIDDEN_SIZE) + HIDDEN_SIZE + (HIDDEN_SIZE * OUTPUT_SIZE) + OUTPUT_SIZE;

/// Component storing the desired outputs from the brain.
/// Format: [turn_amount (radians), forward_thrust]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Intention {
    pub data: [f32; 2],
}

impl Intention {
    pub fn new() -> Self {
        Self { data: [0.0; 2] }
    }
}

/// A thin wrapper to execute forward passes using NdArray and a genome's weight slice.
pub struct NeuralBrain;

impl NeuralBrain {
    /// Executes a forward pass for a single organism given its genome weights and sensory inputs.
    pub fn forward(weights: &[f32], obs: &[f32; 4]) -> [f32; 2] {
        if weights.len() != BRAIN_WEIGHTS_COUNT {
            return [0.0; 2]; // Failsafe if uninitialized or wrong size
        }

        // Layer 1: Input (4) -> Hidden (8)
        let w1_len = INPUT_SIZE * HIDDEN_SIZE;
        let w1_slice = &weights[0..w1_len];
        let b1_slice = &weights[w1_len..w1_len + HIDDEN_SIZE];

        // Layer 2: Hidden (8) -> Output (2)
        let w2_start = w1_len + HIDDEN_SIZE;
        let w2_len = HIDDEN_SIZE * OUTPUT_SIZE;
        let w2_slice = &weights[w2_start..w2_start + w2_len];
        let b2_slice = &weights[w2_start + w2_len..];

        // Construct views
        let w1 = ArrayView2::from_shape((HIDDEN_SIZE, INPUT_SIZE), w1_slice).unwrap();
        let b1 = ArrayView1::from_shape(HIDDEN_SIZE, b1_slice).unwrap();

        let w2 = ArrayView2::from_shape((OUTPUT_SIZE, HIDDEN_SIZE), w2_slice).unwrap();
        let b2 = ArrayView1::from_shape(OUTPUT_SIZE, b2_slice).unwrap();

        let input = ArrayView1::from_shape(INPUT_SIZE, obs).unwrap();

        // Pass 1
        let hidden = w1.dot(&input) + b1;
        // ReLU activation
        let hidden = hidden.mapv(|x| x.max(0.0));

        // Pass 2
        let output = w2.dot(&hidden) + b2;
        // Tanh activation (bounds output between -1.0 and 1.0)
        let output = output.mapv(|x| x.tanh());

        [output[0], output[1]]
    }
}

/// System to execute neural inference across the population.
pub fn process_brain(world: &mut World) {
    puffin::profile_function!();

    for (_entity, (genome, obs, intention)) in
        world.query_mut::<(&Genome, &Observation, &mut Intention)>()
    {
        let result = NeuralBrain::forward(&genome.brain_weights, &obs.data);
        intention.data = result;
    }
}
