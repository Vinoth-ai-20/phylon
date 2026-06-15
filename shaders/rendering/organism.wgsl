struct CameraUniform {
    view_proj: mat4x4<f32>,
    ui_flags: vec4<u32>, // [show_species, show_grid, show_sensors, show_disease]
}
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct InstanceInput {
    @location(1) position:   vec2<f32>,
    @location(2) heading:    f32,
    @location(3) speed:      f32,
    @location(4) size:       f32,
    @location(5) base_color: vec3<f32>,
    @location(6) diet:       f32,
    @location(7) energy:     f32,
    @location(8) health:     f32,
    @location(9) is_infected: f32,
    @location(10) tick_age:  f32,
    @location(11) species_id: f32,
    @location(12) death_age: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_uv:    vec2<f32>,  // -1 to +1
    @location(1) heading:     f32,
    @location(2) speed:       f32,
    @location(3) size:        f32,
    @location(4) base_color:  vec3<f32>,
    @location(5) diet:        f32,
    @location(6) energy:      f32,
    @location(7) health:      f32,
    @location(8) is_infected: f32,
    @location(9) tick_age:    f32,
    @location(10) species_id: f32,
    @location(11) death_age:  f32,
}

struct VertexInput {
    @location(0) position: vec2<f32>,
}

@vertex
fn vs_main(
    model: VertexInput,
    inst: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    // Use the model position from the vertex buffer. It ranges from -0.5 to 0.5.
    // We scale it by 2.0 so the corner is -1.0 to +1.0, which matches the required local_uv range.
    let corner = model.position * 2.0;
    
    let angle = inst.heading;
    let c = cos(angle);
    let s = sin(angle);

    // Rotate corner by heading
    let rotated = vec2<f32>(
        corner.x * c - corner.y * s,
        corner.x * s + corner.y * c,
    );

    // Scale by size and convert to clip space
    let world_pos = inst.position + rotated * inst.size * 1.5;

    // Apply camera transform (camera uniform must be bound)
    out.clip_position = camera.view_proj * vec4(world_pos, 0.0, 1.0);

    // Pass local UV directly — fragment shader uses this for SDF
    out.local_uv    = corner;
    out.heading     = inst.heading;
    out.speed       = inst.speed;
    out.size        = inst.size;
    out.base_color  = inst.base_color;
    out.diet        = inst.diet;
    out.energy      = inst.energy;
    out.health      = inst.health;
    out.is_infected = inst.is_infected;
    out.tick_age    = inst.tick_age;
    out.species_id  = inst.species_id;
    out.death_age   = inst.death_age;

    return out;
}

// SHARED UTILITY FUNCTIONS
fn sd_capsule(p: vec2<f32>, a: vec2<f32>,
              b: vec2<f32>, r: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h  = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h) - r;
}

// ORGANISM SHAPE DESIGNS
fn render_herbivore(uv: vec2<f32>, in: VertexOutput) -> vec4<f32> {
    // Oval body — taller than wide like a real algae cell
    // Scale uv to make an ellipse: compress X axis
    let ellipse_uv = uv * vec2(1.3, 1.0);
    let body_d = length(ellipse_uv) - 0.75;

    if body_d > 0.01 { discard; }

    // Depth: darken toward centre, brighten toward edge
    let depth = -body_d / 0.75; // 0 at edge, 1 at centre
    var color = vec3(0.15, 0.82, 0.18);
    color *= 0.5 + depth * 0.6;

    // Nucleus: dark reddish oval slightly off-centre
    let nuc_offset = vec2(
        (fract(f32(in.species_id) * 0.127) - 0.5) * 0.3,
        (fract(f32(in.species_id) * 0.311) - 0.5) * 0.3
    );
    let nuc_uv = (uv - nuc_offset) * vec2(1.0, 0.7);
    let nuc_d  = length(nuc_uv) - 0.18;
    if nuc_d < 0.0 {
        let t = smoothstep(0.0, -0.1, nuc_d);
        color = mix(color, vec3(0.25, 0.06, 0.04), t);
    }

    // Chloroplasts: 3 small darker green blobs
    for (var i: i32 = 0; i < 3; i++) {
        let seed = f32(in.species_id + f32(i) * 7.0);
        let cx = (fract(seed * 0.173) - 0.5) * 0.8;
        let cy = (fract(seed * 0.431) - 0.5) * 0.9;
        let cr = 0.06 + fract(seed * 0.251) * 0.07;
        let cd = length(uv - vec2(cx, cy)) - cr;
        if cd < 0.0 {
            let t = smoothstep(0.01, -0.03, cd);
            color = mix(color, vec3(0.05, 0.38, 0.05), t);
        }
    }

    // Membrane rim: bright lime glow at the cell edge
    let rim = smoothstep(0.08, 0.0, abs(body_d + 0.03));
    color += rim * vec3(0.4, 1.0, 0.3);

    // Specular highlight: wet look, top-left catch light
    let spec_pos = uv - vec2(-0.2, 0.25);
    let spec = exp(-dot(spec_pos, spec_pos) * 18.0);
    color += spec * vec3(0.9, 1.0, 0.85) * 0.55;

    // Alpha: soft antialiased edge
    let alpha = smoothstep(0.02, -0.01, body_d);

    // State modifiers
    let energy_dim = mix(0.45, 1.0,
                         smoothstep(0.0, 0.3, in.energy));
    if in.health < 0.3 {
        let g = dot(color, vec3(0.299, 0.587, 0.114));
        color = mix(vec3(g), color, in.health / 0.3);
    }
    if u32(in.is_infected) == 1u {
        let p = sin(in.tick_age * 0.2) * 0.5 + 0.5;
        color = mix(color, vec3(0.55, 0.0, 0.75), rim * p);
    }
    if in.death_age > 0.0 {
        let t = clamp(in.death_age / 30.0, 0.0, 1.0);
        return vec4(color, alpha * energy_dim * (1.0 - t));
    }
    return vec4(color, alpha * energy_dim);
}

