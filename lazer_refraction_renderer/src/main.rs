use std::sync::Arc;
use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
use eframe::wgpu::{Adapter, BackendOptions, Backends, DeviceDescriptor, InstanceDescriptor, InstanceFlags, MemoryBudgetThresholds, PowerPreference};
use lazer_refraction_renderer::app::EframeApp;

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
                device_descriptor: Arc::new(|adapter: &Adapter| {
                    DeviceDescriptor {
                        label: Some("Device Desc native"),
                        required_features: adapter.features(),
                        required_limits: adapter.limits(),
                        memory_hints: Default::default(),
                        trace: Default::default(),
                        ..Default::default()
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
        Box::new(|_cc| Ok(Box::new(EframeApp::new()))),
    )
        .unwrap();
}