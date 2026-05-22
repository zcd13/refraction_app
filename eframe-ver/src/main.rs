

mod app;
mod w_app;
mod scene;
mod geometry;
mod wgpu_res;

use std::sync::Arc;
use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
use eframe::wgpu::{BackendOptions, Backends, ExperimentalFeatures, InstanceDescriptor, InstanceFlags, MemoryBudgetThresholds, PowerPreference};
use eframe::wgpu::wgt::DeviceDescriptor;
use crate::app::App;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use eframe::{NativeOptions, Renderer};

    let native_options = NativeOptions {
        vsync: false,
        renderer: Renderer::Wgpu,
        wgpu_options: WgpuConfiguration {
            wgpu_setup: WgpuSetup::CreateNew(WgpuSetupCreateNew {
                instance_descriptor: InstanceDescriptor {
                    backends: Backends::all(),
                    flags: InstanceFlags::empty(),
                    memory_budget_thresholds: MemoryBudgetThresholds::default(),
                    backend_options: BackendOptions::default(),
                    display: None,
                },
                display_handle: None,
                power_preference: PowerPreference::HighPerformance ,
                native_adapter_selector: None,
                device_descriptor: Arc::new(|adapter| {
                    DeviceDescriptor {
                        label: Some("Device Desc native"),
                        required_features: adapter.features(),
                        required_limits: adapter.limits(),
                        experimental_features: unsafe { ExperimentalFeatures::enabled() },
                        memory_hints: Default::default(),
                        trace: Default::default(),
                    }
                }),
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    eframe::run_native(
        "My egui App",
        native_options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
    .unwrap();
}


// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions {
        renderer: Renderer::Wgpu,
        max_fps: None,
        ..Default::default()
    };

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(App::new(cc)))),
            )
            .await;

        // Remove the loading text and spinner:
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}