fn render_carnivore(uv: vec2<f32>, in: VertexOutput) -> vec4<f32> {
    // Elongated body: compress X, stretch Y
    // Head at +Y (top), tail at -Y (bottom)
    let body_uv = uv * vec2(1.8, 1.0);
    let body_d  = length(body_uv) - 0.7;

    // Taper the head: make +Y side sharper
    // Shift the ellipse downward so head is pointier
    let head_uv = (uv - vec2(0.0, 0.15)) * vec2(2.8, 1.0);
    let head_d  = length(head_uv) - 0.55;
    // Smooth union of body and head
    let k = 0.15;
    let h = clamp(0.5 + 0.5*(head_d - body_d)/k, 0.0, 1.0);
    let cell_d = mix(head_d, body_d, h) - k*h*(1.0-h);

    // Two flagella at posterior end (bottom, -Y)
    let t = in.tick_age * 0.12;
    let spd = max(in.speed * 0.6, 0.15);

    let f1_tip = vec2(sin(t) * 0.25 * spd, -0.95);
    let f2_tip = vec2(sin(t + 1.6) * 0.22 * spd, -0.88);
    let f1_d = sd_capsule(uv, vec2(0.04, -0.65),
                          f1_tip, 0.022);
    let f2_d = sd_capsule(uv, vec2(-0.04, -0.65),
                          f2_tip, 0.018);
    let flag_d = min(f1_d, f2_d);

    let combined = min(cell_d, flag_d);
    if combined > 0.01 { discard; }

    let is_flagella = flag_d < cell_d;

    // Flagella color: pale translucent pink-red
    if is_flagella {
        let fa = smoothstep(0.015, -0.005, flag_d) * 0.65;
        return vec4(vec3(0.95, 0.55, 0.55), fa);
    }

    // Body depth
    let depth = -cell_d / 0.7;
    var color = vec3(0.92, 0.12, 0.08);
    color *= 0.5 + depth * 0.55;

    // Elongated nucleus along body axis
    let nuc_d = length(
        (uv - vec2(0.0, 0.1)) * vec2(1.0, 0.5)
    ) - 0.15;
    if nuc_d < 0.0 {
        let t2 = smoothstep(0.0, -0.08, nuc_d);
        color = mix(color, vec3(0.22, 0.02, 0.02), t2);
    }

    // Rim and specular
    let rim = smoothstep(0.07, 0.0, abs(cell_d + 0.03));
    color += rim * vec3(1.1, 0.35, 0.1);
    let spec_pos = uv - vec2(-0.15, 0.3);
    let spec = exp(-dot(spec_pos, spec_pos) * 22.0);
    color += spec * 0.6;

    let alpha = smoothstep(0.02, -0.01, cell_d);
    let ed = mix(0.45, 1.0, smoothstep(0.0, 0.3, in.energy));
    if in.health < 0.3 {
        let g = dot(color, vec3(0.299, 0.587, 0.114));
        color = mix(vec3(g), color, in.health / 0.3);
    }
    if u32(in.is_infected) == 1u {
        let p = sin(in.tick_age * 0.2) * 0.5 + 0.5;
        color = mix(color, vec3(0.55, 0.0, 0.75), rim * p);
    }
    if in.death_age > 0.0 {
        let t3 = clamp(in.death_age / 30.0, 0.0, 1.0);
        return vec4(color, alpha * ed * (1.0 - t3));
    }
    return vec4(color, alpha * ed);
}

