struct CameraUniform {
    view_proj: mat4x4<f32>,
    ui_flags: vec4<u32>, // [show_species, show_grid, show_sensors, show_disease]
}
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct InstanceInput {
    @location(1) pos_head_spd: vec4<f32>,
    @location(2) size_color: vec4<f32>,
    @location(3) states_0: vec4<f32>,
    @location(4) states_1: vec3<f32>,
    @location(5) hox_genes: vec2<u32>,
    @location(6) hox_sizes_0: vec4<f32>,
    @location(7) hox_sizes_1: vec3<f32>,
    @location(8) hox_appends_0: vec4<u32>,
    @location(9) hox_appends_1: vec3<u32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_uv:    vec2<f32>,
    @location(1) heading:     f32,
    @location(2) speed:       f32,
    @location(3) size:        f32,
    @location(4) base_color:  vec3<f32>,
    @location(5) diet:        u32,
    @location(6) energy:      f32,
    @location(7) health:      f32,
    @location(8) is_infected: u32,
    @location(9) tick_age:    f32,
    @location(10) species_id: u32,
    @location(11) death_age:  f32,
    
    // Pass HOX data to fragment
    @location(12) hox_genes_a: u32,
    @location(13) hox_genes_b: u32,
    @location(14) hox_sizes_0: vec4<f32>,
    @location(15) hox_sizes_1: vec3<f32>,
    @location(16) hox_appends_0: vec4<u32>,
    @location(17) hox_appends_1: vec3<u32>,
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

    let corner = model.position * 2.0;
    
    let angle = inst.pos_head_spd.z;
    let c = cos(angle);
    let s = sin(angle);

    let rotated = vec2<f32>(
        corner.x * c - corner.y * s,
        corner.x * s + corner.y * c,
    );

    out.size        = inst.size_color.x;
    let world_pos = inst.pos_head_spd.xy + rotated * out.size * 1.5;

    out.clip_position = camera.view_proj * vec4(world_pos, 0.0, 1.0);

    out.local_uv    = corner;
    out.heading     = inst.pos_head_spd.z;
    out.speed       = inst.pos_head_spd.w;
    out.base_color  = inst.size_color.yzw;
    out.diet        = u32(inst.states_0.x + 0.1);
    out.energy      = inst.states_0.y;
    out.health      = inst.states_0.z;
    out.is_infected = u32(inst.states_0.w);
    out.tick_age    = inst.states_1.x;
    out.species_id  = u32(inst.states_1.y);
    out.death_age   = inst.states_1.z;

    out.hox_genes_a = inst.hox_genes.x;
    out.hox_genes_b = inst.hox_genes.y;
    out.hox_sizes_0 = inst.hox_sizes_0;
    out.hox_sizes_1 = inst.hox_sizes_1;
    out.hox_appends_0 = inst.hox_appends_0;
    out.hox_appends_1 = inst.hox_appends_1;

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

fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

struct HoxPlan {
    seg_types:   array<u32, 7>,
    seg_sizes:   array<f32, 7>,
    app_types:   array<u32, 7>,
    app_counts:  array<u32, 7>,
    seg_count:   u32,
}

fn unpack_hox(in: VertexOutput) -> HoxPlan {
    var plan: HoxPlan;

    plan.seg_types[0] = (in.hox_genes_a)       & 0xFFu;
    plan.seg_types[1] = (in.hox_genes_a >> 8)  & 0xFFu;
    plan.seg_types[2] = (in.hox_genes_a >> 16) & 0xFFu;
    plan.seg_types[3] = (in.hox_genes_a >> 24) & 0xFFu;
    plan.seg_types[4] = (in.hox_genes_b)       & 0xFFu;
    plan.seg_types[5] = (in.hox_genes_b >> 8)  & 0xFFu;
    plan.seg_types[6] = (in.hox_genes_b >> 16) & 0xFFu;
    plan.seg_count    = (in.hox_genes_b >> 24) & 0xFFu;

    plan.seg_sizes[0] = in.hox_sizes_0.x;
    plan.seg_sizes[1] = in.hox_sizes_0.y;
    plan.seg_sizes[2] = in.hox_sizes_0.z;
    plan.seg_sizes[3] = in.hox_sizes_0.w;
    plan.seg_sizes[4] = in.hox_sizes_1.x;
    plan.seg_sizes[5] = in.hox_sizes_1.y;
    plan.seg_sizes[6] = in.hox_sizes_1.z;

    var appends = array<u32, 7>(
        in.hox_appends_0.x, in.hox_appends_0.y, in.hox_appends_0.z, in.hox_appends_0.w,
        in.hox_appends_1.x, in.hox_appends_1.y, in.hox_appends_1.z
    );

    for (var i: i32 = 0; i < 7; i++) {
        plan.app_types[i]  = appends[i] & 0xFFu;
        plan.app_counts[i] = (appends[i] >> 8) & 0xFFu;
    }

    return plan;
}

// Type 0: SMOOTH — simple rounded blob
fn seg_smooth(p: vec2<f32>, centre: vec2<f32>,
              r: f32) -> f32 {
    return length(p - centre) - r;
}

// Type 1: TAPERED — cone narrowing toward tail (-Y)
fn seg_tapered(p: vec2<f32>, centre: vec2<f32>,
               r: f32) -> f32 {
    let lp = p - centre;
    let taper = r * (0.9 - lp.y * 0.6);
    return length(lp * vec2(1.0, 0.6)) - max(taper, 0.05);
}

// Type 2: BULGE — wider, energy storage segment
fn seg_bulge(p: vec2<f32>, centre: vec2<f32>,
             r: f32) -> f32 {
    return length(p - centre) - r * 1.35;
}

// Type 3: ARMOURED — flattened disc
fn seg_armoured(p: vec2<f32>, centre: vec2<f32>,
                r: f32) -> f32 {
    let lp = p - centre;
    return length(lp * vec2(1.0, 2.2)) - r;
}

// Type 4: NECK — narrow connector
fn seg_neck(p: vec2<f32>, centre: vec2<f32>,
            r: f32) -> f32 {
    return length(p - centre) - r * 0.35;
}

// Type 5: HEAD — anterior cap with sensory pit
fn seg_head(p: vec2<f32>, centre: vec2<f32>,
            r: f32) -> f32 {
    let dome = length(p - centre) - r;
    let pit = length(p - (centre + vec2(0.0, r * 0.7)))
              - r * 0.22;
    return max(dome, -pit - 0.02);
}

// Type 6: TAIL — streamlined posterior terminus
fn seg_tail(p: vec2<f32>, centre: vec2<f32>,
            r: f32) -> f32 {
    let lp = p - centre;
    let d = length(lp * vec2(1.0, 0.5)) - r * 0.7;
    let point = length(lp - vec2(0.0, -r * 0.4)) - r * 0.3;
    return smin(d, point, 0.08);
}

// Type 0: NONE
fn app_none(p: vec2<f32>, anchor: vec2<f32>,
            side: f32, t: f32, speed: f32) -> f32 {
    return 999.0;
}

// Type 1: CILIA — short oscillating hair
fn app_cilia(p: vec2<f32>, anchor: vec2<f32>,
             side: f32, t: f32, speed: f32) -> f32 {
    let wave = sin(t * 0.25 + anchor.y * 8.0) * 0.06;
    let tip = anchor + vec2(side * (0.12 + wave), 0.0);
    return sd_capsule(p, anchor, tip, 0.012);
}

// Type 2: FLAGELLUM — long whipping tail appendage
fn app_flagellum(p: vec2<f32>, anchor: vec2<f32>,
                 side: f32, t: f32, speed: f32) -> f32 {
    let wave1 = sin(t * 0.15) * 0.15;
    let wave2 = sin(t * 0.15 + 1.2) * 0.12;
    let mid = anchor + vec2(side * wave1, -0.2);
    let tip = anchor + vec2(side * wave2, -0.42);
    let d1 = sd_capsule(p, anchor, mid, 0.018);
    let d2 = sd_capsule(p, mid, tip, 0.012);
    return min(d1, d2);
}

// Type 3: PSEUDOPOD — extending amoeba arm
fn app_pseudopod(p: vec2<f32>, anchor: vec2<f32>,
                 side: f32, t: f32, speed: f32) -> f32 {
    let extend = sin(t * 0.05 + anchor.y * 3.0)
                 * 0.5 + 0.5;
    let tip = anchor + vec2(
        side * (0.1 + extend * 0.15),
        extend * 0.12
    );
    return sd_capsule(p, anchor, tip,
                      0.04 + extend * 0.015);
}

// Type 4: FIN — rigid flat stabiliser
fn app_fin(p: vec2<f32>, anchor: vec2<f32>,
           side: f32, t: f32, speed: f32) -> f32 {
    let tip = anchor + vec2(side * 0.22, 0.08);
    return sd_capsule(p, anchor, tip, 0.018);
}

// Type 5: SPINE — sharp defensive protrusion
fn app_spine(p: vec2<f32>, anchor: vec2<f32>,
             side: f32, t: f32, speed: f32) -> f32 {
    let tip = anchor + vec2(side * 0.18, 0.05);
    return sd_capsule(p, anchor, tip, 0.008);
}

// Type 6: JAW — anterior grasping appendage
fn app_jaw(p: vec2<f32>, anchor: vec2<f32>,
           side: f32, t: f32, speed: f32) -> f32 {
    let open = speed * 0.3;
    let tip = anchor + vec2(
        side * (0.14 + open * 0.08),
        0.1 + open * 0.05
    );
    return sd_capsule(p, anchor, tip, 0.022);
}

struct BodyResult {
    sdf:         f32,   // distance to organism surface
    seg_index:   i32,   // which segment this pixel is in
    seg_type:    u32,   // type of that segment
    is_appendage: bool, // true if pixel is an appendage
    app_type:    u32,   // appendage type if is_appendage
    depth:       f32,   // 0=surface, 1=deepest interior
}

fn build_hox_body(uv: vec2<f32>, plan: HoxPlan,
                  tick_age: f32, speed: f32) -> BodyResult
{
    var p = plan;
    var result: BodyResult;
    result.sdf       = 999.0;
    result.seg_index = -1;
    result.seg_type  = 0u;
    result.is_appendage = false;
    result.app_type  = 0u;
    result.depth     = 0.0;

    let n = i32(p.seg_count);

    let total_height = 1.7;
    let seg_height   = total_height / f32(n);

    var body_sdf  = 999.0;
    var app_sdf   = 999.0;
    var best_seg  = -1;
    var best_type = 0u;
    var best_app  = 0u;

    for (var i: i32 = 0; i < 7; i++) {
        if i >= n { break; }

        let seg_y = 0.85 - (f32(i) + 0.5) * seg_height;
        let centre = vec2(0.0, seg_y);
        let r = p.seg_sizes[i] * seg_height * 0.6;

        var seg_d = 999.0;
        switch p.seg_types[i] {
            case 0u: { seg_d = seg_smooth(uv, centre, r); }
            case 1u: { seg_d = seg_tapered(uv, centre, r); }
            case 2u: { seg_d = seg_bulge(uv, centre, r); }
            case 3u: { seg_d = seg_armoured(uv, centre, r); }
            case 4u: { seg_d = seg_neck(uv, centre, r); }
            case 5u: { seg_d = seg_head(uv, centre, r); }
            case 6u: { seg_d = seg_tail(uv, centre, r); }
            default: { seg_d = seg_smooth(uv, centre, r); }
        }

        let k = 0.12;
        let h = clamp(0.5 + 0.5*(seg_d - body_sdf)/k,
                      0.0, 1.0);
        let merged = mix(seg_d, body_sdf, h)
                     - k * h * (1.0 - h);

        if merged < body_sdf {
            best_seg  = i;
            best_type = p.seg_types[i];
        }
        body_sdf = merged;

        let app_count = i32(p.app_counts[i]);
        let app_type  = p.app_types[i];
        if app_type == 0u || app_count == 0 { continue; }

        for (var a: i32 = 0; a < 4; a++) {
            if a >= app_count { break; }
            let side = select(-1.0, 1.0, (a % 2) == 0);
            let v_off = (f32(a / 2) - 0.25) * seg_height * 0.4;
            let anchor = vec2(
                side * r * 0.9,
                seg_y + v_off
            );

            var ad = 999.0;
            switch app_type {
                case 1u: { ad = app_cilia(uv, anchor,
                               side, tick_age, speed); }
                case 2u: { ad = app_flagellum(uv, anchor,
                               side, tick_age, speed); }
                case 3u: { ad = app_pseudopod(uv, anchor,
                               side, tick_age, speed); }
                case 4u: { ad = app_fin(uv, anchor,
                               side, tick_age, speed); }
                case 5u: { ad = app_spine(uv, anchor,
                               side, tick_age, speed); }
                case 6u: { ad = app_jaw(uv, anchor,
                               side, tick_age, speed); }
                default: {}
            }

            if ad < app_sdf {
                app_sdf  = ad;
                best_app = app_type;
            }
        }
    }

    result.sdf       = min(body_sdf, app_sdf);
    result.seg_index = best_seg;
    result.seg_type  = best_type;
    result.depth     = clamp(-body_sdf / 0.4, 0.0, 1.0);
    result.is_appendage = app_sdf < body_sdf
                          && app_sdf < 0.0;
    result.app_type  = best_app;
    return result;
}

fn hox_segment_color(
    base_color: vec3<f32>,
    seg_type:   u32,
    depth:      f32,
    diet:       u32
) -> vec3<f32> {
    var color = base_color;

    switch seg_type {
        case 0u: { // SMOOTH
            color = base_color;
        }
        case 1u: { // TAPERED
            color = base_color * 0.85;
        }
        case 2u: { // BULGE
            color = mix(base_color,
                        base_color * 1.3 +
                        vec3(0.05, 0.1, 0.0),
                        0.4);
        }
        case 3u: { // ARMOURED
            let grey = dot(base_color,
                          vec3(0.299, 0.587, 0.114));
            color = mix(base_color, vec3(grey), 0.5);
            color *= 0.9;
        }
        case 4u: { // NECK
            color = base_color * 0.65;
        }
        case 5u: { // HEAD
            color = mix(base_color,
                        base_color + vec3(0.1, 0.1, 0.15),
                        0.5);
        }
        case 6u: { // TAIL
            color = base_color * 0.75;
        }
        default: { color = base_color; }
    }

    color *= 0.45 + depth * 0.6;
    return color;
}

fn hox_appendage_color(
    base_color: vec3<f32>,
    app_type:   u32
) -> vec4<f32> {
    switch app_type {
        case 1u: { // CILIA
            return vec4(base_color * 1.2, 0.55);
        }
        case 2u: { // FLAGELLUM
            return vec4(base_color * 0.9, 0.45);
        }
        case 3u: { // PSEUDOPOD
            return vec4(base_color * 1.1, 0.60);
        }
        case 4u: { // FIN
            return vec4(base_color * 0.8, 0.70);
        }
        case 5u: { // SPINE
            return vec4(base_color * 1.4 +
                        vec3(0.2, 0.2, 0.2), 0.85);
        }
        case 6u: { // JAW
            let c = base_color * 0.5 + vec3(0.1, 0.0, 0.0);
            return vec4(c, 0.90);
        }
        default: { return vec4(base_color, 0.5); }
    }
}

fn diet_base_color(diet: u32, species_id: u32) -> vec3<f32> {
    let variation = (f32(species_id % 64u) / 64.0 - 0.5) * 0.15;
    switch diet {
        case 0u: { // Herbivore
            return vec3(0.15 + variation, 0.88, 0.12 - variation);
        }
        case 1u: { // Carnivore
            return vec3(0.92, 0.10 + variation, 0.08 - variation * 0.5);
        }
        case 2u: { // Scavenger
            return vec3(0.08 - variation * 0.5, 0.65 + variation, 0.95);
        }
        default: { return vec3(0.7, 0.7, 0.7); }
    }
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let plan   = unpack_hox(in);
    let base   = diet_base_color(in.diet, in.species_id);
    let body   = build_hox_body(in.local_uv, plan,
                                in.tick_age, in.speed);

    if body.sdf > 0.02 { discard; }

    var color: vec3<f32>;
    var alpha:  f32;

    if body.is_appendage {
        let ac = hox_appendage_color(base, body.app_type);
        color  = ac.rgb;
        alpha  = ac.a * smoothstep(0.015, -0.005, body.sdf);
    } else {
        color = hox_segment_color(base, body.seg_type,
                                  body.depth, in.diet);
        alpha = smoothstep(0.02, -0.01, body.sdf);
    }

    let rim = smoothstep(0.06, 0.0, abs(body.sdf + 0.03));
    color += rim * (base * 0.6 + vec3(0.3, 0.3, 0.4));

    if body.seg_type == 5u {
        let nuc_d = length(
            (in.local_uv - vec2(0.0, 0.7)) *
            vec2(1.0, 0.6)
        ) - 0.1;
        if nuc_d < 0.0 {
            let t = smoothstep(0.0, -0.05, nuc_d);
            color = mix(color, base * 0.2, t);
        }
    }

    let sp = in.local_uv - vec2(-0.2, 0.3);
    color += exp(-dot(sp, sp) * 16.0) * 0.45;

    let energy_dim = mix(0.4, 1.0,
                         smoothstep(0.0, 0.3, in.energy));

    if in.health < 0.3 {
        let g = dot(color, vec3(0.299, 0.587, 0.114));
        color = mix(vec3(g), color, in.health / 0.3);
    }

    if in.is_infected == 1u {
        let p = sin(in.tick_age * 0.2) * 0.5 + 0.5;
        color = mix(color, vec3(0.5, 0.0, 0.75), rim * p);
    }

    if in.death_age > 0.0 {
        let t = clamp(in.death_age / 30.0, 0.0, 1.0);
        return vec4(color, alpha * energy_dim * (1.0 - t));
    }

    return vec4(color, alpha * energy_dim);
}
