#![allow(dead_code)]

use std::f32::consts::PI;
use bytemuck::{bytes_of, cast, cast_slice, cast_vec};
use glam::{vec2, Vec2};
use wgpu::{BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, Buffer, BufferUsages, Device, Queue};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::wgt::BufferDescriptor;

pub enum Shape {
    Triangle,
    Circle(u32),
    RightTriangle,
}
impl Shape {
    pub fn cast(&self) -> Vec<Vec2> {
        match &self {
            Shape::Triangle => {
                let h = 0.5;
                let f = 0.866;
                vec![
                    Vec2::new(-f, -h),
                    Vec2::new(f, -h),
                    Vec2::new(0.0, 1.),
                ]
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
                vec![
                    Vec2::new(-1., -1.),
                    Vec2::new( 1., -1.),
                    Vec2::new(-1.,  1.),
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


    vertex_buffer: Buffer,
    bind_group: BindGroup,
    layout: BindGroupLayout,
}
impl Polygon {
    pub fn init(device: &Device, queue: &Queue) -> Self {

        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Vert layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        // Optional: Minimum size of the buffer (Header + at least 1 element)
                        min_binding_size: wgpu::BufferSize::new(8 + 8),
                    },
                    count: None,
                },
            ],
        });

        let vertices: Vec<Vec2> = Shape::Triangle.cast();
        let (vertex_buffer, bind_group) = Self::create_vertex_buffer(device, queue, &layout, &vertices);

        Self {
            vertices: vertices.clone(),
            params: Params {
                pos: Vec2::ZERO,
                scale: 1.0,
                rot: 0.0,
            },
            world_space: None,
            vertex_buffer,
            bind_group,
            layout,
        }
    }

    pub fn create_vertex_buffer(device: &Device, queue: &Queue, layout: &BindGroupLayout, vertices: &[Vec2]) -> (Buffer, BindGroup) {
        let length = vertices.len() as u32;
        let vertices_bytes = cast_slice(&vertices);
        let length_bytes = bytes_of(&length);
        let padding_bytes = &[0u8; 4];

        let buffer = device.create_buffer(&BufferDescriptor {
            label: None,
            size: (vertices_bytes.len() + length_bytes.len() + padding_bytes.len()) as u64,
            usage: BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        queue.write_buffer(&buffer, 0, length_bytes);
        queue.write_buffer(&buffer, 4, padding_bytes);
        queue.write_buffer(&buffer, 8, vertices_bytes);

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        (buffer, bind_group)
    }

    /// assumes size is correct
    pub fn update_buffer(queue: &Queue, buffer: &Buffer, vertices: &[Vec2]) {
        queue.write_buffer(&buffer, 8, cast_slice(vertices));
    }

    pub fn cast_worldspace<'a>(world_space: &'a mut Option<Vec<Vec2>>, vertices: &[Vec2], params: &Params) -> &'a [Vec2] {
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

        world_space.as_ref().unwrap()
    }

    pub fn set_shape(&mut self, device: &Device, queue: &Queue, shape: Shape) {
        self.vertices = shape.cast();
        let adjusted_verts = Self::cast_worldspace(&mut self.world_space, &self.vertices, &self.params);
        let (vertex_buffer, bind_group) = Self::create_vertex_buffer(device, queue, &self.layout, adjusted_verts);
        self.vertex_buffer.destroy();
        self.vertex_buffer = vertex_buffer;
        self.bind_group = bind_group;
    }

    pub fn edit(&mut self, queue: &Queue, mut f: impl FnMut(&mut Params)) {
        let mut params_c = self.params;
        f(&mut params_c);
        if params_c != self.params {
            self.params = params_c;
            let verts = Self::cast_worldspace(&mut self.world_space, &self.vertices, &self.params);
            Self::update_buffer(queue, &self.vertex_buffer, verts);
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
    length: u32
}