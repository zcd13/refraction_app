use std::f32::consts::PI;
use macroquad::color::Color;
use macroquad::math::Vec2;
use macroquad::prelude::draw_triangle;
use macroquad::shapes::{draw_circle, draw_circle_lines};
use crate::{line, to_screen};


#[derive(Clone, Debug)]
pub enum GeometryType {
    Polygon(Vec<Vec2>),
    Sphere,
}

#[derive(Clone, Debug)]
pub struct Geometry {
    ty: GeometryType,
    pos: Vec2,
    rot: f32,
    scale: f32,
}
impl Geometry {
    pub fn create(ty: GeometryType) -> Self {
        Self {
            ty,
            pos: Vec2::ZERO,
            rot: 0.0,
            scale: 1.0,
        }
    }
    pub fn tri() -> Self {
        let h = 0.5;
        let f = 0.866;
        Self::create(GeometryType::Polygon(vec![
            Vec2::new(-f, -h),
            Vec2::new(f, -h),
            Vec2::new(0.0, 1.),
        ]))
    }

    pub fn right_tri() -> Self {
        Self::create(GeometryType::Polygon(vec![
            Vec2::new(-0.5, -0.5), // Bottom-Left (The Right Angle)
            Vec2::new( 0.5, -0.5), // Bottom-Right
            Vec2::new(-0.5,  0.5), // Top-Left
        ]))
    }
    pub fn circle_poly(res: usize) -> Self {
        let mut points = Vec::with_capacity(res);
        for i in 0..res {
            let per = i as f32 / res as f32;
            let rad = PI * 2.0 * per;
            points.push(Vec2::new(rad.cos(), rad.sin()))
        }

        Self::create(GeometryType::Polygon(points))
    }
    pub fn line_dyn(a: Vec2, b: Vec2) -> Self {
        Self::create(GeometryType::Polygon(vec![a, b]))
    }

    pub fn circle() -> Self { Self::create(GeometryType::Sphere) }
    pub fn with_pos(mut self, pos: Vec2) -> Self {
        self.pos = pos;
        self
    }
    pub fn with_rot(mut self, dir: f32) -> Self {
        self.rot = dir;
        self
    }
    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }
}

impl Geometry {
    pub fn local_to_world(&self, point: Vec2) -> Vec2 {
        let s = point * self.scale;
        let cos = self.rot.cos();
        let sin = self.rot.sin();

        Vec2::new(
            s.x * cos - s.y * sin + self.pos.x,
            s.x * sin + s.y * cos + self.pos.y
        )
    }

    pub fn intersect(&self, ro: Vec2, rd: Vec2) -> Option<(Vec2, u32, Vec2)> {
        match &self.ty {
            GeometryType::Polygon(vertices) => {
                let cos = (-self.rot).cos();
                let sin = (-self.rot).sin();

                let relative_ro = ro - self.pos;
                let mut lro = Vec2::new(
                    relative_ro.x * cos - relative_ro.y * sin,
                    relative_ro.x * sin + relative_ro.y * cos
                );

                let mut lrd = Vec2::new(
                    rd.x * cos - rd.y * sin,
                    rd.x * sin + rd.y * cos
                );

                lro /= self.scale;
                lrd /= self.scale;

                let (ro, rd) = (lro, lrd);

                let mut hit_distance = f32::INFINITY;
                let mut num_intersects = 0;
                let mut normal = Vec2::ZERO;

                for i in 0..vertices.len() {
                    let a = vertices[i];
                    let b = vertices[(i + 1) % vertices.len()];

                    let v = b - a;
                    let w = ro - a;
                    let det = (v.x * rd.y) - (v.y * rd.x);

                    // If det is 0, the ray and segment are parallel; avoiding div by 0 is good practice.
                    if det.abs() < 1e-6 { continue; }

                    let t = ((w.x * rd.y) - (w.y * rd.x)) / det;

                    // hits line segment
                    if t > 0.0 && t < 1.0 {
                        let u = ((w.x * v.y) - (w.y * v.x)) / det;
                        if u > 0.0 {
                            num_intersects += 1;
                            if u < hit_distance {
                                normal = Vec2::new(v.y, -v.x);
                                hit_distance = u;
                            }

                        }
                    }
                }

                if hit_distance < f32::INFINITY {
                    let local_hit = ro + rd * hit_distance;
                    let world_hit = self.local_to_world(local_hit);

                    let local_normal = normal.normalize();
                    let cos = self.rot.cos();
                    let sin = self.rot.sin();
                    normal = Vec2::new(
                        local_normal.x * cos - local_normal.y * sin,
                        local_normal.x * sin + local_normal.y * cos
                    );

                    Some((world_hit, num_intersects, normal))
                } else {
                    None
                }
            }
            GeometryType::Sphere => {
                let oc = ro - self.pos;
                let r2 = self.scale * self.scale;

                // Check if ray origin is inside the sphere
                let is_inside = oc.length_squared() < r2;

                let a = rd.dot(rd);
                let b = 2.0 * oc.dot(rd);
                let c = oc.dot(oc) - r2;

                let discriminant = b * b - 4.0 * a * c;

                if discriminant < 0.0 {
                    None
                } else {
                    let sqrt_d = discriminant.sqrt();
                    let t0 = (-b - sqrt_d) / (2.0 * a);
                    let t1 = (-b + sqrt_d) / (2.0 * a);

                    // Standard hit detection: find the smallest positive t
                    let t = if t0 > 0.0 {
                        t0
                    } else if t1 > 0.0 {
                        t1
                    } else {
                        return None;
                    };

                    let world_hit = ro + rd * t;

                    // If we hit from the inside, the normal should point inward
                    // to stay consistent with most physics/rendering expectations,
                    // or stay outward for geometric consistency.
                    let mut normal = (world_hit - self.pos).normalize();
                    if is_inside {
                        normal = -normal;
                    }

                    // Return 1 intersection if we started inside, 2 if we started outside
                    let num_intersects = if is_inside { 1 } else { 2 };

                    Some((world_hit, num_intersects, normal))
                }
            }
        }


    }

