const INF: f32 = 3.402823466e+38f;

const vertices_array = array(
    vec2<f32>( 0.0,    0.5),   // Top
    vec2<f32>( 0.0,   -0.5),   // Bottom
    vec2<f32>( 0.3,    0.25),  // Top Right
    vec2<f32>( 0.3,   -0.25),  // Bottom Right
    vec2<f32>(-0.3,    0.25),  // Top Left
    vec2<f32>(-0.3,   -0.25),  // Bottom Left
);

/*

    array of lines in world space from start index to !! + length

*/
struct Collision {
    collision_point: vec2<f32>,
    collision_normal: vec2<f32>,
    walls_collided: u32,
}
fn collide(start: u32, length: u32, ro: vec2<f32>, rd: vec2<f32>) -> Collision {
    var hit_dist: f32 = INF;
    var num_inter: u32 = 0;
    var normal = vec2(0.0);

    for (var i: u32 = 0; i < length; i += 1) {
        let a_raw = vertices_array[start + i];
        let b_raw = vertices_array[start + ((i + 1) % length)];
        // Scale x by aspect so vertices live in the same world space as rays
        let a = vec2(a_raw.x * settings.aspect, a_raw.y);
        let b = vec2(b_raw.x * settings.aspect, b_raw.y);

        let v = b - a;
        let w = ro - a;
        let det = (v.x * rd.y) - (v.y * rd.x);

        if abs(det) < 1e-6 { continue; }

        let t = ((w.x * rd.y) - (w.y * rd.x)) / det;

        if t >= 0.0 && t <= 1.0 {
            let u = ((w.x * v.y) - (w.y * v.x)) / det;
            if u > 0.0 {
                num_inter += 1;
                if u < hit_dist {
                    hit_dist = u;
                    normal = normalize(vec2(v.y, -v.x));
                }
            }
        }
    }

    if hit_dist < INF {
        let hit = ro + rd * hit_dist;
        return Collision(hit, normal, num_inter);
    };

    return Collision(vec2(0.0),vec2(0.0), 0);
}



struct ReRfCalc {
    rle: vec2<f32>,
    rac: vec2<f32>,

    reflection_chance: f32,
}

fn do_reflect_refract(
    dir: vec2<f32>,
    o_normal: vec2<f32>,
    n1: f32,
    n2: f32,
) -> ReRfCalc {
    var normal = o_normal;
    if dot(normal, dir) > 0.0 {
        normal = -normal;
    }

    let ratio: f32 = n1 / n2;
    let c1 = -dot(normal, dir);

    // FIX 3: Replaced pow(x, 2) with x * x. It is faster and avoids WGSL type errors.
    let radicand = 1.0 - (ratio * ratio) * (1.0 - (c1 * c1));

    if radicand >= 0.0 {
        let c2 = sqrt(radicand);

        let rac = (ratio * dir) + (ratio * c1 - c2) * normal;
        let rle = dir + 2.0 * c1 * normal;

        let rs = (n1 * c1 - n2 * c2) / (n1 * c1 + n2 * c2);
        let rs_reflectance = rs * rs;

        let rp = (n2 * c1 - n1 * c2) / (n2 * c1 + n1 * c2);
        let rp_reflectance = rp * rp;

        let rle_fact = (rs_reflectance + rp_reflectance) / 2.0;

        // Ensure we return the normalized vector just to be safe from floating point drift
        return ReRfCalc(normalize(rle), normalize(rac), rle_fact);
    } else {
        // Total Internal Reflection
        let reflect = dir + 2.0 * c1 * normal;
        return ReRfCalc(normalize(reflect), vec2(0.0), 1.0);
    }
}



fn get_ior(wavelength_nm: f32, a: f32, b: f32) -> f32 {
    let wavelength_microns = wavelength_nm / 1000.0;
    return a + (b / pow(wavelength_microns, 2.0));
}