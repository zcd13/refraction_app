pub mod wgpu_app;
mod geometry;
mod texture;

pub struct StartupInfo {
    pub display_tex_format: wgpu::TextureFormat,
    pub display_size: (u32, u32),
}