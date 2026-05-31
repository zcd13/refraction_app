//@group(0) @binding(0) var<storage, read> rays: array<LightRay>;
@group(0) @binding(0) var<uniform> settings: Settings;
@group(1) @binding(0) var<storage, read> geometry: array<vec2<f32>>;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) v_idx: u32,
    @builtin(instance_index) i_idx: u32,
) -> VertexOutput {
    var out: VertexOutput;

    var pos = geometry[v_idx % arrayLength(&geometry)];

    // draw geometry
    pos.x /= settings.aspect;

    out.clip_pos = vec4<f32>(pos, 0.0, 1.0);
    out.color = vec4<f32>(1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}