#![allow(unused_variables)]

use crate::the_code::geometry::{Polygon, Shape};
use crate::the_code::texture::{GpuTexture, GpuTextureBuilder, HasSampler};
use crate::the_code::utils::{AutoUniform, FpsCounter, SimpleBuffer};
use crate::the_code::{LightRay, Settings, StartupInfo};
use egui::{ComboBox, DragValue, Slider};
use glam::{vec2, Vec2};
use web_time::{Duration, Instant};
use wgpu::wgt::SamplerDescriptor;
use wgpu::{BlendComponent, BlendFactor, BlendOperation, BlendState, Color, ColorTargetState, ColorWrites, CommandEncoder, ComputePipeline, ComputePipelineDescriptor, Device, FilterMode, FragmentState, FrontFace, MultisampleState, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, Queue, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, ShaderModule, ShaderModuleDescriptor, ShaderSource, TextureFormat, TextureUsages, TextureView, VertexState};

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

    polygon: Polygon,

    screen_size: (u32, u32),

    fps_counter: FpsCounter,
}
impl WgpuApplication {
    pub fn init(startup_info: StartupInfo, device: Device, queue: Queue) -> Self {
        let screen_size = startup_info.display_size;
        let start_instant = Instant::now();
        let settings_uniform = AutoUniform::init(
            &device,
            Settings {
                aspect: screen_size.0 as f32 / screen_size.1 as f32,
                ..Default::default()
            },
        );

        let light_ray_buffer = SimpleBuffer::init(&device, settings_uniform.data().ray_count as usize);

        let polygon = Polygon::init(&device, Shape::Circle(35));

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



        let line_primitive_renderer = LinePrimitiveRenderer::init(
            &device,
            &light_ray_buffer,
            line_texture.format,
            &settings_uniform,
            &polygon.buffer,
        );

        let display_pipeline =
            TextureSamplerRenderer::init(&device, &line_texture, startup_info.display_tex_format, &settings_uniform);

        let line_physics_compute_pass = LinePhysicsComputePass::init(
            &device,
            &light_ray_buffer,
            &settings_uniform,
            &polygon.buffer,
        );



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
            polygon,
            screen_size,
            fps_counter: FpsCounter::new(Duration::from_millis(250)),
        }
    }

    pub fn resize(&mut self, viewport_size: (u32, u32)) {
        self.screen_size = viewport_size;

        self.line_texture.resize(&self.device, viewport_size);
        self.settings_uniform.edit(&self.queue, |s| {
            s.aspect = viewport_size.0 as f32 / viewport_size.1 as f32;
        });
    }

    pub fn update(&mut self, ppp: f32, ctx: &egui::Context) {
        let fps = self.fps_counter.update();
        let s = self.settings_uniform.mod_data();

        egui::Window::new("Settings")
            .resizable(false)
            .default_width(0.0)
            .show(ctx, |ui| {
            ui.label(format!("{fps:.2} FPS"));

            ui.group(|ui| {
                let mut new = self.polygon.current_shape;
                ComboBox::from_label("Shape")
                    .selected_text(format!("{:?}", self.polygon.current_shape))
                    .show_ui(ui, |ui| {
                        for shape in Shape::iterate_def() {
                            ui.selectable_value(&mut new, *shape, shape.name());
                        }
                    });


                if new != self.polygon.current_shape {
                    self.polygon.set_shape(&self.device, new);
                };

                if let Shape::Circle(x) = self.polygon.current_shape {
                    let mut c = x;
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            if ui.button("+").clicked() {
                                c += 1;
                            };
                            if ui.button("-").clicked() {
                                c -= 1;
                            };
                            ui.add(DragValue::new(&mut c).prefix("+/-: "));
                        });
                    });
                    c = c.clamp(3, u32::MAX);
                    if c != x {
                        self.polygon.set_shape(&self.device, Shape::Circle(c));
                    };
                }

                self.polygon.edit(&self.queue, |s| {
                    ui.add(
                        DragValue::new(&mut s.scale)
                            .prefix("Scale: ")
                            .range(0.0..=100.0)
                            .speed(0.01),
                    );
                    ui.add(DragValue::new(&mut s.rot).prefix("Rotation: ").speed(0.001));

                    ui.add(DragValue::new(&mut s.pos.x).prefix("Pos x: ").speed(0.001));
                    ui.add(DragValue::new(&mut s.pos.y).prefix("Pos y: ").speed(0.001));
                });
            });

            ui.group(|ui| {
                ui.heading("Lighting");
                ui.add(DragValue::new(&mut s.total_light).prefix("Total Light: ").speed(100.0).range(0.0..=f32::MAX));
                ui.add(Slider::new(&mut s.absobtion, 0.0..=1.0).prefix("Absorption: "));
                ui.add(DragValue::new(&mut s.brightness_scale).prefix("Brightness Scale: ").speed(0.05).range(-100.0..=100.0));
                ui.add(DragValue::new(&mut s.spread).prefix("Spread: ").speed(0.0001).range(0.0..=f32::MAX));
                ui.add(DragValue::new(&mut s.width).prefix("Width: ").speed(0.00001).range(0.0..=f32::MAX), );

                let mut checked = s.follow_mouse == 1;
                if ui.checkbox(&mut checked, "Follow mouse").clicked() {
                    s.follow_mouse = if checked { 1 } else { 0 };
                }

                ui.add(DragValue::new(&mut s.light_dir).prefix("Light Dir: ").speed(0.00001));

                ui.horizontal(|ui| {
                    ui.label("Light Pos:");
                    ui.add(DragValue::new(&mut s.light_pos.x).prefix("x: ").speed(0.001));
                    ui.add(DragValue::new(&mut s.light_pos.y).prefix("y: ").speed(0.001));
                });

                ui.separator();

                ui.heading("Glass / Material (Cauchy)");
                ui.add(DragValue::new(&mut s.a_factor).prefix("IOR (a): ").speed(0.001).range(0.0..=3.0));

                ui.add(DragValue::new(&mut s.b_factor).prefix("Dispersion (b): ").speed(0.0001));

                ui.separator();

                ui.heading("Engine");
                ui.add(DragValue::new(&mut s.nudge_factor).prefix("Nudge Factor: ").speed(0.00001));

                ui.add(Slider::new(&mut s.bounces, 1..=128).prefix("Bounces: "));

                if ui.add(
                    DragValue::new(&mut s.ray_count)
                        .prefix("Ray Count: ")
                        .speed(100.0)
                        .range(100..=50_000_000),
                ).changed() {
                    self.light_ray_buffer = SimpleBuffer::init(&self.device, s.ray_count as usize);
                }

                let mut checked = s.tonemapping == 1;
                if ui.checkbox(&mut checked, "Tone mapping").clicked() {
                    s.tonemapping = if checked { 1 } else { 0 };
                }
                ui.add(DragValue::new(&mut s.debug_cutoff).prefix("CUT: ").speed(0.1).range(0.0..=f32::MAX));
            });
        });

        ctx.input(|i| {
            if let Some(p) = i.pointer.latest_pos() {
                let p = p * ppp;

                let mp = mouse_to_clip(p.into(), self.screen_size);
                let aspect = self.screen_size.0 as f32 / self.screen_size.1 as f32;
                let mp_world = vec2(mp.0 * aspect, mp.1);

                s.mouse_pos_clip = mp_world;
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
            &self.polygon.buffer,
        );

        for _ in 0..(self.settings_uniform.data().bounces as usize) {
            self.line_physics_compute_pass.compute_pass(
                encoder,
                &self.light_ray_buffer,
                &self.settings_uniform,
                &self.polygon.buffer,
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
            &self.polygon.buffer,
        );

        self.display_pipeline
            .render(encoder, view, &self.line_texture, &self.settings_uniform);
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
        geometry_buffer: &SimpleBuffer<Vec2>,
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
            bind_group_layouts: &[
                Some(&settings_uniform.bind_group_layout),
                Some(&geometry_buffer.read_only_layout),
            ],
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
        geometry_buffer: &SimpleBuffer<Vec2>,
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
        render_pass.set_bind_group(1, &geometry_buffer.read_only, &[]);
        render_pass.draw(0..(geometry_buffer.size_elements + 1), 0..1);
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
        geometry_buffer: &SimpleBuffer<Vec2>,
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
                Some(&geometry_buffer.read_only_layout),
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
        geometry_buffer: &SimpleBuffer<Vec2>,
    ) {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Compute Pass"),
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(&self.compute_pipeline);
        compute_pass.set_bind_group(0, &light_ray_buffer.read_write, &[]);
        compute_pass.set_bind_group(1, &settings_uniform.bind_group, &[]);
        compute_pass.set_bind_group(2, &geometry_buffer.read_only, &[]);

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
        geometry_buffer: &SimpleBuffer<Vec2>,
    ) {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Init Pass"),
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(&self.init_pipeline);
        compute_pass.set_bind_group(0, &light_ray_buffer.read_write, &[]);
        compute_pass.set_bind_group(1, &settings_uniform.bind_group, &[]);
        compute_pass.set_bind_group(2, &geometry_buffer.read_only, &[]);

        let workgroup_size = 256;
        let total_elements = light_ray_buffer.size_elements;
        let workgroup_count = total_elements.div_ceil(workgroup_size);
        compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
}


pub struct TextureSamplerRenderer {
    pipeline: RenderPipeline,
}
impl TextureSamplerRenderer {
    pub fn init(
        device: &Device,
        texture: &GpuTexture<HasSampler>,
        output_format: TextureFormat,
        settings: &AutoUniform<Settings>,
    ) -> Self {
        // Load the shader from the WGSL file above

        let shader = combine(&device, &[
            include_str!("shaders/display_to_srgb.wgsl"),
            Settings::WGSL(),
        ]);

        // let shader = device.create_shader_module(include_wgsl!("shaders/display_to_srgb.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Texture Sampler Pipeline Layout"),
            bind_group_layouts: &[Some(&texture.bind_group_layout), Some(&settings.bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Texture Sampler Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
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
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format: output_format,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Self { pipeline }
    }

    pub fn render(
        &self,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        input_texture: &GpuTexture<HasSampler>,
        settings: &AutoUniform<Settings>,
    ) {
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Fullscreen Texture Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &input_texture.bind_group, &[]);
        render_pass.set_bind_group(1, &settings.bind_group, &[]);

        // Draw 3 vertices to trigger the fullscreen triangle generation
        render_pass.draw(0..3, 0..1);
    }
}