fn render_scavenger(uv: vec2<f32>, in: VertexOutput) -> vec4<f32> {
    // Irregular blob from 5 offset circles blended together
    let pulse = 1.0 + sin(in.tick_age * 0.035) * 0.04;
    var blob_d = length(uv) - 0.55 * pulse; // central core

    for (var i: i32 = 0; i < 5; i++) {
        let seed = f32(in.species_id + f32(i) * 13.0);
        let angle = fract(seed * 0.197) * 6.2832;
        let dist  = 0.18 + fract(seed * 0.374) * 0.18;
        let r     = 0.18 + fract(seed * 0.512) * 0.14;
        let cpos  = vec2(cos(angle), sin(angle)) * dist * pulse;
        let cd    = length(uv - cpos) - r * pulse;
        // Smooth union
        let k2 = 0.2;
        let h2 = clamp(0.5 + 0.5*(cd - blob_d)/k2,
                       0.0, 1.0);
        blob_d = mix(cd, blob_d, h2) - k2*h2*(1.0-h2);
    }

    // Pseudopods: 4 slow extending capsule arms
    var pseudo_d = 999.0;
    for (var i: i32 = 0; i < 4; i++) {
        let seed = f32(in.species_id + f32(i) * 31.0 + 100.0);
        let base_angle = fract(seed * 0.263) * 6.2832;
        let extend = sin(in.tick_age * 0.04 +
                         f32(i) * 1.5708) * 0.5 + 0.5;
        let tip_r = 0.45 + extend * 0.25;
        let tip   = vec2(cos(base_angle),
                         sin(base_angle)) * tip_r;
        let base  = vec2(cos(base_angle),
                         sin(base_angle)) * 0.2;
        pseudo_d = min(pseudo_d,
            sd_capsule(uv, base, tip,
                       0.04 + extend * 0.02));
    }

    let combined = min(blob_d, pseudo_d);
    if combined > 0.01 { discard; }

    let is_pseudo = pseudo_d < blob_d && pseudo_d < 0.0;
    if is_pseudo {
        let pa = smoothstep(0.02, -0.01, pseudo_d) * 0.6;
        return vec4(vec3(0.15, 0.72, 0.98), pa);
    }

    let depth = -blob_d / 0.55;
    var color = vec3(0.08, 0.65, 0.95);
    color *= 0.5 + depth * 0.55;

    // Internal granules
    for (var i: i32 = 0; i < 4; i++) {
        let seed = f32(in.species_id + f32(i) * 17.0 + 200.0);
        let gp   = vec2(fract(seed * 0.193) - 0.5,
                        fract(seed * 0.417) - 0.5) * 0.7;
        let gd   = length(uv - gp) - 0.04;
        if gd < 0.0 {
            color = mix(color, vec3(0.02, 0.12, 0.28),
                        smoothstep(0.01, -0.01, gd));
        }
    }

    let rim = smoothstep(0.07, 0.0, abs(blob_d + 0.03));
    color += rim * vec3(0.3, 0.9, 1.3);
    let sp  = uv - vec2(-0.2, 0.2);
    color  += exp(-dot(sp, sp) * 20.0) * 0.5;

    let alpha = smoothstep(0.02, -0.01, combined);
    let ed = mix(0.4, 1.0, smoothstep(0.0, 0.3, in.energy));
    if in.health < 0.3 {
        let g = dot(color, vec3(0.299, 0.587, 0.114));
        color = mix(vec3(g), color, in.health / 0.3);
    }
    if u32(in.is_infected) == 1u {
        let p = sin(in.tick_age * 0.2) * 0.5 + 0.5;
        color = mix(color, vec3(0.55, 0.0, 0.75), rim * p);
    }
    if in.death_age > 0.0 {
        let t = clamp(in.death_age / 30.0, 0.0, 1.0);
        return vec4(color, alpha * ed * (1.0 - t));
    }
    return vec4(color, alpha * ed);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    switch u32(in.diet + 0.1) {
        case 0u: { return render_herbivore(in.local_uv, in); }
        case 1u: { return render_carnivore(in.local_uv, in); }
        case 2u: { return render_scavenger(in.local_uv, in); }
        default: {
            // Magenta circle = diet field not set in ECS
            let d = length(in.local_uv) - 0.8;
            if d > 0.0 { discard; }
            return vec4(1.0, 0.0, 1.0, 1.0);
        }
    }
}
