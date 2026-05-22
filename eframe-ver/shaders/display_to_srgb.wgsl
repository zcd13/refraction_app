// shader.wgsl
@group(0) @binding(0) var t_hdr: texture_2d<f32>;
@group(0) @binding(1) var s_hdr: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Full-screen triangle trick: avoids needing a vertex buffer
    let x = f32(i32(in_vertex_index) << 1u & 2u) - 1.0;
    let y = f32(i32(in_vertex_index) & 2u) - 1.0;
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.tex_coords = vec2<f32>(x * 0.5 + 0.5, 1.0 - (y * 0.5 + 0.5));
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let hdr_color = textureSample(t_hdr, s_hdr, in.tex_coords);

    // Simple Reinhard Tonemapping: color / (color + 1.0)
    // This maps [0, inf] to [0, 1]
    let mapped = hdr_color.rgb / (hdr_color.rgb + vec3<f32>(1.0));

    return vec4<f32>(mapped, hdr_color.a);
}