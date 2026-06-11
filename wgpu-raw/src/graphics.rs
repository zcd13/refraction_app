#![allow(dead_code)]

use std::sync::Arc;
use egui_wgpu::ScreenDescriptor;
use wgpu::{Adapter, CurrentSurfaceTexture, Device, DeviceDescriptor, Features, Instance, MemoryHints, PowerPreference, PresentMode, Queue, RequestAdapterOptions, Surface, SurfaceConfiguration, TextureViewDescriptor};
use winit::{dpi::PhysicalSize, event_loop::EventLoopProxy, window::Window};
use winit::event::WindowEvent;
use crate::egui_renderer::EguiRenderer;
use crate::the_code::StartupInfo;
use crate::the_code::wgpu_app::WgpuApplication;

// #[cfg(target_arch = "wasm32")]
// pub type Rc<T> = std::rc::Rc<T>;

// #[cfg(not(target_arch = "wasm32"))]
// pub type Rc<T> = std::sync::Arc<T>;

pub async fn create_graphics(window: Arc<Window>, proxy: EventLoopProxy<Graphics>) {
    let instance = Instance::default();
    let surface = instance.create_surface(Arc::clone(&window)).unwrap();
    let adapter = instance
        .request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        })
        .await
        .expect("Could not get an adapter");

    let (device, queue) = adapter
        .request_device(&DeviceDescriptor {
            label: None,
            required_features: Features::empty(),
            // todo set min limits for the project
            required_limits: adapter.limits(),
            memory_hints: MemoryHints::Performance,
            trace: Default::default(),
            experimental_features: Default::default(),
        })
        .await
        .expect("Failed to get device");

    let size = window.inner_size();
    let width = size.width.max(1);
    let height = size.height.max(1);
    let mut surface_config = surface
        .get_default_config(&adapter, width, height).unwrap();
    surface_config.present_mode = PresentMode::Fifo;


    surface.configure(&device, &surface_config);

    let wgpu_application = WgpuApplication::init(StartupInfo {
        display_tex_format: surface_config.format,
        display_size: (width, height),
    }, device.clone(), queue.clone());

    let egui_renderer = EguiRenderer::new(&device, surface_config.format, None, 0, &window);

    let gfx = Graphics {
        window: window.clone(),
        instance,
        surface,
        surface_config,
        adapter,
        device,
        queue,
        wgpu_application,
        mouse_pos: (0.0, 0.0),
        egui_renderer,
    };

    let _ = proxy.send_event(gfx);
}


pub struct Graphics {
    window: Arc<Window>,
    instance: Instance,
    surface: Surface<'static>,
    surface_config: SurfaceConfiguration,
    adapter: Adapter,
    device: Device,
    queue: Queue,
    wgpu_application: WgpuApplication,
    pub mouse_pos: (f32, f32),
    egui_renderer: EguiRenderer,
}

impl Graphics {
    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.surface_config.width = new_size.width.max(1);
        self.surface_config.height = new_size.height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
        self.wgpu_application.resize((self.surface_config.width, self.surface_config.height));
    }

    pub fn draw(&mut self) {
        let ppp = self.egui_renderer.context().pixels_per_point();

        if let Some(min) = self.window.is_minimized() {
            if min {
                println!("Window is minimized");
                return;
            }
        }

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.surface_config.width, self.surface_config.height],
            pixels_per_point: self.window.scale_factor() as f32
                * 1.0,
        };

        let surface_texture = self.surface.get_current_texture();

        let surface_texture = match surface_texture {
            CurrentSurfaceTexture::Success(tex) => tex,
            _ => {
                eprintln!("Texture problems");
                return;
            }
        };

        let surface_view = surface_texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let window = &self.window;

        self.wgpu_application.render(&surface_view, &mut encoder);

        {
            self.egui_renderer.begin_frame(window);

            self.wgpu_application.update(ppp, self.egui_renderer.context());

            self.egui_renderer.end_frame_and_draw(
                &self.device,
                &self.queue,
                &mut encoder,
                window,
                &surface_view,
                screen_descriptor,
            );
        }

        self.queue.submit(Some(encoder.finish()));
        surface_texture.present();

    }

    pub fn handle_input(&mut self, event: &WindowEvent) {
        self.egui_renderer.handle_input(&self.window, event);
    }
}