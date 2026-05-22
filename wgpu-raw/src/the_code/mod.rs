use wgpu::TextureFormat;

pub mod wgpu_app;




pub struct StartupInfo {
    pub display_tex_format: TextureFormat,
    pub display_size: (u32, u32),
}