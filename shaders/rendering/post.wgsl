struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(position, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@group(0) @binding(0)
var hdr_texture: texture_2d<f32>;
@group(0) @binding(1)
var hdr_sampler: sampler;

// Simple Gaussian weights for bloom
fn gaussian_blur(tex: texture_2d<f32>, samp: sampler, uv: vec2<f32>, dir: vec2<f32>) -> vec3<f32> {
    var color = vec3<f32>(0.0);
    let offset = vec2<f32>(1.0 / 1280.0, 1.0 / 720.0); // Approximation
    var weights = array<f32, 5>(0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);
    
    color += textureSample(tex, samp, uv).rgb * weights[0];
    for (var i = 1; i < 5; i++) {
        color += textureSample(tex, samp, uv + dir * offset * f32(i)).rgb * weights[i];
        color += textureSample(tex, samp, uv - dir * offset * f32(i)).rgb * weights[i];
    }
    return color;
}

// Extract bright parts for bloom
fn bright_pass(c: vec3<f32>) -> vec3<f32> {
    let brightness = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    if (brightness > 1.0) {
        return c;
    }
    return vec3<f32>(0.0);
}

// ACES tonemapping approximation
fn aces_approx(v: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((v * (a * v + b)) / (v * (c * v + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(hdr_texture, hdr_sampler, in.uv).rgb;
    
    // Depth of Field (Vignette Blur)
    let dist_from_center = length(in.uv - vec2<f32>(0.5));
    let blur_amount = smoothstep(0.4, 0.8, dist_from_center);
    
    // Simplistic blur
    var blurred_color = color;
    if (blur_amount > 0.0) {
        let dir = normalize(in.uv - vec2<f32>(0.5)) * 2.0; // simple directional blur
        blurred_color = mix(color, gaussian_blur(hdr_texture, hdr_sampler, in.uv, dir), blur_amount);
    }
    
    // Bloom
    let bright = bright_pass(color);
    let bloom_color = gaussian_blur(hdr_texture, hdr_sampler, in.uv, vec2<f32>(1.0, 0.0)) * 0.5
                    + gaussian_blur(hdr_texture, hdr_sampler, in.uv, vec2<f32>(0.0, 1.0)) * 0.5;
                    
    let final_hdr = blurred_color + bright_pass(bloom_color) * 0.5;

    // Tonemapping
    let ldr = aces_approx(final_hdr);
    
    return vec4<f32>(ldr, 1.0);
}
