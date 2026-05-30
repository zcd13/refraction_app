#![allow(dead_code)]

use std::time::{Duration, Instant};
use egui_wgpu::ScreenDescriptor;
use wgpu::{Adapter, CurrentSurfaceTexture, Device, DeviceDescriptor, Features, Instance, Limits, MemoryHints, PowerPreference, PresentMode, Queue, RequestAdapterOptions, Surface, SurfaceConfiguration, TextureViewDescriptor};
use winit::{dpi::PhysicalSize, event_loop::EventLoopProxy, window::Window};
use winit::event::WindowEvent;
use crate::egui_renderer::EguiRenderer;
use crate::the_code::StartupInfo;
use crate::the_code::wgpu_app::WgpuApplication;

#[cfg(target_arch = "wasm32")]
pub type Rc<T> = std::rc::Rc<T>;

#[cfg(not(target_arch = "wasm32"))]
pub type Rc<T> = std::sync::Arc<T>;

pub async fn create_graphics(window: Rc<Window>, proxy: EventLoopProxy<Graphics>) {
    let instance = Instance::default();
    let surface = instance.create_surface(Rc::clone(&window)).unwrap();
    let adapter = instance
        .request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance, // Power preference for the device
            force_fallback_adapter: false, // Indicates that only a fallback ("software") adapter can be used
            compatible_surface: Some(&surface), // Guarantee that the adapter can render to this surface
        })
        .await
        .expect("Could not get an adapter (GPU).");

    let (device, queue) = adapter
        .request_device(&DeviceDescriptor {
            label: None,
            required_features: Features::empty(), // Specifies the required features by the device request. Fails if the adapter can't provide them.
            // todo set min limits for the project
            required_limits: adapter.limits(),
            memory_hints: MemoryHints::Performance,
            trace: Default::default(),
            experimental_features: Default::default(),
        })
        .await
        .expect("Failed to get device");

    // Get physical pixel dimensions inside the window
    let size = window.inner_size();
    // Make the dimensions at least size 1, otherwise wgpu would panic
    let width = size.width.max(1);
    let height = size.height.max(1);
    let mut surface_config = surface
        .get_default_config(&adapter, width, height).unwrap();
    surface_config.present_mode = PresentMode::Immediate;


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
        fps_counter: FpsCounter::new(Duration::from_secs(1)),
        mouse_pos: (0.0, 0.0),
        egui_renderer,
    };

    let _ = proxy.send_event(gfx);
}


pub struct Graphics {
    window: Rc<Window>,
    instance: Instance,
    surface: Surface<'static>,
    surface_config: SurfaceConfiguration,
    adapter: Adapter,
    device: Device,
    queue: Queue,
    wgpu_application: WgpuApplication,
    fps_counter: FpsCounter,
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




pub struct FpsCounter {
    print_interval: Duration,
    last_print_time: Instant,
    frame_count: u32,
}
impl FpsCounter {
    /// Creates and starts the FPS counter
    pub fn new(print_interval: Duration) -> Self {
        Self {
            print_interval,
            last_print_time: Instant::now(),
            frame_count: 0,
        }
    }

    /// Call this once per frame.
    /// It automatically prints and resets when the interval is reached.
    pub fn update(&mut self) {
        self.frame_count += 1;

        let elapsed = self.last_print_time.elapsed();

        if elapsed >= self.print_interval {
            // Calculate frames per second
            let fps = self.frame_count as f64 / elapsed.as_secs_f64();

            println!("FPS: {:.2}", fps);

            // Reset the counter and timer for the next batch
            self.frame_count = 0;
            self.last_print_time = Instant::now();
        }
    }
}