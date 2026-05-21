const INF: f32 = bitcast<f32>(0x7f800000u);
const NEG_INF: f32 = bitcast<f32>(0xff800000u);

/*

    array of lines in world space from start index to !! + length

*/
struct Collision {
    collision_point: vec2<f32>,
    collision_normal: vec2<f32>,
    walls_collided: u32,
}
fn collide(p: ptr<storage, array<f32>, read>, start: u32, length: u32, ro: vec2<f32>, rd: vec2<f32>) -> Collision {
    var hit_dist: f32 = INF;
    var num_inter: u32 = 0;
    var normal = vec2(0.0);

    for (var i = 0; i < length; i++) {
        let a = p[start + i];
        let b = p[start + ((i + 1) % length)];

        let v = b - a;
        let w = ro - a;
        let det = (v.x * rd.y) - (v.y * rd.x);

        if abs(det) < 1e-6 { continue; }

        let t = ((w.x * rd.y) - (w.y * rd.x)) / det;

        if t > 0.0 && t < 1.0 {
            let u = ((w.x * v.y) - (w.y * v.x)) / det;
            if u > 0.0 {
                num_inter += 1;
                if u < hit_dist {
                    normal = vec2(v.y, -v.y);
                }
            }
        }
    }

    if hit_dist < INV {
        let hit = ro + rd * hit_dist;
        return Collision(hit, normal, num_inter);
    };

    return Collision(vec2(0.0),vec2(0.0), 0);
}


