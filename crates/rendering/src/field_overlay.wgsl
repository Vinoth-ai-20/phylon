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
    
    // Map scalar to a color (dark navy to vibrant green)
    let bg = vec3<f32>(0.012, 0.024, 0.055);
    let fg = vec3<f32>(0.1, 0.9, 0.3);
    
    // Non-linear glow mapping
    let factor = clamp(val, 0.0, 1.0);
    let color = mix(bg, fg, factor * factor); // squared for better contrast
    
    return vec4<f32>(color, 1.0);
}
