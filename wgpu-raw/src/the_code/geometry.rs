#![allow(dead_code)]

use std::f32::consts::PI;
use bytemuck::{cast, cast_slice, cast_vec};
use glam::{vec2, Vec2};
use wgpu::{BindGroupLayoutDescriptor, Buffer, BufferUsages, Device, Queue};
use wgpu::wgt::BufferDescriptor;

pub struct Polygon {
    // in local space
    vertices: Vec<Vec2>,

    position: Vec2,
    rotation: f32,
    scale: f32,

    world_space: Option<Vec<Vec2>>,

    a_coff: f32,
    b_coff: f32,


}
impl Default for Polygon {
    fn default() -> Self {
        Self {
            vertices: vec![],
            position: Vec2::ZERO,
            rotation: 0.0,
            scale: 1.0,
            world_space: None,
            a_coff: 1.4580,
            b_coff: 0.00354,
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

    pub fn coff(&self) -> (f32, f32) { (self.a_coff, self.b_coff) }

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

    pub fn set_coff(&mut self, ab: (f32, f32)) {
        (self.a_coff, self.b_coff) = ab;
    }

    pub fn cast_worldspace(&mut self) -> &Vec<Vec2> {
        if self.world_space.is_none() {
            let mut out = Vec::with_capacity(self.vertices.len());

            let sin_r = self.rotation.sin();
            let cos_r = self.rotation.cos();
            for v in self.vertices.iter() {
                let rx = v.x * cos_r - v.y * sin_r;
                let ry = v.x * sin_r + v.y * cos_r;
                let mut rxy = vec2(rx, ry);
                rxy *= self.scale;
                rxy += self.position;
                out.push(rxy)
            }

            self.world_space = Some(out);
        }

        self.world_space.as_ref().unwrap()
    }
}


pub struct GraphicsPolygon {
    polys: Vec<Polygon>,

    // A coff, B coff, start index, end index
    info_buffer: Buffer,
    position_buffer: Buffer,

    window: u32,
    offset_b: u32,
}
impl GraphicsPolygon {
    pub fn init(device: &Device) {
        let window = device.limits().min_storage_buffer_offset_alignment;

        let info_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("info_buffer"),
            size: 0,
            usage: BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let position_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("info_buffer"),
            size: 0,
            usage: BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Dynamic Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: info_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: position_buffer.as_entire_binding(),
                },
            ],
            label: None,
        });

        let this_self = Self {
            polys: vec![],
            info_buffer,
            position_buffer,
            window,
            offset_b: 0,
        };
    }

    pub fn edit<F: FnMut(&mut Vec<Polygon>)>(&mut self, mut edit: F, queue: &Queue) {

        todo!();

        edit(&mut self.polys);

        let mut vertex_buffer = Vec::new();
        let mut info_buffer = Vec::new();
        for poly in self.polys.iter_mut() {
            let info = PolyInfo {
                a_coff: poly.a_coff,
                b_coff: poly.b_coff,
                start_index: vertex_buffer.len() as u32,
                length: poly.vertices.len() as u32,
            };
            info_buffer.push(info);
            let mut pos = poly.cast_worldspace().clone();
            vertex_buffer.append(&mut pos);
        }


        let mut ver_bytes: Vec<u8> = vec![];
        vertex_buffer.into_iter().for_each(|fe| {
            let t = [fe.x, fe.y];
            let b: &[u8] = cast_slice(&t);
            ver_bytes.extend_from_slice(b);
        });
        let mut info_bytes: Vec<u8> = cast_vec(info_buffer);

        let info_off = align_to(self.window, info_bytes.len() as u32);
        let diff = info_off - info_bytes.len() as u32;
        for _ in 0..diff { info_bytes.push(0u8); }

        let info_off = align_to(self.window, info_bytes.len() as u32);
        let diff = info_off - info_bytes.len() as u32;
        for _ in 0..diff { info_bytes.push(0u8); }


    }
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
    length: u32
}