struct CameraUniform {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct FieldParams {
    width: u32,
    height: u32,
};
@group(0) @binding(1) var<uniform> params: FieldParams;
@group(0) @binding(2) var<storage, read> field_data: array<vec4<f32>>;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    // Assume model.position is a quad from [-0.5, 0.5].
    // Scale by the grid dimensions.
    let world_pos = model.position * vec2<f32>(f32(params.width), f32(params.height));
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 0.0, 1.0);
    out.uv = model.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let x = u32(in.uv.x * f32(params.width));
    let y = u32(in.uv.y * f32(params.height));
    
    let index = y * params.width + x;
    var val = vec4<f32>(0.0);
    if x < params.width && y < params.height {
        val = field_data[index];
    }
    
    let food = clamp(val.x, 0.0, 1.0);
    let toxin = clamp(val.y, 0.0, 1.0);
    let pheromone = clamp(val.z, 0.0, 1.0);
    
    // Food -> deep bioluminescent blue-green
    let food_color = vec3<f32>(0.05, 0.8, 0.6) * food;
    
    // Toxin -> volumetric amber/orange
    let toxin_color = vec3<f32>(0.9, 0.5, 0.1) * toxin;
    
    // Pheromone -> soft violet/magenta
    let pheromone_color = vec3<f32>(0.7, 0.2, 0.8) * pheromone;
    
    let final_color = food_color + toxin_color + pheromone_color;
    
    let alpha = clamp((food + toxin + pheromone) * 0.5, 0.0, 0.8);
    
    return vec4<f32>(final_color, alpha);
}

