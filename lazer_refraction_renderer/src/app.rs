use eframe::egui::{Image, TextureFilter, TextureId, TextureOptions, Ui, Widget};
use eframe::egui_wgpu::RenderState;
use eframe::wgpu::wgt::{PollType, TextureViewDescriptor};
use eframe::wgpu::{
    Device, FilterMode, TexelCopyTextureInfo, Texture, TextureFormat, TextureUsages,
    TextureView,
};
use eframe::{wgpu, App, Frame};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use crate::background::wgpu_app::WgpuApplication;

pub struct EframeApp {
    wgpu_manager: Option<WgpuManager>,
    frame_pack: FramePack,
}
impl Default for EframeApp {
    fn default() -> Self {
        Self::new()
    }
}

impl EframeApp {
    pub fn new() -> Self {
        Self {
            wgpu_manager: None,
            frame_pack: Default::default()
        }
    }
}
impl App for EframeApp {
    fn ui(&mut self, ui: &mut Ui, frame: &mut Frame) {
        let txt = format!("Frame time {:?}", self.frame_pack.frametime);
        ui.heading(&txt);

        if self.wgpu_manager.is_none() {
            let res = frame.wgpu_render_state().unwrap();
            let size = ui.available_size();
            self.wgpu_manager = Some(WgpuManager::start(res, (size.x as u32, size.y as u32)));
        }
        if let Some(wgpu_manager) = &mut self.wgpu_manager {
            let f = wgpu_manager.render_frame(ui, frame.wgpu_render_state().unwrap());
            if let Some(f) = f { self.frame_pack = f };
        }

        ui.ctx().request_repaint_after(Duration::from_millis(7));
    }
}

pub struct StartupInfo {
    pub display_tex_format: TextureFormat,
    pub display_size: (u32, u32),
}


pub struct WgpuManager {
    worker: JoinHandle<()>,
    rf_sender: Sender<Option<((u32, u32), TextureView)>>,
    fin_recv: Receiver<Duration>,

    current_size: (u32, u32),
    render_texture: Texture,
    render_texture_view: TextureView,
    background_tex: Texture,
    egui_tex_id: TextureId,

    device: Device,
}
impl WgpuManager {
    pub fn start(render_state: &RenderState, display_size: (u32, u32)) -> Self {
        let info = StartupInfo {
            display_tex_format: render_state.target_format,
            display_size,
        };
        let mut wap = WgpuApplication::init(
            info,
            render_state.device.clone(),
            render_state.queue.clone(),
        );

        let (render_texture, render_texture_view) = Self::make_tex(
            display_size,
            render_state.target_format,
            &render_state.device,
        );
        let (other_tex, other_view) = Self::make_tex(
            display_size,
            render_state.target_format,
            &render_state.device,
        );

        let tex_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            &render_texture_view,
            FilterMode::Nearest,
        );

        let (rf_sender, rf_recv) = channel::<Option<((u32, u32), TextureView)>>();
        let (fin_sen, fin_recv) = channel::<Duration>();
        let worker = std::thread::spawn(move || {
            let mut tex_view = Some(other_view);
            for tex_change in rf_recv.iter() {
                if let Some((size, view)) = tex_change {
                    wap.resize(size);
                    tex_view = Some(view);
                }

                wap.update();

                if let Some(v) = &tex_view {
                    //todo web
                    let st = Instant::now();
                    wap.render(v);
                    fin_sen.send(st.elapsed()).unwrap();
                }
            }
        });

        Self {
            worker,
            rf_sender,
            fin_recv,
            current_size: display_size,
            render_texture,
            render_texture_view,
            background_tex: other_tex,
            egui_tex_id: tex_id,
            device: render_state.device.clone(),
        }
    }

    fn make_tex(
        size: (u32, u32),
        format: TextureFormat,
        device: &Device,
    ) -> (Texture, TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Display texture"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::COPY_DST
                | TextureUsages::COPY_SRC
                | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());
        (texture, view)
    }

    pub fn render_frame(&mut self, ui: &mut Ui, render_state: &RenderState) -> Option<FramePack> {
        let mut framepack = None;

        let size_v = ui.available_size();
        let size = (size_v.x as u32, size_v.y as u32);

        if size != self.current_size {
            (self.render_texture, self.render_texture_view) =
                Self::make_tex(size, self.render_texture.format(), &self.device);
            render_state
                .renderer
                .write()
                .free_texture(&self.egui_tex_id);
            self.egui_tex_id = render_state.renderer.write().register_native_texture(
                &render_state.device,
                &self.render_texture_view,
                FilterMode::Nearest,
            );
            let (_other_tex, other_view) =
                Self::make_tex(size, self.render_texture.format(), &self.device);
            self.current_size = size;
            self.rf_sender.send(Some((size, other_view))).unwrap();
            ui.spinner();
        } else {
            if let Ok(frametime) = self.fin_recv.try_recv() {
                framepack = Some(FramePack {
                    frametime,
                });

                let mut encoder = self
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Weird fucky encoder") });
                encoder.copy_texture_to_texture(
                    TexelCopyTextureInfo {
                        texture: &self.background_tex,
                        mip_level: 0,
                        origin: Default::default(),
                        aspect: Default::default(),
                    },
                    TexelCopyTextureInfo {
                        texture: &self.render_texture,
                        mip_level: 0,
                        origin: Default::default(),
                        aspect: Default::default(),
                    },
                    self.render_texture.size(),
                );

                let command_buffer = encoder.finish();
                render_state.queue.submit(Some(command_buffer));
                self.device.poll(PollType::Wait { submission_index: None, timeout: None }).unwrap();
            }

            let image = Image::new((self.egui_tex_id, size_v))
                .fit_to_exact_size(size_v)
                .texture_options(TextureOptions {
                    magnification: TextureFilter::Nearest,
                    minification: TextureFilter::Nearest,
                    wrap_mode: Default::default(),
                    mipmap_mode: None,
                });
            image.ui(ui);

            self.rf_sender.send(None).unwrap();
        }


        framepack
    }
}

#[derive(Default)]
pub struct FramePack {
    frametime: Duration,
}
