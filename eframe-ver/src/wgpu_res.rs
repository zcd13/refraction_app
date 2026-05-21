use bytemuck::Pod;
use bytemuck::Zeroable;
use eframe::egui::Vec2;
use eframe::wgpu;
use eframe::wgpu::{VertexAttribute, VertexFormat};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex(pub [f32; 2]);
impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[VertexAttribute {
                format: VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        }
    }
}

impl From<Vec2> for Vertex {
    fn from(value: Vec2) -> Self {
        Self([value.x, value.y])
    }
}