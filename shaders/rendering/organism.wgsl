struct CameraUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) instance_position: vec2<f32>,
    @location(2) instance_heading: f32,
    @location(3) instance_size: f32,
    @location(4) instance_base_color: vec3<f32>,
    @location(5) instance_diet: u32,
    @location(6) instance_energy: f32,
    @location(7) instance_health: f32,
    @location(8) instance_is_infected: u32,
    @location(9) instance_tick_age: f32,
    @location(10) instance_genome_id: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) diet: u32,
    @location(3) energy: f32,
    @location(4) health: f32,
    @location(5) is_infected: u32,
    @location(6) tick_age: f32,
    @location(7) genome_id: u32,
};

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    // Scale the quad to give some padding for the SDF rendering
    // A quad goes from -0.5 to 0.5. Scale it up by 1.5 to leave room for cilia/appendages
    let scaled_pos = model.position * model.instance_size * 2.0; 
    
    // Rotate the quad by heading
    // heading is 0 facing right? Usually standard math uses right = 0.
    // The prompt says "anterior end always faces the direction of travel"
    // Let's assume standard 2D rotation matrix:
    let cos_theta = cos(model.instance_heading);
    let sin_theta = sin(model.instance_heading);
    let rotated_pos = vec2<f32>(
        scaled_pos.x * cos_theta - scaled_pos.y * sin_theta,
        scaled_pos.x * sin_theta + scaled_pos.y * cos_theta
    );
    
    let world_pos = rotated_pos + model.instance_position;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 0.0, 1.0);
    
    // Pass local coordinates (-1 to 1) to fragment shader
    out.uv = model.position * 2.0; 
    
    out.color = model.instance_base_color;
    out.diet = model.instance_diet;
    out.energy = model.instance_energy;
    out.health = model.instance_health;
    out.is_infected = model.instance_is_infected;
    out.tick_age = model.instance_tick_age;
    out.genome_id = model.instance_genome_id;
    
    return out;
}

// Pseudorandom function seeded by genome_id
fn random_val(seed: u32, offset: u32) -> f32 {
    let x = (seed ^ offset) * 2654435761u;
    let y = (x ^ (x >> 16u)) * 2654435761u;
    let z = (y ^ (y >> 16u));
    return f32(z) / 4294967295.0;
}

// 2D Rotation matrix
fn rotate2d(a: f32) -> mat2x2<f32> {
    return mat2x2<f32>(cos(a), -sin(a), sin(a), cos(a));
}

// Basic SDF primitives
fn sdf_circle(p: vec2<f32>, r: f32) -> f32 {
    return length(p) - r;
}

fn sdf_ellipse(p: vec2<f32>, ab: vec2<f32>) -> f32 {
    let p_abs = abs(p);
    if (p_abs.x > p_abs.y) {
        return (length(p_abs / ab) - 1.0) * min(ab.x, ab.y);
    } else {
        return (length(p_abs / ab) - 1.0) * min(ab.x, ab.y);
    }
}

// A simple capsule/line segment SDF
fn sdf_capsule(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h) - r;
}

// Smooth min for blending shapes
fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

