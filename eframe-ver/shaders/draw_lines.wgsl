struct LightRay {
    @location(0) position: vec2<f32>,
    @location(1) draw_last_position: vec2<f32>,
    @location(2) strength: f32,
    @location(3) wavelength: f32,
    @location(4) ray_status: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: LightRayInstance,
) -> VertexOutput {
    var out: VertexOutput;

    // ray_status == 2 'discard
    if instance.ray_status == 2u {
        out.clip_position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return out;
    }

    var current_pos: vec2<f32>;
    if vertex_index == 0u { // start
        current_pos = instance.draw_last_position;
    } else { // end
        current_pos = instance.position;
    }


    out.clip_position = vec4<f32>(current_pos, 0.0, 1.0);
    out.color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}