// shader.wgsl
@group(0) @binding(0) var t_hdr: texture_2d<f32>;
@group(0) @binding(1) var s_hdr: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    let pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0)
    );

    let current_pos = pos[in_vertex_index];
    out.clip_position = vec4<f32>(current_pos, 0.0, 1.0);

    out.uv = vec2<f32>(
        current_pos.x * 0.5 + 0.5,
        0.5 - current_pos.y * 0.5
    );

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let hdr_color = textureSample(t_hdr, s_hdr, in.uv).rgb;

    // 1. Filmic Tonemapping (ACES-like)
    // This curve prevents high values from clipping too harshly
    // and balances channel intensity.
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    let mapped = (hdr_color * (a * hdr_color + b)) / (hdr_color * (c * hdr_color + d) + e);

    // 2. Gamma Correction
    // Raising to power of 1/2.2 expands the lower end (darker values)
    // making them much more perceptible to the human eye.
    let gamma = 2.2;
    let final_color = pow(max(vec3<f32>(0.0), mapped), vec3<f32>(1.0 / gamma));

    return vec4<f32>(final_color, 1.0);
}
