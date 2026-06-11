#![allow(dead_code)]

use crate::the_code::utils::SimpleBuffer;
use glam::{vec2, Vec2};
use std::f32::consts::PI;
use wgpu::{Device, Queue, };
use crate::enum_iter;



enum_iter!(
    #[derive(Debug, PartialOrd, PartialEq, Copy, Clone)]
    pub enum Shape {
        Triangle,
        Circle(u32 => 300),
        RightTriangle,
        Diamond,
        DiamondFlat,
        Star,
        FancyDiamond,
        Hexagon,
        Trapezoid,
        DiamondFancy,
    }
);

impl Shape {
    pub fn cast(&self) -> Vec<Vec2> {
        match &self {
            Shape::Triangle => {
                let h = 0.5;
                let f = 0.866;
                vec![Vec2::new(-f, -h), Vec2::new(f, -h), Vec2::new(0.0, 1.)]
            }
            Shape::Circle(vert) => {
                let vert = *vert as usize;
                let mut vertices = Vec::with_capacity(vert);
                for i in 0..vert {
                    let per = i as f32 / vert as f32;
                    let rad = PI * 2.0 * per;
                    vertices.push(Vec2::new(rad.cos(), rad.sin()))
                }
                vertices
            }
            Shape::RightTriangle => {
                vec![Vec2::new(-1., -1.), Vec2::new(1., -1.), Vec2::new(-1., 1.)]
            }
            Shape::Diamond => {
                vec![
                    Vec2::new(0.0, 1.0),  // Top
                    Vec2::new(0.7, 0.0),  // Right
                    Vec2::new(0.0, -1.0), // Bottom
                    Vec2::new(-0.7, 0.0), // Left
                ]
            }
            Shape::Star => {
                // A clean 5-pointed star outline using alternating inner/outer points
                let mut vertices = Vec::with_capacity(10);
                for i in 0..10 {
                    let per = i as f32 / 10.0;
                    // Offset by PI/2 to make the star point straight up
                    let rad = (PI * 2.0 * per) + (PI / 2.0);
                    let r = if i % 2 == 0 { 1.0 } else { 0.382 }; // Outer vs inner radius
                    vertices.push(Vec2::new(rad.cos() * r, rad.sin() * r));
                }
                vertices
            }
            Shape::FancyDiamond => {
                // A classic 2D stylized crystal or gem silhouette (elongated diamond with wider shoulders)
                vec![
                    Vec2::new(0.0, 1.0),   // Top sharp point
                    Vec2::new(0.5, 0.3),   // Upper right shoulder
                    Vec2::new(0.0, -1.0),  // Bottom long point
                    Vec2::new(-0.5, 0.3),  // Upper left shoulder
                ]
            }
            Shape::Hexagon => {
                // Flat-topped regular hexagon
                let mut vertices = Vec::with_capacity(6);
                for i in 0..6 {
                    let per = i as f32 / 6.0;
                    let rad = (PI * 2.0 * per) + (PI / 6.0); // Offset to keep it flat on top/bottom
                    vertices.push(Vec2::new(rad.cos(), rad.sin()));
                }
                vertices
            }
            Shape::Trapezoid => {
                vec![
                    Vec2::new(-0.5, 0.7),  // Top Left
                    Vec2::new(0.5, 0.7),   // Top Right
                    Vec2::new(1.0, -0.7),  // Bottom Right
                    Vec2::new(-1.0, -0.7), // Bottom Left
                ]
            }
            Shape::DiamondFlat => {
                let short = 0.2;
                let long = 0.8;

                vec![
                    Vec2::new(-short,  long),
                    Vec2::new( short,  long),
                    Vec2::new( long,   short),
                    Vec2::new( long,  -short),
                    Vec2::new( short, -long),
                    Vec2::new(-short, -long),
                    Vec2::new(-long,  -short),
                    Vec2::new(-long,   short),
                ]
            }

            Shape::DiamondFancy => {
                let table_w = 0.45;
                let crown_h = 0.25;
                let girdle_w = 0.90;
                let girdle_h = 0.05;
                let pavilion_h = 0.70;

                vec![
                    Vec2::new(-table_w, crown_h + girdle_h),
                    Vec2::new(table_w, crown_h + girdle_h),
                    Vec2::new(girdle_w, girdle_h),
                    Vec2::new(girdle_w, 0.0),
                    Vec2::new(0.0, -pavilion_h),
                    Vec2::new(-girdle_w, 0.0),
                    Vec2::new(-girdle_w, girdle_h),
                ]
            }
        }
    }
}

pub struct Polygon {
    // in local space
    vertices: Vec<Vec2>,
    params: Params,
    world_space: Option<Vec<Vec2>>,
    pub buffer: SimpleBuffer<Vec2>,
    pub current_shape: Shape,
}
impl Polygon {
    pub fn init(device: &Device, shape: Shape) -> Self {
        let vertices = shape.cast();
        let params = Params {
            pos: Vec2::ZERO,
            scale: 0.3,
            rot: 0.140,
        };
        let mut world_space = None;
        let world = Self::cast_worldspace(&mut world_space, &vertices, &params);
        let buffer = SimpleBuffer::init_with(device, world);

        Self {
            vertices,
            params,
            world_space,
            buffer,
            current_shape: shape,
        }
    }

    pub fn cast_worldspace<'a>(
        world_space: &'a mut Option<Vec<Vec2>>,
        vertices: &[Vec2],
        params: &Params,
    ) -> &'a [Vec2] {
        if world_space.is_none() {
            let mut out = Vec::with_capacity(vertices.len());

            let sin_r = params.rot.sin();
            let cos_r = params.rot.cos();
            for v in vertices.iter() {
                let rx = v.x * cos_r - v.y * sin_r;
                let ry = v.x * sin_r + v.y * cos_r;
                let mut rxy = vec2(rx, ry);
                rxy *= params.scale;
                rxy += params.pos;
                out.push(rxy)
            }

            *world_space = Some(out);
        }

        unsafe { world_space.as_ref().unwrap_unchecked() }
    }

    pub fn set_shape(&mut self, device: &Device, shape: Shape) {
        self.world_space = None;
        self.vertices = shape.cast();
        let world = Self::cast_worldspace(&mut self.world_space, &self.vertices, &self.params);
        self.buffer = SimpleBuffer::init_with(device, world);
        self.current_shape = shape;
    }

    pub fn edit(&mut self, queue: &Queue, mut f: impl FnMut(&mut Params)) {
        let mut params_c = self.params;
        f(&mut params_c);
        if params_c != self.params {
            self.world_space = None;
            self.params = params_c;
            let verts = Self::cast_worldspace(&mut self.world_space, &self.vertices, &self.params);
            self.buffer.write_with(queue, verts);
        }
    }
}

#[derive(PartialEq, Copy, Clone)]
pub struct Params {
    pub pos: Vec2,
    pub scale: f32,
    pub rot: f32,
}

fn align_to(size: u32, alignment: u32) -> u32 {
    (size + alignment - 1) & !(alignment - 1)
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PolyInfo {
    a_coff: f32,
    b_coff: f32,
    start_index: u32,
    length: u32,
}
