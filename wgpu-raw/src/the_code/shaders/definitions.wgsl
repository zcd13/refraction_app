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

