struct CtrnnNode {
    state: f32,
    time_constant: f32,
    bias: f32,
    activation: u32,
    first_synapse: u32,
    synapse_count: u32,
    neuromodulator: f32,
}

struct CtrnnSynapse {
    source: u32,
    tgt_node: u32,
    weight: f32,
    plasticity_rate: f32,
}

struct BrainConfig {
    dt: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
}

@group(0) @binding(0) var<storage, read_write> nodes: array<CtrnnNode>;
@group(0) @binding(1) var<storage, read_write> synapses: array<CtrnnSynapse>;
@group(0) @binding(2) var<uniform> config: BrainConfig;

fn apply_activation(x: f32, act_id: u32) -> f32 {
    switch act_id {
        case 0u: { return 1.0 / (1.0 + exp(-x)); } // Sigmoid
        case 1u: { return tanh(x); }               // Tanh
        case 2u: { return max(0.0, x); }           // ReLU
        case 3u: {                                 // LeakyReLU
            if x > 0.0 { return x; } else { return 0.01 * x; }
        }
        case 4u: { return sin(x); }                // Sine
        case 5u: { return exp(-x * x); }           // Gaussian
        case 6u: { return abs(x); }                // Abs
        case 7u: { return x; }                     // Linear
        case 8u: {                                 // Step
            if x > 0.0 { return 1.0; } else { return 0.0; }
        }
        default: { return x; }
    }
}

@compute @workgroup_size(64)
fn integrate_nodes(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if index >= arrayLength(&nodes) {
        return;
    }

    let node = nodes[index];
    
    // If a node is purely an input node, its state is driven by sensors, 
    // but in CTRNNs even input nodes can be part of the network dynamics if they have a finite time constant.
    // We treat input nodes as having a very small time constant or we assume the CPU overrides their state before integration.
    // If the CPU wrote the sensory state, we just integrate it as normal (or we could use a specific flag).
    // For now, let's just do standard CTRNN integration on all nodes:
    
    var sum = 0.0;
    for (var i = 0u; i < node.synapse_count; i++) {
        let syn = synapses[node.first_synapse + i];
        let src_node = nodes[syn.source];
        let src_act = apply_activation(src_node.state + src_node.bias, src_node.activation);
        sum += src_act * syn.weight;
        
        // Hebbian Plasticity Update: ΔW = η * O_src * O_tgt * M
        if syn.plasticity_rate > 0.0 {
            let tgt_act = apply_activation(node.state + node.bias, node.activation);
            // We use the post-activation states and the neuromodulator concentration
            let delta_w = syn.plasticity_rate * src_act * tgt_act * node.neuromodulator;
            
            // In-place weight update (clamped to prevent explosion)
            let new_weight = clamp(syn.weight + delta_w * config.dt, -10.0, 10.0);
            synapses[node.first_synapse + i].weight = new_weight;
        }
    }
    
    // Euler step
    let dy_dt = (1.0 / node.time_constant) * (-node.state + sum);
    
    // Update node state, clamped to prevent mathematical explosion
    nodes[index].state = clamp(nodes[index].state + dy_dt * config.dt, -10.0, 10.0);
}
