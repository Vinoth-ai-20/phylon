// Debug badge billboard shader (Epic 8.3) — Health/Disease/Category/colony-
// link markers, converted from the Phase 7 flat world-space-XY quad
// technique to true camera-facing billboards. `pos_a`/`pos_b` are now
// world-space Vec3 (Phase 8); the AABB-in-a-plane technique the old shader
// used is unchanged in spirit, just rebased onto the camera's own right/up
// basis instead of the world XY plane, so a badge always faces the camera
// regardless of view direction. Depth-tested (not depth-written) against
// `OrganismRenderer`'s shared depth buffer so badges correctly hide behind
// nearer organisms.

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_pos: vec2<f32>,
    @location(2) pos_a: vec2<f32>,
    @location(3) pos_b: vec2<f32>,
    @location(4) @interpolate(flat) segment_type: u32,
    @location(5) @interpolate(flat) max_radius: f32,
}

struct CameraUniform {
    view_proj: mat4x4<f32>,
    right: vec3<f32>,
    _pad0: f32,
    up: vec3<f32>,
    _pad1: f32,
}
@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct InstanceInput {
    @location(1) pos_a: vec3<f32>,
    @location(2) pos_b: vec3<f32>,
    @location(3) color: vec4<f32>,
    @location(4) radius: f32,
    @location(5) segment_type: u32,
}

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
    instance: InstanceInput,
) -> VertexOutput {
    // Clamp radius based on SegmentType
    // Head=0, Torso=1, Muscle=2, Tail=3, Fin=4, Line=99
    var clamped_radius = instance.radius;
    var max_allowed = 20.0;
    if (instance.segment_type == 0u) { max_allowed = 15.0; }
    else if (instance.segment_type == 1u) { max_allowed = 20.0; }
    else if (instance.segment_type == 2u) { max_allowed = 12.0; }
    else if (instance.segment_type == 3u) { max_allowed = 8.0; }
    else if (instance.segment_type == 4u) { max_allowed = 10.0; }
    else if (instance.segment_type == 99u) { max_allowed = 100.0; } // lines can be thick

    if (clamped_radius > max_allowed) {
        clamped_radius = max_allowed;
    }

    // Project the world-space bone into the camera's own billboard plane
    // (its right/up basis), using `pos_a` as the plane's local origin — the
    // same min/max-bounding-box-in-a-plane technique the old shader used,
    // just rebased from world XY onto the camera-facing plane so the badge
    // always faces the viewer.
    let local_a = vec2<f32>(0.0, 0.0);
    let offset_b = instance.pos_b - instance.pos_a;
    let local_b = vec2<f32>(dot(offset_b, camera.right), dot(offset_b, camera.up));

    let pad = clamped_radius * 1.5;
    let mn = min(local_a, local_b) - vec2<f32>(pad, pad);
    let mx = max(local_a, local_b) + vec2<f32>(pad, pad);

    var pos = array<vec2<f32>, 4>(
        vec2<f32>(mn.x, mx.y),
        vec2<f32>(mn.x, mn.y),
        vec2<f32>(mx.x, mx.y),
        vec2<f32>(mx.x, mn.y),
    );
    let local_pos = pos[in_vertex_index];
    let world_pos = instance.pos_a + local_pos.x * camera.right + local_pos.y * camera.up;

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.color = instance.color;
    out.local_pos = local_pos;
    out.pos_a = local_a;
    out.pos_b = local_b;
    out.segment_type = instance.segment_type;
    out.max_radius = clamped_radius;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let pa = in.local_pos - in.pos_a;
    let ba = in.pos_b - in.pos_a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    var dist = 0.0;
    if (length(ba) < 0.0001) {
        dist = length(pa);
    } else {
        dist = length(pa - ba * h);
    }

    // Crisp threshold for alpha
    let fw = min(fwidth(dist), in.max_radius * 0.99);
    let alpha = smoothstep(in.max_radius + fw, in.max_radius - fw, dist);

    if (alpha < 0.01) {
        discard;
    }

    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
