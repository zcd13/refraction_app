use wgpu::TextureFormat;

pub mod wgpu_app;
mod geometry;

pub struct StartupInfo {
    pub display_tex_format: TextureFormat,
    pub display_size: (u32, u32),
}