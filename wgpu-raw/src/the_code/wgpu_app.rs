#![allow(unused_variables)]

use crate::the_code::texture::{GpuTexture, GpuTextureBuilder, HasSampler};
use crate::the_code::utils::{AutoUniform, SimpleBuffer, SimpleUniform, TextureSamplerRenderer};
use crate::the_code::{LightRay, Settings, StartupInfo, ITERATIONS, RAY_COUNT};
use glam::vec2;
use std::time::Instant;
use wgpu::wgt::SamplerDescriptor;
use wgpu::{
    BlendComponent, BlendFactor, BlendOperation, BlendState
    , Color, ColorTargetState, ColorWrites, CommandEncoder,
    ComputePipeline, ComputePipelineDescriptor, Device, FilterMode, FragmentState, FrontFace,
    MultisampleState, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology,
    Queue, RenderPipeline, RenderPipelineDescriptor, ShaderModule, ShaderModuleDescriptor,
    ShaderSource, TextureFormat, TextureUsages, TextureView, VertexState,
};

pub struct WgpuApplication {
    device: Device,
    queue: Queue,

    light_ray_buffer: SimpleBuffer<LightRay>,
    line_texture: GpuTexture<HasSampler>,
    line_primitive_renderer: LinePrimitiveRenderer,

    display_pipeline: TextureSamplerRenderer,

    line_physics_compute_pass: LinePhysicsComputePass,

    start_instant: Instant,
    settings_uniform: AutoUniform<Settings>,

    screen_size: (u32, u32),
}
impl WgpuApplication {
    pub fn init(startup_info: StartupInfo, device: Device, queue: Queue) -> Self {
        let screen_size = startup_info.display_size;

        let light_ray_buffer = SimpleBuffer::init(&device, RAY_COUNT);

        let line_texture = GpuTextureBuilder::new(
            &device,
            startup_info.display_size,
            TextureFormat::Rgba16Float,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        )
        .with_sampler(
            SamplerDescriptor {
                mag_filter: FilterMode::Nearest,
                min_filter: FilterMode::Nearest,
                ..Default::default()
            },
            true,
        )
        .make();

        let start_instant = Instant::now();
        let settings_uniform = AutoUniform::init(
            &device,
            Settings {
                aspect: screen_size.0 as f32 / screen_size.1 as f32,
                ..Default::default()
            },
        );

        let line_primitive_renderer = LinePrimitiveRenderer::init(
            &device,
            &light_ray_buffer,
            line_texture.format,
            &settings_uniform,
        );

        let display_pipeline =
            TextureSamplerRenderer::init(&device, &line_texture, startup_info.display_tex_format);

        let line_physics_compute_pass =
            LinePhysicsComputePass::init(&device, &light_ray_buffer, &settings_uniform);

        Self {
            device,
            queue,
            light_ray_buffer,
            line_texture,
            line_primitive_renderer,
            display_pipeline,
            line_physics_compute_pass,
            start_instant,
            settings_uniform,
            screen_size,
        }
    }

    pub fn resize(&mut self, viewport_size: (u32, u32)) {
        self.screen_size = viewport_size; // ← add this

        self.line_texture.resize(&self.device, viewport_size);
        self.settings_uniform.edit(&self.queue, |s| {
            s.aspect = viewport_size.0 as f32 / viewport_size.1 as f32;
        });
    }

    pub fn update(&mut self, ppp: f32, ctx: &egui::Context) {
        let s = self.settings_uniform.mod_data();

        egui::Window::new("winit + egui + wgpu says hello!")
            .resizable(true)
            .vscroll(true)
            .default_open(false)
            .show(ctx, |ui| {
                ui.label("Label!");

                if ui.button("Button!").clicked() {
                    println!("boom!")
                }

                ui.label(format!("{}", ppp));
            });

        ctx.input(|i| {
            if let Some(p) = i.pointer.latest_pos() {
                let p = p * ppp;

                let mp = mouse_to_clip(p.into(), self.screen_size);
                let aspect = self.screen_size.0 as f32 / self.screen_size.1 as f32;
                let mp_world = vec2(mp.0 * aspect, mp.1);

                s.mouse_pos_clip = mp_world.into();
            }
        });

        s.timestamp = self.start_instant.elapsed().as_secs_f32();

        self.settings_uniform.write_current_data(&self.queue);
    }

