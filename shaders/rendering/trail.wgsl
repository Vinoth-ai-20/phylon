struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(position, 0.0, 1.0); // Fullscreen quad
    out.uv = uv;
    return out;
}

@group(0) @binding(0)
var trail_texture: texture_2d<f32>;
@group(0) @binding(1)
var trail_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample previous frame's trail
    let prev_color = textureSample(trail_texture, trail_sampler, in.uv);
    
    // Decay factor
    let decay = 0.97;
    
    let new_color = prev_color * decay;
    
    // If the color gets too faint, discard or output zero to prevent denormal/NaN issues
    if (new_color.a < 0.001) {
        return vec4<f32>(0.0);
    }
    
    return new_color;
}
