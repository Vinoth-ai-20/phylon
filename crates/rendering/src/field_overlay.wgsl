struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    // Generate fullscreen triangle (covers viewport with 1 triangle)
    let x = f32((in_vertex_index << 1u) & 2u);
    let y = f32(in_vertex_index & 2u);
    var out: VertexOutput;
    // Flip Y for texture coordinates because wgpu textures are Y-down
    out.uv = vec2<f32>(x, 1.0 - y);
    out.clip_position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return out;
}

@group(0) @binding(0) var t_field: texture_2d<f32>;
@group(0) @binding(1) var s_field: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let val = textureSample(t_field, s_field, in.uv).r;
    
    // The background is cleared by the render pass.
    // We only need to output the green diffusion field with transparency.
    let fg = vec3<f32>(0.1, 0.9, 0.3);
    
    // Configurable constants for tuning the field visualization curve
    let FIELD_VALUE_SCALE: f32 = 0.05; // Scales raw simulation value (which peaks around 10.0)
    let FIELD_MAX_ALPHA: f32 = 0.45;   // Hard cap on opacity so the background always shows through
    
    // Scale down the raw simulation value
    let scaled_val = max(val, 0.0) * FIELD_VALUE_SCALE;
    
    // Use a soft non-linear curve (sqrt) to spread the gradient outward,
    // then clamp to the max alpha so the center doesn't saturate to an opaque blob.
    let curve = sqrt(scaled_val);
    let alpha = clamp(curve, 0.0, FIELD_MAX_ALPHA);
    
    return vec4<f32>(fg, alpha);
}
