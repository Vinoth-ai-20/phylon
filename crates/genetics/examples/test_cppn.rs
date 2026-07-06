use common::EntityId;
use genetics::genome::Genome;
use genetics::Cppn;

fn main() {
    let genome = Genome::seed(
        genetics::GenomeId(0),
        EntityId(0),
        Cppn::new(),
        Cppn::new(),
        Cppn::new(),
    );

    let total_nodes = 15;
    for i in 0..total_nodes {
        for j in 0..total_nodes {
            let w_inputs = [
                (i as f32) / (total_nodes as f32),
                (j as f32) / (total_nodes as f32),
            ];
            let w_outputs = genome.brain_cppn.evaluate(&w_inputs);
            if i == 0 && j == 1 {
                println!("Output for 0->1: {:?}", w_outputs);
            }
        }
    }
}