    pub fn render(&mut self, view: &TextureView, encoder: &mut CommandEncoder) {
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Line Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.line_texture.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }

        self.line_physics_compute_pass.init_pass(
            encoder,
            &self.light_ray_buffer,
            &self.settings_uniform,
        );

        for _ in 0..ITERATIONS {
            self.line_physics_compute_pass.compute_pass(
                encoder,
                &self.light_ray_buffer,
                &self.settings_uniform,
            );

            self.line_primitive_renderer.render_lines(
                encoder,
                &self.line_texture.view,
                &self.light_ray_buffer,
                &self.settings_uniform,
            );
        }

        self.line_primitive_renderer.render_geometry(
            encoder,
            &self.line_texture.view,
            &self.settings_uniform,
        );

        self.display_pipeline
            .render(encoder, view, &self.line_texture);
    }
}

pub struct LinePrimitiveRenderer {
    ray_pipeline: RenderPipeline,

    geometry_pipeline: RenderPipeline,
}
impl LinePrimitiveRenderer {
    pub fn init(
        device: &Device,
        rays_buffer: &SimpleBuffer<LightRay>,
        output_format: TextureFormat,
        settings_uniform: &AutoUniform<Settings>,
    ) -> Self {
        let line_shader_module = combine(
            device,
            &[
                include_str!("shaders/draw_lines.wgsl"),
                include_str!("shaders/definitions.wgsl"),
                Settings::WGSL(),
                LightRay::WGSL(),
            ],
        );

        let line_render_pipeline_layout =
            device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("line_render_pipeline_layout"),
                bind_group_layouts: &[
                    Some(&rays_buffer.read_only_layout),
                    Some(&settings_uniform.bind_group_layout),
                ],
                immediate_size: 0,
            });

        let line_render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("line_render_pipeline"),
            layout: Some(&line_render_pipeline_layout),
            vertex: VertexState {
                module: &line_shader_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(FragmentState {
                module: &line_shader_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format: output_format,
                    // blend: None,
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent::REPLACE,
                    }),
                    write_mask: ColorWrites::COLOR,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let geo_shader_module = combine(
            device,
            &[
                include_str!("shaders/draw_geometry.wgsl"),
                include_str!("shaders/definitions.wgsl"),
                include_str!("shaders/collision.wgsl"),
                Settings::WGSL(),
                LightRay::WGSL(),
            ],
        );

        let geometry_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("geometry_layout"),
            bind_group_layouts: &[Some(&settings_uniform.bind_group_layout)],
            immediate_size: 0,
        });

        let geometry_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("line_render_pipeline"),
            layout: Some(&geometry_layout),
            vertex: VertexState {
                module: &geo_shader_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::LineStrip,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(FragmentState {
                module: &geo_shader_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format: output_format,
                    // blend: None,
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent::REPLACE,
                    }),
                    write_mask: ColorWrites::COLOR,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Self {
            ray_pipeline: line_render_pipeline,
            geometry_pipeline,
        }
    }

    pub fn render_lines(
        &self,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        ray_buffer: &SimpleBuffer<LightRay>,
        settings_uniform: &AutoUniform<Settings>,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Line Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        render_pass.set_pipeline(&self.ray_pipeline);

        render_pass.set_bind_group(0, &ray_buffer.read_only, &[]);
        render_pass.set_bind_group(1, &settings_uniform.bind_group, &[]);
        render_pass.draw(0..2, 0..ray_buffer.size_elements);
    }

    pub fn render_geometry(
        &self,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        settings_uniform: &AutoUniform<Settings>,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Line Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        render_pass.set_pipeline(&self.geometry_pipeline);
        render_pass.set_bind_group(0, &settings_uniform.bind_group, &[]);
        render_pass.draw(0..10, 0..1);
    }
}

