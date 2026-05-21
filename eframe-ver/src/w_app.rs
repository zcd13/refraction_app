#![allow(unused_assignments)]

use std::sync::{Arc, Mutex};
use eframe::{egui, wgpu};
use eframe::egui::{Image, TextureFilter, TextureOptions, Widget};
use eframe::egui_wgpu::RenderState;
use eframe::epaint::Vec2;
use crate::scene::Scene;

pub struct WCont {
    // Stores the raw wgpu TextureView from the background thread
    latest_raw_view: Arc<Mutex<Option<wgpu::TextureView>>>,
    // Tracks what egui actually has registered
    active_egui_texture: Option<egui::TextureId>,

    is_running: Arc<Mutex<bool>>,
    target_size: Arc<Mutex<(u32, u32)>>,
    // Added to tell egui to repaint immediately when a new frame is ready
    egui_ctx: egui::Context,
}

impl WCont {
    pub fn init(render_state: &RenderState, egui_ctx: egui::Context) -> Self {
        let latest_raw_view = Arc::new(Mutex::new(None));
        let is_running = Arc::new(Mutex::new(true));
        let target_size = Arc::new(Mutex::new((512, 512)));

        let raw_view_clone = latest_raw_view.clone();
        let running_clone = is_running.clone();
        let size_clone = target_size.clone();
        let ctx_clone = egui_ctx.clone();

        let device = Arc::new(render_state.device.clone());
        let queue = Arc::new(render_state.queue.clone());

        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                Self::background_render_loop(device, queue, raw_view_clone, running_clone, size_clone, ctx_clone);
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                Self::background_render_loop_wasm(device, queue, raw_view_clone, running_clone, size_clone, ctx_clone).await;
            });
        }

        Self {
            latest_raw_view,
            active_egui_texture: None,
            is_running,
            target_size,
            egui_ctx,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn background_render_loop(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        raw_view_store: Arc<Mutex<Option<wgpu::TextureView>>>,
        is_running: Arc<Mutex<bool>>,
        target_size: Arc<Mutex<(u32, u32)>>,
        egui_ctx: egui::Context,
    ) {
        let mut scene = Scene::init(device.clone(), queue.clone());

        while *is_running.lock().unwrap() {
            let (width, height) = *target_size.lock().unwrap();
            scene.resize(width, height);

            if let Some(view) = scene.render() {
                // Hand off the raw texture view safely without touching egui's renderer
                {
                    let mut store = raw_view_store.lock().unwrap();
                    *store = Some(view.clone());
                }
                // Wake up the GUI loop instantly for a zero-sleep cycle
                egui_ctx.request_repaint();
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    async fn background_render_loop_wasm(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        raw_view_store: Arc<Mutex<Option<wgpu::TextureView>>>,
        is_running: Arc<Mutex<bool>>,
        target_size: Arc<Mutex<(u32, u32)>>,
        egui_ctx: egui::Context,
    ) {
        // Change back to Scene if TestSceneRenderer was a typo in your script
        let mut scene = Scene::init(device.clone(), queue.clone());

        while *is_running.lock().unwrap() {
            // yield control to prevent freezing the web browser tab
            let _ = gloo_timers::future::TimeoutFuture::new(0).await;

            let (width, height) = *target_size.lock().unwrap();
            scene.resize(width, height);

            if let Some(view) = scene.render() {
                {
                    let mut store = raw_view_store.lock().unwrap();
                    *store = Some(view);
                }
                egui_ctx.request_repaint();
            }
        }
    }

    pub fn render_frame(&mut self, ui: &mut egui::Ui, size: Vec2, render_state: &RenderState) {
        // 1. Communicate size adjustments back
        *self.target_size.lock().unwrap() = (size.x as u32, size.y as u32);

        // 2. Safely process textures inside the main thread frame loop
        let mut raw_view_guard = self.latest_raw_view.lock().unwrap();
        if let Some(new_view) = raw_view_guard.take() {
            let mut renderer_guard = render_state.renderer.write();

            // Clean up the previous frame's registration safely
            if let Some(old_id) = self.active_egui_texture {
                renderer_guard.free_texture(&old_id);
            }

            // Register our fresh frame natively
            self.active_egui_texture = Some(renderer_guard.register_native_texture(
                &render_state.device,
                &new_view,
                wgpu::FilterMode::Linear
            ));
        }

        // 3. Output the texture if present
        if let Some(texture_id) = self.active_egui_texture {
            let image = Image::new((texture_id, size))
                .fit_to_exact_size(size)
                .texture_options(TextureOptions {
                    magnification: TextureFilter::Nearest,
                    minification: TextureFilter::Nearest,
                    wrap_mode: Default::default(),
                    mipmap_mode: None,
                });
            image.ui(ui);
        } else {
            ui.spinner();
        }
    }
}

impl Drop for WCont {
    fn drop(&mut self) {
        if let Ok(mut running) = self.is_running.lock() {
            *running = false;
        }
    }
}