//@group(0) @binding(0) var<storage, read> rays: array<LightRay>;
@group(0) @binding(0) var<uniform> settings: Settings;

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

    var pos = VERT_ARRAY[v_idx % VERT_LEN];

    // draw geometry
    let aspect = f32(settings.width) / f32(settings.height);
    pos.x /= aspect;

    out.clip_pos = vec4<f32>(pos, 0.0, 1.0);
    out.color = vec4<f32>(1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}