    pub fn is_inside(&self, world_point: Vec2) -> bool {
        match &self.ty {
            GeometryType::Polygon(vertices) => {
                // 1. Transform world point to local space
                let relative_p = world_point - self.pos;
                let cos = (-self.rot).cos();
                let sin = (-self.rot).sin();

                let mut local_p = Vec2::new(
                    relative_p.x * cos - relative_p.y * sin,
                    relative_p.x * sin + relative_p.y * cos
                );
                local_p /= self.scale;

                // 2. Ray Casting Algorithm (Even-Odd Rule)
                // We cast a ray from local_p to the right (positive X direction)
                let mut inside = false;
                let count = vertices.len();

                for i in 0..count {
                    let a = vertices[i];
                    let b = vertices[(i + 1) % count];

                    // Check if the point's Y coordinate is between the edge's Y coordinates
                    // and if the point is to the left of the edge
                    if ((a.y > local_p.y) != (b.y > local_p.y)) &&
                        (local_p.x < (b.x - a.x) * (local_p.y - a.y) / (b.y - a.y) + a.x)
                    {
                        inside = !inside;
                    }
                }

                inside
            }
            GeometryType::Sphere => {
                let delta = world_point - self.pos;
                delta.length_squared() < (self.scale * self.scale)
            }
        }
    }
}

impl Geometry {
    pub fn draw_outline(&self, thick: f32, color: Color) {
        match &self.ty {
            GeometryType::Polygon(vertices) => {
                for i in 0..vertices.len() {
                    let a = self.local_to_world(vertices[i]);
                    let b = self.local_to_world(vertices[(i + 1) % vertices.len()]);
                    line(a, b, thick, color);
                }
            }
            GeometryType::Sphere => {
                let p = to_screen(self.pos);
                let w = macroquad::window::screen_width();
                let h = macroquad::window::screen_height();
                let min_dim = w.min(h);
                let radius = self.scale * (min_dim / 2.0);
                draw_circle_lines(p.x, p.y, radius, thick, color);
            }
        }

    }
    pub fn fan_shaded(&self, color: Color) {
        match &self.ty {
            GeometryType::Polygon(vertices) => {
                let l = vertices.len();
                if l >= 3 {
                    for i in 0..l {
                        let a = i;
                        let b = (i + 1) % l;
                        let c = (i + 2) % l;


                        draw_triangle(
                            to_screen(self.local_to_world(vertices[a])),
                            to_screen(self.local_to_world(vertices[b])),
                            to_screen(self.local_to_world(vertices[c])),
                            color
                        );
                    }
                }
            }
            GeometryType::Sphere => panic!("Only for Polygon")
        }

    }
}