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
//    return vec4<f32>(in.uv.x, in.uv.y, 1.0, 1.0);
//     textureSample uses the calculated UVs
    let color = textureSample(t_hdr, s_hdr, in.uv);
//    return vec4<f32>(color.r, color.g, color.b, 1.0);
//
//    // Simple Reinhard Tonemapping
    let mapped = color.rgb / (color.rgb + vec3<f32>(1.0));
    return vec4<f32>(mapped, color.a);
}
