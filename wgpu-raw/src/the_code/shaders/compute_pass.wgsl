@group(0) @binding(0) var<storage, read_write> data: array<LightRay>;
@group(1) @binding(0) var<uniform> settings: Settings;

const MIN_STRENGTH: f32 = 0.002;
const MAX_STRENGTH: f32 = 10000.0;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    if index >= arrayLength(&data) {
        return;
    }

    if data[index].strength == -1.0 || data[index].strength == 0.0 {
        data[index].strength = 0.0;
        return;
    }

    // collide
    let c = collide(data[index].pos, data[index].dir);
    if c.walls_collided != 0 {
        data[index].last_pos = data[index].pos;

        let float_wave = packed_to_wavelength(data[index].wave_length_and_ior);
        let ior_glass = get_ior(float_wave, 1.517 * 0.5, 0.0042 * 3.0);

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
            data[index].strength = new_strength(data[index].strength, (1.0 - rl_rr.reflection_chance));
        } else {
            data[index].dir = rl_rr.rle;
            data[index].strength = new_strength(data[index].strength, rl_rr.reflection_chance);
        }
        data[index].pos = c.collision_point + data[index].dir * 0.001;
    } else {
        data[index].last_pos = data[index].pos;
        data[index].pos = vec2<f32>(data[index].pos + data[index].dir * 100.0);
        data[index].strength *= -1.0;
    }


}


const WIDTH: f32 = 0.0005;
const SPREAD: f32 = 0.000;

@compute @workgroup_size(256)
fn init(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    if index >= arrayLength(&data) {
        return;
    }

    let random = get_random(settings.timestamp, index);
    let x = (random - 0.5) * 2.0;

    let zero_pos = vec2<f32>(-1.0, 0.0);
    let mp = settings.mouse_pos;

    let dir_to_mouse = normalize(mp - zero_pos);

    let left = vec2<f32>(-dir_to_mouse.y, dir_to_mouse.x);
    let right = vec2<f32>(dir_to_mouse.y, -dir_to_mouse.x);
    let diff = left - right;
    let aj = (diff * WIDTH) * x;

    var dir_rad = atan2(dir_to_mouse.y, dir_to_mouse.x);
    dir_rad += SPREAD * x;
    let dir = vec2<f32>(cos(dir_rad), sin(dir_rad));

//    let wave = ((780.0 - 380.0) * random) + 380;
    let wave_length = float_to_packed(get_random(settings.timestamp, index + 3));

    data[index].pos = zero_pos + aj;
    data[index].last_pos = zero_pos;
    data[index].dir = dir;
    data[index].strength = 0.05;
    data[index].wave_length_and_ior = wave_length;
}
