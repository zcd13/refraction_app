@group(0) @binding(0) var<storage, read_write> data: array<LightRay>;
@group(1) @binding(0) var<uniform> settings: Settings;
@group(2) @binding(0) var<storage, read> geometry: array<vec2<f32>>;

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
        data[index].strength *= settings.absobtion;
        data[index].last_pos = data[index].pos;

        let float_wave = packed_to_wavelength(data[index].wave_length_and_ior);
        let ior_glass = get_ior(float_wave, settings.a_factor, settings.b_factor);

        var n1: f32 = 1.0;       // Default Air
        var n2: f32 = ior_glass; // Default Glass

        var op = -1.0; // move in normal direction
        if dot(data[index].dir, c.collision_normal) > 0.0 {
            n1 = ior_glass; // We are in the glass
            n2 = 1.0;       // We are exiting to air
            op *= -1.0;
        }

        let rl_rr = do_reflect_refract(data[index].dir, c.collision_normal, n1, n2);

        let random = get_random(settings.timestamp, index);
        if random > rl_rr.reflection_chance {
            data[index].dir = rl_rr.rac;
            data[index].strength = new_strength(data[index].strength, (1.0 - rl_rr.reflection_chance));
            data[index].pos = c.collision_point + (c.collision_normal * op) * settings.nudge_factor;
        } else {
            data[index].dir = rl_rr.rle;
            data[index].strength = new_strength(data[index].strength, rl_rr.reflection_chance);
            data[index].pos = c.collision_point + (c.collision_normal * (op * -1.0)) * settings.nudge_factor;
        }
//        data[index].pos = c.collision_point + data[index].dir * 0.00001;
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

    let x = (get_random(settings.timestamp, index) - 0.5) * 2.0;

    let zero_pos = settings.light_pos;

    var light_dir: vec2<f32>;
    if (settings.follow_mouse == 1) {
        let mp = settings.mouse_pos_clip;
        light_dir = normalize(mp - zero_pos);
    } else {
        light_dir = vec2<f32>(cos(settings.light_dir), sin(settings.light_dir));
    };

    let left = vec2<f32>(-light_dir.y, light_dir.x);
    let right = vec2<f32>(light_dir.y, -light_dir.x);
    let diff = left - right;
    let aj = (diff * settings.width) * x;

    var dir_rad = atan2(light_dir.y, light_dir.x);
    dir_rad += settings.spread * x;
    let dir = vec2<f32>(cos(dir_rad), sin(dir_rad));

    let wave = ((780.0 - 380.0) * get_random(settings.timestamp, index + 1)) + 380;
    let wave_length = float_to_packed(get_random(settings.timestamp, index + 2));

    data[index].pos = zero_pos + aj;
    data[index].last_pos = zero_pos;
    data[index].dir = dir;
    data[index].strength = settings.total_light / f32(settings.ray_count);
    data[index].wave_length_and_ior = wave_length;
}