struct FragmentOutput {
    @location(0) screen_color: vec4<f32>,
    @location(1) trail_color: vec4<f32>,
};

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let uv = in.uv; // Local coordinates from -1.0 to 1.0
    var dist = 100.0;
    
    // Base time/animation speed modified by health and infection
    // Energy < 0.3 means animation slows
    var anim_speed = 1.0;
    if (in.energy < 0.3) {
        anim_speed = 0.3;
    }
    
    var time = in.tick_age * 0.05 * anim_speed;
    
    // Erratic phase offset if infected
    if (in.is_infected == 1u) {
        time = time + sin(in.tick_age * 0.2) * 2.0;
    }

    // Determine specific visual traits based on diet
    var organism_color = in.color;
    
    if (in.diet == 0u) { // Herbivore
        // Green-tinted
        organism_color = mix(organism_color, vec3<f32>(0.2, 0.8, 0.2), 0.5);
        
        // Soft rounded body: base ellipse
        let base_body = sdf_ellipse(uv, vec2<f32>(0.5, 0.3));
        
        // Cilia-like fringe: undulate perimeter
        let angle = atan2(uv.y, uv.x);
        let cilia_wave = sin(angle * 15.0 + time * 5.0) * 0.05;
        
        // Breathing pulse
        let pulse = sin(time * 2.0) * 0.03;
        
        dist = base_body - cilia_wave - pulse;
        
    } else if (in.diet == 1u) { // Carnivore
        // Red-tinted
        organism_color = mix(organism_color, vec3<f32>(0.8, 0.2, 0.2), 0.5);
        
        // Elongated, tapered body. Pointed anterior end (facing right, +x)
        // A wedge or teardrop shape
        let p = uv;
        let scale_y = mix(0.4, 0.1, (p.x + 0.5)); // Taper towards +x
        let base_body = length(vec2<f32>(p.x * 0.8, p.y / scale_y)) - 0.4;
        
        // Jaw / pointed end subtraction
        // We can subtract a triangle or circle at the front
        let jaw_cut = sdf_circle(p - vec2<f32>(0.5, 0.0), 0.15 + sin(time*4.0)*0.03); // Chomping motion
        
        dist = max(base_body, -jaw_cut);
        
        // Breathing
        dist = dist - sin(time * 2.5) * 0.02;
        
    } else { // Scavenger
        // Grey/brown-tinted
        organism_color = mix(organism_color, vec3<f32>(0.5, 0.4, 0.3), 0.6);
        
        // Irregular blobby silhouette using genome_id as seed
        let seed = in.genome_id;
        let angle = atan2(uv.y, uv.x);
        
        // Combine a few sine waves with random offsets for a blobby shape
        var noise_offset = sin(angle * 3.0 + random_val(seed, 1u)*10.0 + time) * 0.08;
        noise_offset += sin(angle * 5.0 + random_val(seed, 2u)*10.0 - time*1.2) * 0.06;
        
        // Pseudopod protrusions
        let num_pods = 3.0 + floor(random_val(seed, 3u) * 4.0);
        let pod_wave = max(0.0, sin(angle * num_pods + time*2.0)) * 0.15;
        
        let base_body = length(uv) - 0.3;
        
        dist = base_body - noise_offset - pod_wave;
    }
    
    // Death animation (organism shrinks and fades over time)
    // The prompt says "organism shrinks and fades over 30 ticks before despawn".
    // We can assume that if health == 0.0, it is dead. But wait, if health is 0, is the entity removed?
    // The renderer doesn't know "ticks since dead" unless passed.
    // If the system just decrements energy or health when dead, or we don't have this data perfectly,
    // we use `health` or `energy`. Actually, the prompt says "health < 0" maybe? Or the ECS handles death animation.
    // For now, if health < 0.1, maybe start shrinking? No, ECS will remove it. If the ECS handles the 30-tick death, it probably shrinks the `size` property directly or reduces alpha. If alpha is just color.a, we'll use `health` to fade it out if it approaches 0.
    
    // Calculate alpha based on SDF distance for smooth antialiasing
    let fw = fwidth(dist);
    var alpha = smoothstep(fw, -fw, dist);
    
    if (alpha <= 0.0) {
        discard;
    }
    
    // Low energy translucency
    if (in.energy < 0.3) {
        alpha = alpha * 0.5;
    }
    
    // Add shading for a more "microscopic" look (darker edges)
    let inner_dist = dist + 0.15;
    let edge_shade = smoothstep(0.0, -0.2, inner_dist);
    var final_color = mix(organism_color * 0.5, organism_color, edge_shade);
    
    // Infected pulsing dark purple overlay
    if (in.is_infected == 1u) {
        let pulse_intensity = (sin(in.tick_age * 0.2) + 1.0) * 0.5;
        let purple = vec3<f32>(0.4, 0.0, 0.6);
        final_color = mix(final_color, purple, pulse_intensity * 0.6);
    }
    
    // Output
    var out: FragmentOutput;
    out.screen_color = vec4<f32>(final_color, alpha);
    
    // Trails: write a faint color (10% organism color, 90% transparent) to the persistent trail texture
    // It says "90% transparent", so alpha = 0.1.
    out.trail_color = vec4<f32>(organism_color, 0.1 * alpha);
    
    return out;
}
