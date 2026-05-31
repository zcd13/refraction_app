
use encase::{ShaderType};
use glam::Vec2;
use macro_rules_attribute::apply;
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

// const RAY_COUNT: usize = 50_000;
// const ITERATIONS: usize = 32;


#[apply(into_wgsl)]
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightRay {
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub dir: Vec2,
    pub strength: f32,
    pub wave_length_and_ior: u32, // Combine u16s into u32 for easier alignment
}



#[apply(into_wgsl)]
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
    pub absobtion: f32,
    pub bounces: u32,

    // light settings
    pub spread: f32,
    pub width: f32,
    pub light_dir: f32, // dir in radians
    pub light_pos: Vec2,
    pub follow_mouse: u32, // 0 = no 1 = yes

    pub nudge_factor: f32,
    pub tonemapping: u32,
    pub debug_cutoff: f32,
}


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
            ray_count: 50_000,
            total_light: 14200.0,
            a_factor: 1.752,
            b_factor: 0.0103,
            brightness_scale: 1.0,
            absobtion: 0.53,
            bounces: 25,
            spread: 0.0,
            width: 0.00191,
            light_dir: 0.0,
            light_pos: [-2.0, 0.280].into(),
            follow_mouse: 0,
            nudge_factor: 0.0001,
            tonemapping: 1,
            debug_cutoff: 0.0,
        }
    }
}


