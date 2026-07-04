struct Spring {
    node_a: u32,
    node_b: u32,
    constraint_type: u32,
    rest_length: f32,
    base_length: f32,
    stiffness: f32,
    damping: f32,
    actuation_amplitude: f32,
    actuation_phase: f32,
    breaking_strain: f32,
    is_fin: u32,
    _padding: u32,
}

struct PhysicsConfig {
    dt: f32,
    time: f32,
    active_node_count: u32,
    active_spring_count: u32,
}

@group(0) @binding(1) var<storage, read_write> springs: array<Spring>;
@group(0) @binding(2) var<uniform> config: PhysicsConfig;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= config.active_spring_count) {
        return;
    }

    var spring = springs[index];
    if (spring.actuation_amplitude > 0.0) {
        // Frequency 2.0 rad/s → period ≈ 3.1 s, one smooth undulation per 3 s.
        // Previously 5.0 rad/s caused rest_length to change faster than the
        // spring-damper system could respond, injecting excess kinetic energy.
        let frequency = 2.0;
        let raw_length = spring.base_length + spring.actuation_amplitude * sin(frequency * config.time + spring.actuation_phase);
        spring.rest_length = max(0.1, raw_length);
    }
    springs[index] = spring;
}
