struct Vertex {
    pos: vec2<f32>,
}

// Access your "vertex" array as a storage buffer
@group(0) @binding(0) var<storage, read> vertex_data: array<Vertex>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>, // Add this to track where we are
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0)
    );
    out.clip_position = vec4<f32>(pos[in_vertex_index], 0.0, 1.0);
    out.uv = pos[in_vertex_index]; // Pass coordinate to fragment shader
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // FIX: Look at the whole [-1, 1] screen, not just the [0, 1] quadrant
    let ro = in.uv;
    let rd = vec2<f32>(1.0, 0.0);

    // FIX: Use 0u (unsigned) to match the parameter type
    let inter = collide(0u, arrayLength(&vertex_data), ro, rd);
    let cn = inter.collision_normal * 0.5 + 0.5;

    if inter.walls_collided == 0 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    return vec4<f32>(cn.x, cn.y, 0.0, 1.0);

//    // If walls_collided is odd, we are inside the polygon
//    if inter.walls_collided % 2u != 0u {
//        // Map the normal from [-1.0, 1.0] to [0.0, 1.0] to visualize it as a color
//        let color_r = inter.collision_normal.x * 0.5 + 0.5;
//        let color_g = inter.collision_normal.y * 0.5 + 0.5;
//        return vec4<f32>(color_r, color_g, 0.5, 1.0);
//    }
//
//    // Outside = Dark Blue
//    return vec4<f32>(0.05, 0.05, 0.1, 1.0);
}



const INF: f32 = 0x1.fp+126;
const NEG_INF: f32 = -0x1.fp+126;

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

    for (var i: u32 = 0u; i < length; i += 1u) {
        let a = vertex_data[start + i].pos;
        let b = vertex_data[start + ((i + 1) % length)].pos;

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
                    normal = normalize(vec2<f32>(-v.y, v.x));
                    hit_dist = u;
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