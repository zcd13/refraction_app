struct LightRay {
    pos: vec2<f32>,
    last_pos: vec2<f32>,
    wavelength: f32,
    strength: f32,
    status: u32,
    _pad1: u32,          // Explicit padding
    direction: vec2<f32>,
    current_ior: f32,
    _pad2: u32,          // Explicit padding to reach 48 bytes
};

@group(0) @binding(0) var<storage, read> rays: array<LightRay>;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) v_idx: u32,
    @builtin(instance_index) i_idx: u32,
) -> VertexOutput {
    let ray = rays[i_idx];

    // If ray is inactive, collapse it to a single point at zero to "cull" it
    if (ray.status == 2u) {
        return VertexOutput(vec4<f32>(0.0), vec4<f32>(0.0));
    }

    // Determine if we are at the start (0) or end (1) of the line
    let is_end = f32(v_idx % 2u);
    let world_pos = mix(ray.pos, ray.last_pos, is_end);

    var out: VertexOutput;
    out.clip_pos = vec4<f32>(world_pos, 0.0, 1.0);

    // Simple wavelength to RGB (placeholder logic)
    out.color = vec4<f32>(wavelength_to_rgb(ray.wavelength), 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}


fn wavelength_to_rgb(wavelength: f32) -> vec3<f32> {
    var r = 0.0;
    var g = 0.0;
    var b = 0.0;

    if (wavelength >= 380.0 && wavelength < 440.0) {
        r = (440.0 - wavelength) / (440.0 - 380.0);
        g = 0.0;
        b = 1.0;
    } else if (wavelength >= 440.0 && wavelength < 490.0) {
        r = 0.0;
        g = (wavelength - 440.0) / (490.0 - 440.0);
        b = 1.0;
    } else if (wavelength >= 490.0 && wavelength < 510.0) {
        r = 0.0;
        g = 1.0;
        b = (510.0 - wavelength) / (510.0 - 490.0);
    } else if (wavelength >= 510.0 && wavelength < 580.0) {
        r = (wavelength - 510.0) / (580.0 - 510.0);
        g = 1.0;
        b = 0.0;
    } else if (wavelength >= 580.0 && wavelength < 645.0) {
        r = 1.0;
        g = (645.0 - wavelength) / (645.0 - 580.0);
        b = 0.0;
    } else if (wavelength >= 645.0 && wavelength <= 780.0) {
        r = 1.0;
        g = 0.0;
        b = 0.0;
    } else {
        r = 0.0;
        g = 0.0;
        b = 0.0;
    }

    return vec3<f32>(r, g, b);
}