#![allow(unused_variables)]

use eframe::wgpu::{Device, Queue, TextureView};
use crate::app::StartupInfo;

pub struct WgpuApplication {}
impl WgpuApplication {
    pub fn init(startup_info: StartupInfo, device: Device, queue: Queue) -> Self {
        todo!()
    }

    pub fn resize(&mut self, viewport_size: (u32, u32)) {
        todo!()
    }

    pub fn update(&mut self) {
        todo!()
    }

    pub fn render(&mut self, view: &TextureView) {
        todo!()
    }
}