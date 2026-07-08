struct LayerConfig {
    diffusion_rate: f32,
    decay_rate: f32,
    emitter_count: u32,
    emitter_offset: u32,
}

struct DiffusionConfig {
    dt: f32,
    _pad1: u32,
    _pad2: u32,
    _pad3: u32,
    layers: array<LayerConfig, 5>,
}

struct GpuEmitter {
    grid_pos: vec2<f32>,
    value: f32,
    grid_radius: f32,
}

@group(0) @binding(0) var field_in: texture_2d_array<f32>;
@group(0) @binding(1) var field_out: texture_storage_2d_array<r32float, write>;
@group(0) @binding(2) var<uniform> config: DiffusionConfig;
@group(0) @binding(3) var<storage, read> emitters: array<GpuEmitter>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dim = textureDimensions(field_in);
    let pos = vec2<i32>(global_id.xy);
    let layer = global_id.z;
    
    if (pos.x >= i32(dim.x) || pos.y >= i32(dim.y) || layer >= 5u) {
        return;
    }

    let layer_conf = config.layers[layer];

    // Neumann boundary logic (reflecting at edges)
    let left   = max(pos.x - 1, 0);
    let right  = min(pos.x + 1, i32(dim.x) - 1);
    let top    = max(pos.y - 1, 0);
    let bottom = min(pos.y + 1, i32(dim.y) - 1);

    let center_val = textureLoad(field_in, pos, layer, 0).r;
    let left_val   = textureLoad(field_in, vec2<i32>(left, pos.y), layer, 0).r;
    let right_val  = textureLoad(field_in, vec2<i32>(right, pos.y), layer, 0).r;
    let top_val    = textureLoad(field_in, vec2<i32>(pos.x, top), layer, 0).r;
    let bottom_val = textureLoad(field_in, vec2<i32>(pos.x, bottom), layer, 0).r;

    // 5-point discrete Laplacian stencil
    let laplacian = left_val + right_val + top_val + bottom_val - 4.0 * center_val;

    // Calculate emissions
    var emission = 0.0;
    let cell_pos = vec2<f32>(f32(pos.x), f32(pos.y));
    for (var i: u32 = 0u; i < layer_conf.emitter_count; i++) {
        let e = emitters[layer_conf.emitter_offset + i];
        let dist = distance(cell_pos, e.grid_pos);
        if (dist <= e.grid_radius) {
            // Add value, maybe fade out by distance
            let intensity = 1.0 - (dist / max(e.grid_radius, 0.001));
            emission += e.value * intensity;
        }
    }

    // Explicit Euler step: u(t+1) = u(t) + dt * (D * ∇²u - λ * u + S)
    let delta = config.dt * (layer_conf.diffusion_rate * laplacian - layer_conf.decay_rate * center_val + emission);
    var new_val = center_val + delta;
    
    // Clamp to prevent runaway values or negative concentrations
    new_val = clamp(new_val, 0.0, 1000.0);

    textureStore(field_out, pos, layer, vec4<f32>(new_val, 0.0, 0.0, 0.0));
}
