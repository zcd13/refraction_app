#![allow(dead_code)]

use std::f32::consts::PI;
use eframe::egui::Vec2;
use crate::wgpu_res::Vertex;

pub struct Polygon {
    // in local space
    vertices: Vec<Vec2>,

    position: Vec2,
    rotation: f32,
    scale: f32,

    world_space: Option<Vec<Vertex>>,
}

impl Default for Polygon {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            rotation: 0.0,
            scale: 1.0,
            vertices: vec![],
            world_space: None,
        }
    }
}

impl Polygon {
    pub fn tri() -> Self {
        let h = 0.5;
        let f = 0.866;
        Self {
            vertices: vec![
                Vec2::new(-f, -h),
                Vec2::new(f, -h),
                Vec2::new(0.0, 1.),
            ],
            ..Default::default()
        }
    }

    pub fn right_angle_tri() -> Self {
        Self {
            vertices: vec![
                Vec2::new(-1., -1.),
                Vec2::new( 1., -1.),
                Vec2::new(-1.,  1.),
            ],
            ..Default::default()
        }
    }

    pub fn circle_poly(res: usize) -> Self {
        let mut vertices = Vec::with_capacity(res);
        for i in 0..res {
            let per = i as f32 / res as f32;
            let rad = PI * 2.0 * per;
            vertices.push(Vec2::new(rad.cos(), rad.sin()))
        }

        Self {
            vertices,
            ..Default::default()
        }
    }
}
impl Polygon {
    pub fn pos(&self) -> Vec2 {
        self.position
    }

    pub fn rot(&self) -> f32 {
        self.rotation
    }

    pub fn scale(&self) -> f32 { self.scale }

    pub fn set_pos(&mut self, pos: Vec2) {
        self.world_space = None;
        self.position = pos;
    }

    pub fn set_rot(&mut self, rot: f32) {
        self.world_space = None;
        self.rotation = rot;
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.world_space = None;
        self.scale = scale;
    }

    pub fn cast_worldspace(&mut self) -> &Vec<Vertex> {
        if self.world_space.is_none() {
            let mut out = Vec::with_capacity(self.vertices.len());

            let sin_r = self.rotation.sin();
            let cos_r = self.rotation.cos();
            for v in self.vertices.iter() {
                let mut rx = v.x * cos_r - v.y * sin_r;
                let mut ry = v.x * sin_r + v.y * cos_r;
                rx *= self.scale;
                ry *= self.scale;
                rx += self.position.x;
                ry += self.position.y;
                out.push(Vertex([rx, ry]))
            }

            self.world_space = Some(out);
        }

        self.world_space.as_ref().unwrap()
    }
}
pub struct Geometry {
    pub polygons: Vec<Polygon>,
}
impl Geometry {
    pub fn new() -> Self {
        Self {
            polygons: Vec::new(),
        }
    }


}
