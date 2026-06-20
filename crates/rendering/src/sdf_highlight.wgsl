// ── Highlight Composite pass ────────────────────────────────────────────────
// Samples the accumulated density texture to draw an outline around
// the highlighted (hovered/selected) organisms.

@group(0) @binding(0) var accum_texture: texture_2d<f32>;
@group(0) @binding(1) var accum_sampler: sampler;

struct HighlightUniform {
    color: vec4<f32>,
}
@group(1) @binding(0) var<uniform> highlight: HighlightUniform;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0)       uv:       vec2<f32>,
}

@vertex
fn vs_highlight(@builtin(vertex_index) vi: u32) -> VertexOutput {
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
fn fs_highlight(in: VertexOutput) -> @location(0) vec4<f32> {
    let accum = textureSample(accum_texture, accum_sampler, in.uv);
    let density = accum.a;

    // The highlight should be a crisp, solid outline with no gradient.
    // We use a band from outer_edge to inner_edge.
    // For 1-pixel anti-aliasing, we calculate the screen-space derivative of density.
    let df = fwidth(density);
    let aa_width = df * 0.7; // ~0.7 pixels of anti-aliasing for smooth edges
    
    // Choose the bounds of our crisp line. The skin boundary is at 0.7.
    // The width of the line is defined by how far the outer edge is from 0.7.
    let outer_boundary = 0.45;
    let inner_boundary = 0.70;
    
    let outer_alpha = smoothstep(outer_boundary - aa_width, outer_boundary + aa_width, density);
    let inner_alpha = smoothstep(inner_boundary - aa_width, inner_boundary + aa_width, density);
    
    // The ring is the solid band between outer and inner.
    let ring_alpha = clamp(outer_alpha - inner_alpha, 0.0, 1.0);
    
    if ring_alpha < 0.01 {
        discard;
    }
    
    // Output highlight color, scaled by ring_alpha and the requested color's alpha
    return vec4<f32>(highlight.color.rgb, ring_alpha * highlight.color.a);
}
