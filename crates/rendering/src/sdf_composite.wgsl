// ── Composite pass ────────────────────────────────────────────────────────
// Full-screen triangle that samples the accumulated density texture.
// Pixels where density ≥ 1.0 are "inside" the organism skin and get
// composited onto the swapchain with a smooth alpha edge.

@group(0) @binding(0) var accum_texture: texture_2d<f32>;
@group(0) @binding(1) var accum_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0)       uv:       vec2<f32>,
}

@vertex
fn vs_composite(@builtin(vertex_index) vi: u32) -> VertexOutput {
    // Full-screen triangle (covers the entire NDC cube with 3 vertices)
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 3.0,  1.0),
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 2.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(2.0, 0.0),
    );

    var out: VertexOutput;
    out.clip_pos = vec4<f32>(positions[vi], 0.0, 1.0);
    out.uv       = uvs[vi];
    return out;
}

@fragment
fn fs_composite(in: VertexOutput) -> @location(0) vec4<f32> {
    let accum = textureSample(accum_texture, accum_sampler, in.uv);
    let density = accum.a;

    // Threshold at 1.0 with a narrow smoothstep band for anti-aliasing.
    let alpha = smoothstep(0.7, 1.0, density);

    if alpha < 0.01 {
        discard;
    }

    // Organism body colour — un-premultiply the accumulated color
    let base_color = accum.rgb / max(density, 0.0001);
    
    // Add slight luminance variation driven by density
    let brightness = clamp(density * 0.4 + 0.6, 0.0, 1.0);
    let color      = base_color * brightness;

    return vec4<f32>(color, alpha);
}
