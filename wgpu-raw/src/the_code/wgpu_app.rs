#![allow(unused_variables)]

use crate::the_code::texture::{GpuTexture, GpuTextureBuilder, HasSampler};
use crate::the_code::wgpu_app::sample_render::TextureSamplerRenderer;
use crate::the_code::StartupInfo;
use bytemuck::{bytes_of, cast, cast_slice, Pod, Zeroable};
use glam::{vec2, Vec2};
use rand::{random, rng, RngExt};
use std::f32::consts::PI;
use std::marker::PhantomData;
use std::time::Instant;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::wgt::{BufferDescriptor, PollType, SamplerDescriptor};
use wgpu::{
    include_wgsl, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendComponent, BlendFactor,
    BlendOperation, BlendState, Buffer, BufferAddress, BufferBindingType, BufferUsages, Color,
    ColorTargetState, ColorWrites, CommandEncoder, ComputePipeline, ComputePipelineDescriptor,
    Device, FilterMode, FragmentState, FrontFace, LoadOpDontCare, MultisampleState,
    PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, Queue,
    RenderPipeline, RenderPipelineDescriptor, ShaderModule, ShaderModuleDescriptor, ShaderSource,
    ShaderStages, TextureFormat, TextureUsages, TextureView, VertexState,
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightRay {
    pub pos: [f32; 2],
    pub last_pos: [f32; 2],
    pub dir: [f32; 2],
    pub strength: f32,
    pub wave_length_and_ior: u32, // Combine u16s into u32 for easier alignment
}
impl LightRay {
    fn u16_wave(wave: f32) -> u16 {
        let clamped = wave.clamp(381.0, 779.0);
        let normalized = (clamped - 350.0) / 350.0;
        (normalized * 65535.0).round() as u16
    }
}

#[test]
fn t() {
    println!("{}", size_of::<LightRay>())
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Settings {
    pub timestamp: f32,
    pub aspect: f32,
    pub mouse_pos_clip: [f32; 2],
    pub ray_count: u32,
    pub ray_light_scale: f32,
    pub a_factor: f32,
    pub b_factor: f32,
    pub brightness_scale: f32,
    pub spread: f32,
    pub width: f32,

    pub light_dir: f32, // dir in radians
    pub light_position: [f32; 2],
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            timestamp: 0.0,
            aspect: 1.0,
            mouse_pos_clip: [0.0; 2],
            ray_count: RAY_COUNT as u32,
            ray_light_scale: 100_000.0,
            a_factor: 1.517,
            b_factor: 0.0042,
            brightness_scale: 0.15,
            spread: 0.0,
            width: 0.0005,
            light_dir: 0.1,
            light_position: [-2.0, 0.0],
        }
    }
}

const RAY_COUNT: usize = 50_000;
const ITERATIONS: usize = 32;

pub struct WgpuApplication {
    device: Device,
    queue: Queue,

    light_ray_buffer: SimpleBuffer<LightRay>,
    line_texture: GpuTexture<HasSampler>,
    line_primitive_renderer: LinePrimitiveRenderer,

    display_pipeline: TextureSamplerRenderer,

    line_physics_compute_pass: LinePhysicsComputePass,

    start_instant: Instant,
    settings_uniform: SimpleUniform<Settings>,

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
        let settings_uniform = SimpleUniform::init(
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
        // let mut encoder = self
        //     .device
        //     .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            // clear texture
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

        // let command_buffer = encoder.finish();
        // self.queue.submit(Some(command_buffer));
        // self.device
        //     .poll(PollType::Wait {
        //         submission_index: None,
        //         timeout: None,
        //     })
        //     .unwrap();
    }
}

pub struct SimpleBuffer<T: Pod + Zeroable> {
    data_type: PhantomData<T>,
    pub buffer: Buffer,
    pub size_elements: u32,
    pub size_bytes: u32,

