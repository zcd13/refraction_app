@group(0) @binding(0) var<storage, read_write> data: array<LightRay>;
@group(1) @binding(0) var<uniform> settings: Settings;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    if index >= arrayLength(&data) {
        return;
    }

    if data[index].strength < 0.000000001 || data[index].strength <= 0.0 {
        data[index].strength = 0.0;
        return;
    }

    // collide
    let c = collide(0, 6, data[index].pos, data[index].dir);
    if c.walls_collided != 0 {
        data[index].last_pos = data[index].pos;

        let wavelength = data[index].wave_length_and_ior & 0xFFFFu;
        let float_wave = (f32(wavelength) / 65535.0) * 350.0 + 350.0;
        let ior_glass = get_ior(float_wave, 1.517, 0.0042);

        var n1: f32 = 1.0;       // Default Air
        var n2: f32 = ior_glass; // Default Glass

        if dot(data[index].dir, c.collision_normal) > 0.0 {
            n1 = ior_glass; // We are in the glass
            n2 = 1.0;       // We are exiting to air
        }

        let rl_rr = do_reflect_refract(data[index].dir, c.collision_normal, 1.0, ior_glass);

        let random = get_random(settings.timestamp, index);
        if random > rl_rr.reflection_chance {
            data[index].dir = rl_rr.rac;
            data[index].strength = data[index].strength * (1.0 - rl_rr.reflection_chance);
        } else {
            data[index].dir = rl_rr.rle;
            data[index].strength = data[index].strength * rl_rr.reflection_chance;
        }
        data[index].pos = c.collision_point + data[index].dir * 0.001;
    } else {
        data[index].last_pos = data[index].pos;
        data[index].pos = vec2<f32>(data[index].pos + data[index].dir * 100.0);
        data[index].strength *= -1.0;
    }


}

