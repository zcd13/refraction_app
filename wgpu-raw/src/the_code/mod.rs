
use encase::ShaderType;
use glam::Vec2;
use crate::into_wgsl;

pub mod wgpu_app;
mod geometry;
mod texture;
mod utils;
mod macros;

pub struct StartupInfo {
    pub display_tex_format: wgpu::TextureFormat,
    pub display_size: (u32, u32),
}

const RAY_COUNT: usize = 50_000;
const ITERATIONS: usize = 32;


into_wgsl!(
    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct LightRay {
        pub pos: Vec2,
        pub last_pos: Vec2,
        pub dir: Vec2,
        pub strength: f32,
        pub wave_length_and_ior: u32, // Combine u16s into u32 for easier alignment
    }
);



into_wgsl!(
    #[derive(ShaderType)]
    pub struct Settings {
        pub timestamp: f32,
        pub aspect: f32,
        pub mouse_pos_clip: Vec2,
        pub ray_count: u32,
        pub total_light: f32,
        pub a_factor: f32,
        pub b_factor: f32,
        pub brightness_scale: f32,

        // light settings
        pub spread: f32,
        pub width: f32,
        pub light_dir: f32, // dir in radians
        pub light_pos: Vec2,
    }
);

#[test]
fn test() {
    println!("{}", Settings::WGSL());
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            timestamp: 0.0,
            aspect: 1.0,
            mouse_pos_clip: [0.0; 2].into(),
            ray_count: RAY_COUNT as u32,
            total_light: 100_000.0,
            a_factor: 1.517,
            b_factor: 0.0042,
            brightness_scale: 0.15,
            spread: 0.0,
            width: 0.0005,
            light_dir: 0.1,
            light_pos: [-2.0, 0.0].into(),
        }
    }
}


