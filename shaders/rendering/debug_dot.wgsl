struct CameraUniform {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct InstanceInput {
    @location(1) instance_pos: vec2<f32>,
    @location(2) instance_color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    
    // Scale quad to give room for soft edges
    let world_pos = (model.position * 4.0) + instance.instance_pos;
    
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 0.0, 1.0);
    out.color = instance.instance_color;
    out.uv = model.position * 2.0; // -1 to 1 local coordinates
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dist = length(in.uv);
    
    // Soft circular particle with a glowing edge
    if (dist > 1.0) {
        discard;
    }
    
    // Glowing core and fading edge
    let alpha = smoothstep(1.0, 0.0, dist) * in.color.a;
    let core_glow = smoothstep(0.4, 0.0, dist);
    
    let color = mix(in.color.rgb, vec3<f32>(1.0, 1.0, 0.8), core_glow);
    
    return vec4<f32>(color, alpha * 0.8);
}