    pub read_only_layout: BindGroupLayout,
    pub read_write_layout: BindGroupLayout,
    pub read_only: BindGroup,
    pub read_write: BindGroup,
}
impl<T: Pod + Zeroable> SimpleBuffer<T> {
    pub fn init_with(data: &[T], device: &Device) -> Self {
        let size_bytes = size_of::<T>() * data.len();
        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: cast_slice(data),
            usage: BufferUsages::STORAGE | BufferUsages::VERTEX,
        });

        let (read_write_layout, read_only_layout, read_write, read_only) =
            Self::layouts(device, &buffer);

        Self {
            data_type: Default::default(),
            buffer,
            size_elements: data.len() as u32,
            size_bytes: size_bytes as u32,
            read_only_layout,
            read_write_layout,
            read_only,
            read_write,
        }
    }

    pub fn init(device: &Device, size: usize) -> Self {
        let buffer = device.create_buffer(&BufferDescriptor {
            label: None,
            size: size as u64,
            usage: BufferUsages::VERTEX | BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let (read_write_layout, read_only_layout, read_write, read_only) =
            Self::layouts(device, &buffer);

        Self {
            data_type: Default::default(),
            buffer,
            size_elements: size as u32,
            size_bytes: (size * size_of::<T>()) as u32,
            read_only_layout,
            read_write_layout,
            read_only,
            read_write,
        }
    }
    pub fn layouts(
        device: &Device,
        buffer: &Buffer,
    ) -> (BindGroupLayout, BindGroupLayout, BindGroup, BindGroup) {
        let read_write_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let read_only_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT | ShaderStages::VERTEX | ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let read_only = device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &read_only_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        let read_write = device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &read_write_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        (read_write_layout, read_only_layout, read_write, read_only)
    }
}

pub struct SimpleUniform<T: Pod + Zeroable> {
    data: T,
    buffer: Buffer,
    pub bind_group: BindGroup,
    pub bind_group_layout: BindGroupLayout,
}
impl<T: Pod + Zeroable> SimpleUniform<T> {
    pub fn init(device: &Device, data: T) -> Self {
        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Uniform"),
            contents: bytes_of(&data),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT | ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            data,
            buffer,
            bind_group,
            bind_group_layout,
        }
    }

    pub fn edit<F: FnMut(&mut T)>(&mut self, queue: &Queue, mut f: F) {
        f(&mut self.data);
        queue.write_buffer(&self.buffer, 0, bytes_of(&self.data));
    }

    pub fn write_current_data(&mut self, queue: &Queue) {
        queue.write_buffer(&self.buffer, 0, bytes_of(&self.data));
    }

    pub fn mod_data(&mut self) -> &mut T {
        &mut self.data
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
        settings_uniform: &SimpleUniform<Settings>,
    ) -> Self {
        let line_shader_module = combine(
            &device,
            &[
                include_str!("shaders/draw_lines.wgsl"),
                include_str!("shaders/definitions.wgsl"),
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
            &device,
            &[
                include_str!("shaders/draw_geometry.wgsl"),
                include_str!("shaders/definitions.wgsl"),
                include_str!("shaders/collision.wgsl"),
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
        settings_uniform: &SimpleUniform<Settings>,
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
        settings_uniform: &SimpleUniform<Settings>,
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
        fin.push_str(*s)
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
        settings_uniform: &SimpleUniform<Settings>,
    ) -> Self {
        let shader_module = combine(
            &device,
            &[
                include_str!("shaders/compute_pass.wgsl"),
                include_str!("shaders/collision.wgsl"),
                include_str!("shaders/definitions.wgsl"),
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
        settings_uniform: &SimpleUniform<Settings>,
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
        let workgroup_count = (total_elements + workgroup_size - 1) / workgroup_size;
        compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
    }

    pub fn init_pass(
        &self,
        encoder: &mut CommandEncoder,
        light_ray_buffer: &SimpleBuffer<LightRay>,
        settings_uniform: &SimpleUniform<Settings>,
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
        let workgroup_count = (total_elements + workgroup_size - 1) / workgroup_size;
        compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
}

pub mod sample_render {
    use crate::the_code::texture::{GpuTexture, HasSampler};
    use wgpu::*;

    pub struct TextureSamplerRenderer {
        pipeline: RenderPipeline,
    }

    impl TextureSamplerRenderer {
        pub fn init(
            device: &Device,
            texture: &GpuTexture<HasSampler>,
            output_format: TextureFormat,
        ) -> Self {
            // Load the shader from the WGSL file above
            let shader = device.create_shader_module(include_wgsl!("shaders/display_to_srgb.wgsl"));

            let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Texture Sampler Pipeline Layout"),
                bind_group_layouts: &[Some(&texture.bind_group_layout)],
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

            // Draw 3 vertices to trigger the fullscreen triangle generation
            render_pass.draw(0..3, 0..1);
        }
    }
}
