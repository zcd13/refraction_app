#![allow(unused_variables)]

use wgpu::{Device, Queue, TextureView};
use crate::the_code::StartupInfo;

#[derive(Debug)]
pub struct WgpuApplication {}
impl WgpuApplication {
    pub fn init(startup_info: StartupInfo, device: Device, queue: Queue) -> Self {

        Self {}
    }

    pub fn resize(&mut self, viewport_size: (u32, u32)) {

    }

    pub fn update(&mut self) {

    }

    pub fn render(&mut self, view: &TextureView) {

    }
}