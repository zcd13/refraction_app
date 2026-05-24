struct LightRay {
    pos: vec2<f32>,
    last_pos: vec2<f32>,
    dir: vec2<f32>,
    strength: f32,
    wave_length_and_ior: u32,
};

struct Settings {
   timestamp: f32,
   width: u32,
   height: u32,
   aspect: f32,        // was commented padding
   mouse_pos: vec2<f32>,
   do_ray_jobs: u32,
   _padding: u32,
};


fn hash_u32(x: u32) -> u32 {
    var s = x;
    s ^= s >> 16u;
    s *= 0x85ebca6bu;
    s ^= s >> 13u;
    s *= 0xc2b2ae35u;
    s ^= s >> 16u;
    return s;
}

fn get_random(timestamp: f32, id: u32) -> f32 {
    // 1. Bit-cast the float to a u32
    let ts_bits = bitcast<u32>(timestamp);

    // 2. Combine inputs (using a simple hash-combining XOR shift)
    let combined_seed = ts_bits ^ hash_u32(id);

    // 3. Hash the seed
    let hashed_val = hash_u32(combined_seed);

    // 4. Normalize to [0, 1]
    return f32(hashed_val) / 4294967295.0;
}


const FIRST_HALF: u32 = 0xFFFFu;
const LOWER_CONS: f32 = 355.0;
const UPPER_CONS: f32 = 775.0;

fn packed_to_float(pack: u32) -> f32 {
    let half = pack & FIRST_HALF;
    let float = f32(half) / 65535.0;
    return float;
};

fn packed_to_wavelength(pack: u32) -> f32 {
    let float = packed_to_float(pack);
    return ((UPPER_CONS - LOWER_CONS) * float) + LOWER_CONS;
}

fn float_to_packed(float: f32) -> u32 {
    return u32(65535.0 * float) & FIRST_HALF;
}



const SOFTNESS: f32 = 0.1; // Higher = weaker rays lose almost no energy
fn new_strength(strength: f32, factor: f32) -> f32 {
    let adjustment = pow(strength, SOFTNESS);
    return strength * pow(factor, adjustment);
}