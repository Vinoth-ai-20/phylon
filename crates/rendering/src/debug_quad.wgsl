struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_pos: vec2<f32>,
    @location(2) @interpolate(flat) segment_type: u32,
    @location(3) @interpolate(flat) max_radius: f32,
}

struct CameraUniform {
    view_proj: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct InstanceInput {
    @location(1) position: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) radius: f32,
    @location(4) segment_type: u32,
}

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
    instance: InstanceInput,
) -> VertexOutput {
    var pos = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, -1.0),
    );
    let local_pos = pos[in_vertex_index];
    
    // Clamp radius based on SegmentType
    // Head=0, Torso=1, Muscle=2, Tail=3, Fin=4
    var clamped_radius = instance.radius;
    var max_allowed = 20.0;
    if (instance.segment_type == 0u) { max_allowed = 15.0; }
    else if (instance.segment_type == 1u) { max_allowed = 20.0; }
    else if (instance.segment_type == 2u) { max_allowed = 12.0; }
    else if (instance.segment_type == 3u) { max_allowed = 8.0; }
    else if (instance.segment_type == 4u) { max_allowed = 10.0; }
    
    if (clamped_radius > max_allowed) {
        clamped_radius = max_allowed;
    }

    // Render slightly larger quad for SDF calculation
    let quad_size = clamped_radius * 1.5;
    let world_pos = instance.position + local_pos * quad_size;

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 0.0, 1.0);
    out.color = instance.color;
    out.local_pos = local_pos;
    out.segment_type = instance.segment_type;
    out.max_radius = clamped_radius;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate distance from center (0,0) in local space (-1 to 1)
    let dist = length(in.local_pos);
    
    // To make it an SDF, we need a sharp threshold
    // Let's say radius matches dist = 1.0/1.5 = 0.666
    let threshold = 0.666;
    
    // Crisp threshold for alpha
    // fwidth gives us the amount dist changes per pixel, used for anti-aliasing
    let fw = fwidth(dist);
    let alpha = smoothstep(threshold + fw, threshold - fw, dist);
    
    if (alpha < 0.01) {
        discard;
    }
    
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
