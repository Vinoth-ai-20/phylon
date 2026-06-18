struct Spring {
    node_a: u32,
    node_b: u32,
    rest_length: f32,
    base_length: f32,
    stiffness: f32,
    damping: f32,
    actuation_amplitude: f32,
    actuation_phase: f32,
}

struct Time {
    t: f32,
}

@group(0) @binding(0) var<storage, read_write> springs: array<Spring>;
@group(0) @binding(1) var<uniform> time: Time;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= arrayLength(&springs)) {
        return;
    }

    var spring = springs[index];
    if (spring.actuation_amplitude > 0.0) {
        let frequency = 5.0; // Fast oscillation for crawling
        spring.rest_length = spring.base_length + spring.actuation_amplitude * sin(frequency * time.t + spring.actuation_phase);
        springs[index] = spring;
    }
}
