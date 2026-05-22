use eframe::egui;
use crate::w_app::WCont;

#[derive(Default)]
pub struct App {
    w_context: Option<WCont>
}

impl App {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // 1. Initialize the background renderer on the first frame
        if self.w_context.is_none()
            && let Some(render_state) = frame.wgpu_render_state() {
                self.w_context = Some(WCont::init(render_state, ui.ctx().clone()));
            }

        ui.ctx().request_repaint_after(std::time::Duration::from_millis(7));

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Hello WGPU Triangle!");

            // 3. Draw the texture, scaling it to fill the available space
            if let Some(w_cont) = &mut self.w_context {
                let available_size = ui.available_size();
                w_cont.render_frame(ui, available_size, frame.wgpu_render_state().unwrap());
            }
        });
    }
}

