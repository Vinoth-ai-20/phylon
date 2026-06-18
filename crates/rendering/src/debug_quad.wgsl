struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

struct CameraUniform {
    view_proj: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct InstanceInput {
    @location(1) position: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) radius: f32,
}

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
    instance: InstanceInput,
) -> VertexOutput {
    // Generate quad vertices as a triangle strip
    var pos = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, -1.0),
    );
    let local_pos = pos[in_vertex_index];
    let world_pos = instance.position + local_pos * instance.radius;

    var out: VertexOutput;
    // We expect the view_proj to handle proper projection to clip space.
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 0.0, 1.0);
    out.color = instance.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
