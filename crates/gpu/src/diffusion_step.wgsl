struct DiffusionConfig {
    diffusion_rate: f32,
    decay_rate: f32,
    dt: f32,
    emitter_count: u32,
}

struct GpuEmitter {
    grid_pos: vec2<f32>,
    value: f32,
    grid_radius: f32,
}

@group(0) @binding(0) var field_in: texture_2d<f32>;
@group(0) @binding(1) var field_out: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var<uniform> config: DiffusionConfig;
@group(0) @binding(3) var<storage, read> emitters: array<GpuEmitter>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dim = textureDimensions(field_in);
    let pos = vec2<i32>(global_id.xy);
    
    if (pos.x >= i32(dim.x) || pos.y >= i32(dim.y)) {
        return;
    }

    // Neumann boundary logic (reflecting at edges)
    let left   = max(pos.x - 1, 0);
    let right  = min(pos.x + 1, i32(dim.x) - 1);
    let top    = max(pos.y - 1, 0);
    let bottom = min(pos.y + 1, i32(dim.y) - 1);

    let center_val = textureLoad(field_in, pos, 0).r;
    let left_val   = textureLoad(field_in, vec2<i32>(left, pos.y), 0).r;
    let right_val  = textureLoad(field_in, vec2<i32>(right, pos.y), 0).r;
    let top_val    = textureLoad(field_in, vec2<i32>(pos.x, top), 0).r;
    let bottom_val = textureLoad(field_in, vec2<i32>(pos.x, bottom), 0).r;

    // 5-point discrete Laplacian stencil
    let laplacian = left_val + right_val + top_val + bottom_val - 4.0 * center_val;

    // Calculate emissions
    var emission = 0.0;
    let cell_pos = vec2<f32>(f32(pos.x), f32(pos.y));
    for (var i: u32 = 0u; i < config.emitter_count; i++) {
        let e = emitters[i];
        let dist = distance(cell_pos, e.grid_pos);
        if (dist <= e.grid_radius) {
            // Add value, maybe fade out by distance
            let intensity = 1.0 - (dist / max(e.grid_radius, 0.001));
            emission += e.value * intensity;
        }
    }

    // Explicit Euler step: u(t+1) = u(t) + dt * (D * ∇²u - λ * u + S)
    let delta = config.dt * (config.diffusion_rate * laplacian - config.decay_rate * center_val + emission);
    var new_val = center_val + delta;
    
    // Clamp to prevent runaway values or negative concentrations
    new_val = clamp(new_val, 0.0, 1000.0);

    textureStore(field_out, pos, vec4<f32>(new_val, 0.0, 0.0, 0.0));
}
