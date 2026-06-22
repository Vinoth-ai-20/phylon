struct SplatConfig {
    emitter_count: u32,
    _pad1: u32,
    _pad2: u32,
    _pad3: u32,
}

struct GpuSplat {
    grid_pos: vec2<f32>,
    value: f32,
    grid_radius: f32,
}

@group(0) @binding(0) var field_out: texture_storage_2d<r32float, write>;
@group(0) @binding(1) var<uniform> config: SplatConfig;
@group(0) @binding(2) var<storage, read> splats: array<GpuSplat>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dim = textureDimensions(field_out);
    let pos = vec2<i32>(global_id.xy);
    
    if (pos.x >= i32(dim.x) || pos.y >= i32(dim.y)) {
        return;
    }

    var total_value = 0.0;
    let cell_pos = vec2<f32>(f32(pos.x), f32(pos.y));
    
    for (var i: u32 = 0u; i < config.emitter_count; i++) {
        let s = splats[i];
        let dist = distance(cell_pos, s.grid_pos);
        
        // We use grid_radius to determine the standard deviation (sigma)
        let sigma = max(s.grid_radius, 0.001) * 0.5;
        let dist_sq = dist * dist;
        let sigma_sq = sigma * sigma;
        
        // Gaussian falloff: e^(-x^2 / 2sigma^2)
        let intensity = exp(-dist_sq / (2.0 * sigma_sq));
        
        // Only accumulate if intensity is non-negligible to save math
        if (intensity > 0.001) {
            total_value += s.value * intensity;
        }
    }

    textureStore(field_out, pos, vec4<f32>(total_value, 0.0, 0.0, 0.0));
}
