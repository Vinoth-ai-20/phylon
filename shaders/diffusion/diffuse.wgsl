struct Uniforms {
    grid_width: u32,
    grid_height: u32,
    diffusion_rate: f32,
    decay_rate: f32,
};

@group(0) @binding(0) var<uniform> params: Uniforms;
@group(0) @binding(1) var<storage, read> field_in: array<f32>;
@group(0) @binding(2) var<storage, read_write> field_out: array<f32>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let x = global_id.x;
    let y = global_id.y;
    
    if x >= params.grid_width || y >= params.grid_height {
        return;
    }
    
    let index = y * params.grid_width + x;
    
    // Neumann boundaries (clamp to edge)
    let xl = max(x, 1u) - 1u;
    let xr = min(x + 1u, params.grid_width - 1u);
    let yt = max(y, 1u) - 1u;
    let yb = min(y + 1u, params.grid_height - 1u);
    
    let val_c = field_in[index];
    let val_l = field_in[y * params.grid_width + xl];
    let val_r = field_in[y * params.grid_width + xr];
    let val_t = field_in[yt * params.grid_width + x];
    let val_b = field_in[yb * params.grid_width + x];
    
    // Discrete Laplacian
    let laplacian = val_l + val_r + val_t + val_b - 4.0 * val_c;
    
    // Explicit Euler step
    let next_val = val_c + params.diffusion_rate * laplacian - params.decay_rate * val_c;
    
    field_out[index] = next_val;
}
