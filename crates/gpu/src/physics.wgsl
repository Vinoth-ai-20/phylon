struct ParticleNode {
    position: vec2<f32>,
    velocity: vec2<f32>,
    force: vec2<f32>,
    mass: f32,
}

struct Spring {
    node_a: u32,
    node_b: u32,
    constraint_type: u32, // 0 = Elastic, 1 = Rigid, 2 = Passive, 3 = Rotational
    rest_length: f32,
    base_length: f32,
    stiffness: f32,
    damping: f32,
    actuation_amplitude: f32,
    actuation_phase: f32,
    breaking_strain: f32,
    is_fin: u32,
    padding_2: u32,
}

struct PhysicsConfig {
    dt: f32,
    time: f32,
    _padding: vec2<f32>,
}

@group(0) @binding(0) var<storage, read_write> nodes: array<ParticleNode>;
@group(0) @binding(1) var<storage, read_write> springs: array<Spring>;
@group(0) @binding(2) var<uniform> config: PhysicsConfig;
@group(0) @binding(3) var<storage, read_write> atomic_forces_x: array<atomic<i32>>;
@group(0) @binding(4) var<storage, read_write> atomic_forces_y: array<atomic<i32>>;

const FORCE_SCALE: f32 = 10000.0;

// Pass 1: Compute Forces
@compute @workgroup_size(64)
fn compute_forces(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= arrayLength(&springs)) { return; }

    let spring = springs[index];
    let a_idx = spring.node_a;
    let b_idx = spring.node_b;
    
    let pos_a = nodes[a_idx].position;
    let pos_b = nodes[b_idx].position;
    
    let delta = pos_b - pos_a;
    let dist = max(length(delta), 0.0001); // Prevent div-by-zero

    let dir = delta / dist;
    // Only apply standard forces for Elastic, Passive, and Rotational. Rigid is handled in PBD.
    if (spring.constraint_type != 1u) {
            let displacement = dist - spring.rest_length;
            let spring_force = dir * (displacement * spring.stiffness);
            
            let vel_a = nodes[a_idx].velocity;
            let vel_b = nodes[b_idx].velocity;
            let rel_vel = vel_b - vel_a;
            let damp_force = dir * (dot(rel_vel, dir) * spring.damping);
            
            var total_force = spring_force + damp_force;
            
            // Apply anisotropic drag if it's a Fin
            if (spring.is_fin == 1u) {
                let mid_vel = (vel_a + vel_b) * 0.5; // Velocity relative to fluid
                let normal = vec2<f32>(-dir.y, dir.x);
                let v_norm = dot(mid_vel, normal);
                
                // High drag perpendicular to fin, low drag parallel
                let drag_force = -normal * (v_norm * abs(v_norm) * 50.0); // Quadratic drag
                
                // Add drag force to the nodes (divided equally)
                let half_drag = drag_force * 0.5;
                
                let dfx = i32(half_drag.x * FORCE_SCALE);
                let dfy = i32(half_drag.y * FORCE_SCALE);
                atomicAdd(&atomic_forces_x[a_idx], dfx);
                atomicAdd(&atomic_forces_y[a_idx], dfy);
                atomicAdd(&atomic_forces_x[b_idx], dfx);
                atomicAdd(&atomic_forces_y[b_idx], dfy);
            }
            
            let fx = i32(total_force.x * FORCE_SCALE);
            let fy = i32(total_force.y * FORCE_SCALE);
            
            atomicAdd(&atomic_forces_x[a_idx], fx);
            atomicAdd(&atomic_forces_y[a_idx], fy);
            atomicAdd(&atomic_forces_x[b_idx], -fx);
            atomicAdd(&atomic_forces_y[b_idx], -fy);
    }
}

// Pass 2: Integrate
@compute @workgroup_size(64)
fn integrate(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= arrayLength(&nodes)) { return; }

    var node = nodes[index];
    
    let fx = f32(atomicLoad(&atomic_forces_x[index])) / FORCE_SCALE;
    let fy = f32(atomicLoad(&atomic_forces_y[index])) / FORCE_SCALE;
    let total_force = node.force + vec2<f32>(fx, fy);
    
    // Symplectic Euler
    let acceleration = total_force / node.mass;
    node.velocity = node.velocity + acceleration * config.dt;
    
    // Global damping
    node.velocity = node.velocity * 0.99;
    
    node.position = node.position + node.velocity * config.dt;
    
    // Reset forces
    node.force = vec2<f32>(0.0, 0.0);
    atomicStore(&atomic_forces_x[index], 0);
    atomicStore(&atomic_forces_y[index], 0);
    
    nodes[index] = node;
}

// Pass 3: PBD Projection
@compute @workgroup_size(64)
fn pbd_projection(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= arrayLength(&springs)) { return; }

    let spring = springs[index];
    
    // Only process Rigid constraints
    if (spring.constraint_type == 1u) {
        let a_idx = spring.node_a;
        let b_idx = spring.node_b;
        
        let pos_a = nodes[a_idx].position;
        let pos_b = nodes[b_idx].position;
        
        let delta = pos_b - pos_a;
        let dist = max(length(delta), 0.0001); // Prevent div-by-zero
        
        // Dampen correction by 0.25 (relaxation factor) to prevent multi-spring atomicAdd explosions
        let correction_mag = (dist - spring.rest_length) * 0.5 * 0.25;
        let dir = delta / dist;
        let correction = dir * correction_mag;
            
            // To be thread-safe without atomic floats, PBD on GPU typically uses Graph Coloring or Jacobi methods.
            // For a simple implementation, we just atomic add the positional correction and divide later, 
            // or we accept slight tearing. Here we use atomicAdd on fixed-point positions.
            
            let cx = i32(correction.x * FORCE_SCALE);
            let cy = i32(correction.y * FORCE_SCALE);
            
            atomicAdd(&atomic_forces_x[a_idx], cx);
            atomicAdd(&atomic_forces_y[a_idx], cy);
            atomicAdd(&atomic_forces_x[b_idx], -cx);
            atomicAdd(&atomic_forces_y[b_idx], -cy);
    }
}

// Pass 4: Apply PBD Corrections
@compute @workgroup_size(64)
fn apply_pbd(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= arrayLength(&nodes)) { return; }

    var node = nodes[index];
    
    let cx = f32(atomicLoad(&atomic_forces_x[index])) / FORCE_SCALE;
    let cy = f32(atomicLoad(&atomic_forces_y[index])) / FORCE_SCALE;
    let correction = vec2<f32>(cx, cy);
    
    node.position = node.position + correction;
    node.velocity = node.velocity + (correction / config.dt);
    
    atomicStore(&atomic_forces_x[index], 0);
    atomicStore(&atomic_forces_y[index], 0);
    
    nodes[index] = node;
}
