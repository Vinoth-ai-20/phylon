// Capsule-mesh instancing shader (Phase 8, ADR-P8-03) — replaces the
// retired 2-pass SDF metaball accumulate-blend technique.
//
// The vertex shader is the "oriented-look-at" technique the ADR names: the
// shared unit-capsule mesh (see `capsule_mesh.rs`'s doc comment for its
// local-space convention) is rotated per-instance so its local +Y axis
// aligns with the bone direction (`pos_b - pos_a`), then each vertex is
// reconstructed in world space from whichever of the 3 local-space regions
// (bottom cap / cylinder body / top cap) it belongs to. No per-instance
// rotation/quaternion is stored — only the two endpoints and a radius.
//
// The fragment shader is a single-light Cook-Torrance PBR model (GGX
// distribution, Smith geometry, Schlick Fresnel) plus a flat ambient term,
// driven by `sunlight` (the same `GlobalAtmosphere.sunlight` scalar that
// already tints the background clear color) — untuned roughness/metallic
// constants, same status as every other not-yet-measured value introduced
// this phase.

struct Camera {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad0: f32,
}

struct Light {
    sun_dir: vec3<f32>,
    sunlight: f32,
}

@group(0) @binding(0)
var<uniform> camera: Camera;
@group(0) @binding(1)
var<uniform> light: Light;

struct VertexInput {
    @location(0) local_position: vec3<f32>,
    @location(1) local_normal: vec3<f32>,
}

struct InstanceInput {
    @location(2) pos_a: vec3<f32>,
    @location(3) pos_b: vec3<f32>,
    @location(4) radius: f32,
    @location(5) color: vec3<f32>,
    @location(6) health: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_position: vec3<f32>,
    @location(2) color: vec3<f32>,
}

@vertex
fn vs_main(vert: VertexInput, inst: InstanceInput) -> VertexOutput {
    let bone_vec = inst.pos_b - inst.pos_a;
    let bone_len = length(bone_vec);
    // Point-like entities (pellets, corpses) have `pos_a == pos_b` — fall
    // back to a fixed axis so the capsule degenerates to a sphere instead
    // of a NaN-producing zero-length basis.
    let up = select(vec3<f32>(0.0, 0.0, 1.0), bone_vec / max(bone_len, 1e-5), bone_len > 1e-5);

    // Build an orthonormal basis with `up` as local +Y — a reference
    // vector is picked to avoid the near-parallel degenerate case exactly
    // the same way `ui::camera`'s controllers do.
    let reference = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(up.x) > 0.9);
    let right = normalize(cross(reference, up));
    let fwd = cross(up, right);

    var local_offset: vec3<f32>;
    var world_center: vec3<f32>;
    if (vert.local_position.y <= 0.0) {
        world_center = inst.pos_a;
        local_offset = vert.local_position;
    } else if (vert.local_position.y >= 1.0) {
        world_center = inst.pos_b;
        local_offset = vert.local_position - vec3<f32>(0.0, 1.0, 0.0);
    } else {
        world_center = mix(inst.pos_a, inst.pos_b, vert.local_position.y);
        local_offset = vec3<f32>(vert.local_position.x, 0.0, vert.local_position.z);
    }

    let rotated_offset = right * local_offset.x + up * local_offset.y + fwd * local_offset.z;
    let world_pos = world_center + rotated_offset * inst.radius;
    let rotated_normal = right * vert.local_normal.x + up * vert.local_normal.y + fwd * vert.local_normal.z;

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_normal = normalize(rotated_normal);
    out.world_position = world_pos;
    out.color = inst.color * inst.health;
    return out;
}

const PI: f32 = 3.14159265359;
// Untuned organic-material defaults (roadmap's Material pipeline section:
// "fixed to reasonable organic-material defaults... not evolvable").
const ROUGHNESS: f32 = 0.6;
const METALLIC: f32 = 0.05;
const AMBIENT_FLOOR: f32 = 0.12;

fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / max(PI * d * d, 1e-4);
}

fn geometry_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    let g_v = n_dot_v / (n_dot_v * (1.0 - k) + k);
    let g_l = n_dot_l / (n_dot_l * (1.0 - k) + k);
    return g_v * g_l;
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let v = normalize(camera.camera_pos - in.world_position);
    let l = normalize(-light.sun_dir);
    let h = normalize(v + l);

    let n_dot_v = max(dot(n, v), 1e-4);
    let n_dot_l = max(dot(n, l), 0.0);
    let n_dot_h = max(dot(n, h), 0.0);
    let v_dot_h = max(dot(v, h), 0.0);

    let albedo = in.color;
    let f0 = mix(vec3<f32>(0.04), albedo, METALLIC);

    let d = distribution_ggx(n_dot_h, ROUGHNESS);
    let g = geometry_smith(n_dot_v, n_dot_l, ROUGHNESS);
    let f = fresnel_schlick(v_dot_h, f0);

    let specular = (d * g * f) / max(4.0 * n_dot_v * n_dot_l, 1e-4);
    let k_d = (vec3<f32>(1.0) - f) * (1.0 - METALLIC);
    let diffuse = k_d * albedo / PI;

    // Sun intensity scales with the day/night cycle, matching the
    // background clear color's own sunlight blend; a nonzero floor keeps
    // the scene readable at night, same rationale as that clear color's
    // own nonzero night floor.
    let light_intensity = AMBIENT_FLOOR + (1.0 - AMBIENT_FLOOR) * light.sunlight;
    let direct = (diffuse + specular) * light_intensity * n_dot_l;
    let ambient = albedo * AMBIENT_FLOOR * mix(0.5, 1.0, light.sunlight);

    let color = ambient + direct;
    return vec4<f32>(color, 1.0);
}

// ── Highlight pass ──────────────────────────────────────────────────────
// Reuses `vs_main` verbatim (same oriented-capsule reconstruction, driven
// by the same instance buffer format at a slightly inflated radius — the
// caller passes highlight instances through the same pipeline input
// layout). Only the fragment shader differs: a flat, unlit color instead
// of PBR shading, matching the "inverted hull outline" technique's own
// requirement of a solid silhouette color, not lit geometry.

struct HighlightColor {
    color: vec4<f32>,
}

@group(1) @binding(0)
var<uniform> highlight_color: HighlightColor;

@fragment
fn fs_highlight(in: VertexOutput) -> @location(0) vec4<f32> {
    return highlight_color.color;
}
