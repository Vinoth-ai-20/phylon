struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    // Generate fullscreen triangle (covers viewport with 1 triangle)
    let x = f32((in_vertex_index << 1u) & 2u);
    let y = f32(in_vertex_index & 2u);
    var out: VertexOutput;
    // Flip Y for texture coordinates because wgpu textures are Y-down
    out.uv = vec2<f32>(x, 1.0 - y);
    out.clip_position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return out;
}

struct FieldConfig {
    min_val: f32,
    max_val: f32,
    camera_pos: vec2<f32>,
    camera_zoom: f32,
    _pad0: u32,
    screen_size: vec2<f32>,
    colormap: u32,
    _pad1: u32,
    world_bounds: vec2<f32>,
}

@group(0) @binding(0) var t_field: texture_2d<f32>;
@group(0) @binding(1) var s_field: sampler;
@group(0) @binding(2) var<uniform> config: FieldConfig;

fn map_colormap(x: f32, 
  v0: vec3<f32>, v1: vec3<f32>, v2: vec3<f32>, v3: vec3<f32>, v4: vec3<f32>, 
  v5: vec3<f32>, v6: vec3<f32>, v7: vec3<f32>, v8: vec3<f32>
) -> vec3<f32> {
  let e1 = 0.13; let e2 = 0.25; let e3 = 0.38; let e4 = 0.50;
  let e5 = 0.63; let e6 = 0.75; let e7 = 0.88;
  
  var col = v0;
  if (x < e1) { col = mix(v0, v1, smoothstep(0.0, e1, x)); }
  else if (x < e2) { col = mix(v1, v2, smoothstep(e1, e2, x)); }
  else if (x < e3) { col = mix(v2, v3, smoothstep(e2, e3, x)); }
  else if (x < e4) { col = mix(v3, v4, smoothstep(e3, e4, x)); }
  else if (x < e5) { col = mix(v4, v5, smoothstep(e4, e5, x)); }
  else if (x < e6) { col = mix(v5, v6, smoothstep(e5, e6, x)); }
  else if (x < e7) { col = mix(v6, v7, smoothstep(e6, e7, x)); }
  else { col = mix(v7, v8, smoothstep(e7, 1.0, x)); }
  
  return clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn viridis(t: f32) -> vec3<f32> {
    return map_colormap(t,
        vec3<f32>(0.26666, 0.00392, 0.32941), vec3<f32>(0.27843, 0.17254, 0.47843),
        vec3<f32>(0.23137, 0.31764, 0.54509), vec3<f32>(0.17254, 0.44313, 0.55686),
        vec3<f32>(0.12941, 0.56470, 0.55294), vec3<f32>(0.15294, 0.67843, 0.50588),
        vec3<f32>(0.36078, 0.78431, 0.38823), vec3<f32>(0.66666, 0.86274, 0.19607),
        vec3<f32>(0.99215, 0.90588, 0.14509)
    );
}

fn magma(t: f32) -> vec3<f32> {
    return map_colormap(t,
        vec3<f32>(0.0, 0.0, 0.01568), vec3<f32>(0.10980, 0.06274, 0.26666),
        vec3<f32>(0.30980, 0.07058, 0.48235), vec3<f32>(0.50588, 0.14509, 0.50588),
        vec3<f32>(0.70980, 0.21176, 0.47843), vec3<f32>(0.89803, 0.31372, 0.39215),
        vec3<f32>(0.98431, 0.52941, 0.38039), vec3<f32>(0.99607, 0.76078, 0.52941),
        vec3<f32>(0.98823, 0.99215, 0.74901)
    );
}

fn plasma(t: f32) -> vec3<f32> {
    return map_colormap(t,
        vec3<f32>(0.05098, 0.03137, 0.52941), vec3<f32>(0.29411, 0.01176, 0.63137),
        vec3<f32>(0.49019, 0.01176, 0.65882), vec3<f32>(0.65882, 0.13333, 0.58823),
        vec3<f32>(0.79607, 0.27450, 0.47450), vec3<f32>(0.89803, 0.41960, 0.36470),
        vec3<f32>(0.97254, 0.58039, 0.25490), vec3<f32>(0.99215, 0.76470, 0.15686),
        vec3<f32>(0.94117, 0.97647, 0.12941)
    );
}

fn inferno(t: f32) -> vec3<f32> {
    return map_colormap(t,
        vec3<f32>(0.0, 0.0, 0.01568), vec3<f32>(0.12156, 0.04705, 0.28235),
        vec3<f32>(0.33333, 0.05882, 0.42745), vec3<f32>(0.53333, 0.13333, 0.41568),
        vec3<f32>(0.72941, 0.21176, 0.33333), vec3<f32>(0.89019, 0.34901, 0.20000),
        vec3<f32>(0.97647, 0.54901, 0.03921), vec3<f32>(0.97647, 0.78823, 0.19607),
        vec3<f32>(0.98823, 1.0, 0.64313)
    );
}

fn turbo(t: f32) -> vec3<f32> {
    let x = clamp(t, 0.0, 1.0);
    let r = 0.13572138 + x * (4.61539260 + x * (-42.66032258 + x * (132.13108234 + x * (-152.94239396 + x * 59.28637943))));
    let g = 0.09140261 + x * (2.19418839 + x * (4.84296658 + x * (-14.18503333 + x * (4.27729857 + x * 2.82956604))));
    let b = 0.10667330 + x * (12.64194608 + x * (-60.58204836 + x * (110.36276771 + x * (-89.90310912 + x * 27.34824973))));
    return clamp(vec3<f32>(r, g, b), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 1. Clip space coordinates [-1, 1]
    let clip_x = in.uv.x * 2.0 - 1.0;
    let clip_y = in.uv.y * 2.0 - 1.0;

    // 2. Map clip space back to world space
    let world_x = clip_x * (config.screen_size.x * 0.5) / config.camera_zoom + config.camera_pos.x;
    let world_y = clip_y * (config.screen_size.y * 0.5) / config.camera_zoom + config.camera_pos.y;

    // 3. Map world space back to the simulation grid space [0..1]
    let grid_u = (world_x / config.world_bounds.x) * 0.5 + 0.5;
    let grid_v = (-world_y / config.world_bounds.y) * 0.5 + 0.5;
    
    let sample_uv = vec2<f32>(grid_u, grid_v);

    let val = textureSample(t_field, s_field, sample_uv).r;
    
    // Configurable constants for tuning the field visualization curve
    let FIELD_MAX_ALPHA: f32 = 0.6;   // Hard cap on opacity
    
    var normalized = 0.0;
    let range = config.max_val - config.min_val;
    if (range > 0.0001) {
        normalized = (val - config.min_val) / range;
    }
    
    // Clamp so we don't pick a color out of bounds
    normalized = clamp(normalized, 0.0, 1.0);
    
    var color = vec3<f32>(0.0);
    if (config.colormap == 0u) {
        color = viridis(normalized);
    } else if (config.colormap == 1u) {
        color = magma(normalized);
    } else if (config.colormap == 2u) {
        color = plasma(normalized);
    } else if (config.colormap == 3u) {
        color = inferno(normalized);
    } else {
        color = turbo(normalized);
    }
    
    // We only want to show the heatmap if there's actually value.
    // If val is effectively <= config.min_val or 0.0, alpha should be low.
    var alpha = normalized * FIELD_MAX_ALPHA;
    
    // Don't draw the absolute lowest noise layer
    if (normalized < 0.01) {
        alpha = 0.0;
    }
    
    return vec4<f32>(color, alpha);
}