pub fn format_bytes(bytes: usize) -> String {
    let mut size = bytes as f64;
    let suffixes = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut i = 0;

    while size >= 1024.0 && i < suffixes.len() - 1 {
        size /= 1024.0;
        i += 1;
    }

    // Don't show decimal places if it's just bytes
    if i == 0 {
        format!("{} {}", bytes, suffixes[i])
    } else {
        format!("{:.2} {}", size, suffixes[i])
    }
}

pub fn combine(device: &Device, shaders: &[&'static str]) -> ShaderModule {
    let mut fin = String::new();
    for s in shaders {
        fin.push_str(s)
    }
    device.create_shader_module(ShaderModuleDescriptor {
        label: None,
        source: ShaderSource::Wgsl(fin.into()),
    })
}

fn mouse_to_clip(mouse_pos: (f32, f32), screen_size: (u32, u32)) -> (f32, f32) {
    let clip_x = (mouse_pos.0 / screen_size.0 as f32) * 2.0 - 1.0;
    let clip_y = 1.0 - (mouse_pos.1 / screen_size.1 as f32) * 2.0; // ← flip Y
    (clip_x, clip_y)
}

pub struct LinePhysicsComputePass {
    compute_pipeline: ComputePipeline,
    init_pipeline: ComputePipeline,
}
impl LinePhysicsComputePass {
    pub fn init(
        device: &Device,
        light_ray_buffer: &SimpleBuffer<LightRay>,
        settings_uniform: &AutoUniform<Settings>,
    ) -> Self {
        let shader_module = combine(
            device,
            &[
                include_str!("shaders/compute_pass.wgsl"),
                include_str!("shaders/collision.wgsl"),
                include_str!("shaders/definitions.wgsl"),
                Settings::WGSL(),
                LightRay::WGSL(),
            ],
        );

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Compute Pipeline Layout"),
            bind_group_layouts: &[
                Some(&light_ray_buffer.read_write_layout),
                Some(&settings_uniform.bind_group_layout),
            ],
            immediate_size: 0,
        });

        let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Compute Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let init_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Compute Pipeline Layout"),
            bind_group_layouts: &[
                Some(&light_ray_buffer.read_write_layout),
                Some(&settings_uniform.bind_group_layout),
            ],
            immediate_size: 0,
        });

        let init_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Compute Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: Some("init"),
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            compute_pipeline,
            init_pipeline,
        }
    }

    pub fn compute_pass(
        &self,
        encoder: &mut CommandEncoder,
        light_ray_buffer: &SimpleBuffer<LightRay>,
        settings_uniform: &AutoUniform<Settings>,
    ) {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Compute Pass"),
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(&self.compute_pipeline);
        compute_pass.set_bind_group(0, &light_ray_buffer.read_write, &[]);
        compute_pass.set_bind_group(1, &settings_uniform.bind_group, &[]);

        let workgroup_size = 256;
        let total_elements = light_ray_buffer.size_elements;
        let workgroup_count = total_elements.div_ceil(workgroup_size);
        compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
    }

    pub fn init_pass(
        &self,
        encoder: &mut CommandEncoder,
        light_ray_buffer: &SimpleBuffer<LightRay>,
        settings_uniform: &AutoUniform<Settings>,
    ) {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Init Pass"),
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(&self.init_pipeline);
        compute_pass.set_bind_group(0, &light_ray_buffer.read_write, &[]);
        compute_pass.set_bind_group(1, &settings_uniform.bind_group, &[]);

        let workgroup_size = 256;
        let total_elements = light_ray_buffer.size_elements;
        let workgroup_count = total_elements.div_ceil(workgroup_size);
        compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
}
