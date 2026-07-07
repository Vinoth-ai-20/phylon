// ── Accumulation pass ─────────────────────────────────────────────────────
// Each bone is rendered as a world-space AABB quad.
// The fragment outputs a density contribution that is ADDITIVELY blended
// into a single-channel Rgba16Float accumulation texture.
// When all bones of an organism have been rendered, pixels where the
// accumulated density ≥ 1.0 are considered "inside" the organism skin.

struct CameraUniform {
    view_proj: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct BoneInstance {
    @location(1) pos_a:  vec2<f32>,   // World-space endpoint A
    @location(2) pos_b:  vec2<f32>,   // World-space endpoint B
    @location(3) radius: f32,          // Capsule skin radius
    @location(4) color:  vec3<f32>,    // RGB tint (unused in accum; stored for composite)
    @location(5) health: f32,          // Vitality dimming factor [0, 1] (Phase 5, SX-1c) — see SdfBoneInstance's doc comment
}

struct VertexOutput {
    @builtin(position) clip_pos:  vec4<f32>,
    @location(0)       world_pos: vec2<f32>,  // Interpolated world-space fragment position
    @location(1)       pos_a:     vec2<f32>,
    @location(2)       pos_b:     vec2<f32>,
    @location(3)       radius:    f32,
    @location(4)       color:     vec3<f32>,
    @location(5)       health:    f32,
}

@vertex
fn vs_accum(
    @builtin(vertex_index) vi: u32,
    inst: BoneInstance,
) -> VertexOutput {
    // Build a quad that exactly covers the capsule AABB + extra padding for the highlight ring.
    // The highlight ring extends outwards up to ~1.9x the radius (where density drops to 0.1).
    let pad   = inst.radius * 2.0 + 1.0;
    let mn    = min(inst.pos_a, inst.pos_b) - vec2<f32>(pad, pad);
    let mx    = max(inst.pos_a, inst.pos_b) + vec2<f32>(pad, pad);

    // Triangle-strip order: TL, BL, TR, BR
    var corners = array<vec2<f32>, 4>(
        vec2<f32>(mn.x, mx.y),
        vec2<f32>(mn.x, mn.y),
        vec2<f32>(mx.x, mx.y),
        vec2<f32>(mx.x, mn.y),
    );

    let world_pos = corners[vi];

    var out: VertexOutput;
    out.clip_pos  = camera.view_proj * vec4<f32>(world_pos, 0.0, 1.0);
    out.world_pos = world_pos;
    out.pos_a     = inst.pos_a;
    out.pos_b     = inst.pos_b;
    out.radius    = inst.radius;
    out.color     = inst.color;
    out.health    = inst.health;
    return out;
}

/// Standard capsule SDF.
/// Returns the signed distance from point `p` to the line segment (a, b)
/// inflated by radius `r`. Negative values are inside the capsule.
///
/// Reference: Inigo Quilez — https://iquilezles.org/articles/distfunctions2d/
///   d = length(pa - ba * clamp(dot(pa,ba)/dot(ba,ba), 0.0, 1.0)) - r
fn capsule_sdf(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h  = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h) - r;
}

@fragment
fn fs_accum(in: VertexOutput) -> @location(0) vec4<f32> {
    let d = capsule_sdf(in.world_pos, in.pos_a, in.pos_b, in.radius);

    // Density contribution — smooth falloff over one radius worth of distance
    // so that adjacent bones blend smoothly rather than stepping.
    // Clamped to [0, 2] so a single bone can contribute at most 2× threshold.
    let density = clamp(1.0 - d / in.radius, 0.0, 2.0);

    if density <= 0.0 {
        discard;
    }

    // Output density in the A channel and pre-multiplied color in RGB.
    // `health` dims only the color contribution, never `density` — the
    // composite pass's shape/edge thresholding (and the separate highlight
    // pass, which reads only this alpha channel) are therefore completely
    // unaffected by vitality; only the body's rendered color darkens.
    let color_contribution = in.color * density * in.health;
    return vec4<f32>(color_contribution, density);
}
