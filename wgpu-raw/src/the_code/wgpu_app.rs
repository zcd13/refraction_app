#![allow(unused_variables)]

use crate::the_code::wgpu_app::sample_render::TextureSamplerRenderer;
use crate::the_code::wgpu_app::texture::{GpuTexture, GpuTextureBuilder, HasSampler};
use crate::the_code::StartupInfo;
use bytemuck::{bytes_of, cast, cast_slice, Pod, Zeroable};
use glam::{vec2, Vec2};
use rand::{random, rng, RngExt};
use std::f32::consts::PI;
use std::marker::PhantomData;
use std::time::Instant;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::wgt::{PollType, SamplerDescriptor};
use wgpu::{
    include_wgsl, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendComponent, BlendFactor,
    BlendOperation, BlendState, Buffer, BufferBindingType, BufferUsages, Color, ColorTargetState,
    ColorWrites, CommandEncoder, ComputePipeline, ComputePipelineDescriptor, Device, FilterMode,
    FragmentState, FrontFace, LoadOpDontCare, MultisampleState, PipelineLayoutDescriptor,
    PolygonMode, PrimitiveState, PrimitiveTopology, Queue, RenderPipeline,
    RenderPipelineDescriptor, ShaderModule, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    TextureFormat, TextureUsages, TextureView, VertexState,
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
    pub width: u32,
    pub height: u32,
    pub aspect: f32, // replaces _padding1, same offset (12)
    pub mouse_pos_clip: [f32; 2],
    pub _padding2: [u32; 2],
}

fn init_rays(mp: [f32; 2]) -> Vec<LightRay> {
    let debug_ray_count = 8000;

    // println!(
    //     "Ray buffer size = {} with {debug_ray_count} rays",
    //     format_bytes(debug_ray_count * size_of::<LightRay>())
    // );

    let mut data: Vec<LightRay> = Vec::with_capacity(debug_ray_count);
    for i in 0..debug_ray_count {
        let f = i as f32 / debug_ray_count as f32;
        let x = (f - 0.5) * 2.0;

        let v_mp = Vec2::from(mp);
        let v_p = Vec2::new(-1.0, 0.5);
        let dir_t = (v_mp - v_p).normalize();

        let mut dir_rad = dir_t.x.atan2(dir_t.y);
        dir_rad += x * 0.2;
        let dir_t = vec2(dir_rad.sin(), dir_rad.cos());

        let left = Vec2::new(-dir_t.y, dir_t.x);
        let right = Vec2::new(dir_t.y, -dir_t.x);
        let diff = left - right;
        let aj = (diff * 0.0005) * x;

        let np = v_p + aj;

        // println!("{dir_v:?}");

        let cf: f32 = rng().random_range(0.0..1.0);
        let w = (700.0 - 380.0) * cf + 380.0;


        data.push(LightRay {
            pos: np.into(),
            last_pos: [0.0; 2],
            dir: dir_t.into(),
            strength: 0.1,
            wave_length_and_ior: LightRay::u16_wave(w) as u32,
            // wavelength: LightRay::u16_wave(650.0),
            // current_ior_index: 0,
        })
    }
    data
}

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
        let data = init_rays([1.0, 0.0]);

        let light_ray_buffer = SimpleBuffer::init_with(&data, &device);

        let line_texture = GpuTextureBuilder::new(
            &device,
            (2000, 2000),
            TextureFormat::Rgba16Float,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        )
        .with_sampler(
            SamplerDescriptor {
                mag_filter: FilterMode::Linear,
                min_filter: FilterMode::Linear,
                ..Default::default()
            },
            true,
        )
        .make();

        let start_instant = Instant::now();
        let settings_uniform = SimpleUniform::init(
            &device,
            Settings {
                timestamp: 0.0,
                width: startup_info.display_size.0,
                height: startup_info.display_size.1,
                aspect: startup_info.display_size.0 as f32 / startup_info.display_size.1 as f32,
                mouse_pos_clip: [0.0; 2],
                _padding2: [0; 2],
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

        let screen_size = startup_info.display_size;

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
            s.width = viewport_size.0;
            s.height = viewport_size.1;
            s.aspect = viewport_size.0 as f32 / viewport_size.1 as f32;
        });
    }

    pub fn update(&mut self, mouse_pos: (f32, f32)) {
        let mp = mouse_to_clip(mouse_pos, self.screen_size);
        let aspect = self.screen_size.0 as f32 / self.screen_size.1 as f32;
        let mp_world = (mp.0 * aspect, mp.1);

        let data = init_rays([mp_world.0, mp_world.1]);
        self.light_ray_buffer = SimpleBuffer::init_with(&data, &self.device);

        self.settings_uniform.edit(&self.queue, |s| {
            s.timestamp = self.start_instant.elapsed().as_secs_f32();
            s.mouse_pos_clip = [mp.0, mp.1];
        });
    }

    pub fn render(&mut self, view: &TextureView) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

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

            for _ in 0..32 {
                self.line_physics_compute_pass.compute_pass(
                    &mut encoder,
                    &self.light_ray_buffer,
                    &self.settings_uniform,
                );
                self.line_primitive_renderer.render(
                    &mut encoder,
                    &self.line_texture.view,
                    &self.light_ray_buffer,
                    &self.settings_uniform,
                );
            }

            self.display_pipeline
                .render(&mut encoder, view, &self.line_texture);
        }

        let command_buffer = encoder.finish();
        self.queue.submit(Some(command_buffer));
        self.device
            .poll(PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .unwrap();
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
            contents: bytemuck::cast_slice(data),
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
}

pub struct LinePrimitiveRenderer {
    pipeline: RenderPipeline,
}
impl LinePrimitiveRenderer {
    pub fn init(
        device: &Device,
        rays_buffer: &SimpleBuffer<LightRay>,
        output_format: TextureFormat,
        settings_uniform: &SimpleUniform<Settings>,
    ) -> Self {
        let shader_module = combine(
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
                module: &shader_module,
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
                module: &shader_module,
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
            pipeline: line_render_pipeline,
        }
    }

    pub fn render(
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

        render_pass.set_pipeline(&self.pipeline);

        render_pass.set_bind_group(0, &ray_buffer.read_only, &[]);
        render_pass.set_bind_group(1, &settings_uniform.bind_group, &[]);
        render_pass.draw(0..2, 0..ray_buffer.size_elements);
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

        Self { compute_pipeline }
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
}

pub mod texture {
    pub struct NoSampler;
    pub struct HasSampler {
        pub sampler: wgpu::Sampler,
        pub is_filtering: bool,
    }

    // Typestate markers for the builder
    pub struct BuilderNoSampler;
    pub struct BuilderHasSampler<'a> {
        pub desc: wgpu::SamplerDescriptor<'a>,
        pub is_filtering: bool,
    }

    pub struct GpuTexture<S> {
        pub texture: wgpu::Texture,
        pub view: wgpu::TextureView,
        pub sampler_state: S, // This will be either NoSampler or HasSampler
        pub bind_group_layout: wgpu::BindGroupLayout,
        pub bind_group: wgpu::BindGroup,
        pub format: wgpu::TextureFormat,
        pub usage: wgpu::TextureUsages,
    }

    pub struct GpuTextureBuilder<'a, S> {
        device: &'a wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        label: &'a str,
        sampler_info: S,
    }

    impl<'a> GpuTextureBuilder<'a, BuilderNoSampler> {
        /// Start building a new texture. Defaults to NoSampler.
        pub fn new(
            device: &'a wgpu::Device,
            wh: (u32, u32),
            format: wgpu::TextureFormat,
            usage: wgpu::TextureUsages,
        ) -> Self {
            Self {
                device,
                width: wh.0,
                height: wh.1,
                format,
                usage,
                label: "gpu_texture",
                sampler_info: BuilderNoSampler,
            }
        }

        pub fn with_label(mut self, label: &'a str) -> Self {
            self.label = label;
            self
        }

        /// Consumes the current builder and returns a NEW builder with the HasSampler type.
        pub fn with_sampler(
            self,
            desc: wgpu::SamplerDescriptor<'a>,
            is_filtering: bool,
        ) -> GpuTextureBuilder<'a, BuilderHasSampler<'a>> {
            GpuTextureBuilder {
                device: self.device,
                width: self.width,
                height: self.height,
                format: self.format,
                usage: self.usage,
                label: self.label,
                sampler_info: BuilderHasSampler { desc, is_filtering },
            }
        }

        /// Builds a texture WITHOUT a sampler
        pub fn make(self) -> GpuTexture<NoSampler> {
            let bind_group_layout =
                self.device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some(&format!("{}_layout", self.label)),
                        entries: &[wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        }],
                    });

            let (texture, view, bind_group) = create_resources(
                self.device,
                &bind_group_layout,
                None,
                self.width,
                self.height,
                self.format,
                self.usage,
                self.label,
            );

            GpuTexture {
                texture,
                view,
                bind_group_layout,
                bind_group,
                format: self.format,
                usage: self.usage,
                sampler_state: NoSampler,
            }
        }
    }

    impl<'a> GpuTextureBuilder<'a, BuilderHasSampler<'a>> {
        pub fn with_label(mut self, label: &'a str) -> Self {
            self.label = label;
            self
        }

        /// Builds a texture WITH a sampler
        pub fn make(self) -> GpuTexture<HasSampler> {
            let sampler = self.device.create_sampler(&self.sampler_info.desc);

            let bind_group_layout =
                self.device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some(&format!("{}_layout", self.label)),
                        entries: &[
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: self.sampler_info.is_filtering,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Sampler(if self.sampler_info.is_filtering {
                                    wgpu::SamplerBindingType::Filtering
                                } else {
                                    wgpu::SamplerBindingType::NonFiltering
                                }),
                                count: None,
                            },
                        ],
                    });

            let (texture, view, bind_group) = create_resources(
                self.device,
                &bind_group_layout,
                Some(&sampler),
                self.width,
                self.height,
                self.format,
                self.usage,
                self.label,
            );

            GpuTexture {
                texture,
                view,
                bind_group_layout,
                bind_group,
                format: self.format,
                usage: self.usage,
                sampler_state: HasSampler {
                    sampler,
                    is_filtering: self.sampler_info.is_filtering,
                },
            }
        }
    }

    // Shared internal function
    fn create_resources(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: Option<&wgpu::Sampler>,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        label: &str,
    ) -> (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&view),
        }];

        if let Some(s) = sampler {
            entries.push(wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(s),
            });
        }

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &entries,
            label: Some(&format!("{}_bind_group", label)),
        });

        (texture, view, bind_group)
    }

    // Type-safe resizing
    impl GpuTexture<NoSampler> {
        pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
            let (t, v, bg) = create_resources(
                device,
                &self.bind_group_layout,
                None,
                width,
                height,
                self.format,
                self.usage,
                "resized",
            );
            self.texture = t;
            self.view = v;
            self.bind_group = bg;
        }
    }

    impl GpuTexture<HasSampler> {
        pub fn resize(&mut self, device: &wgpu::Device, wh: (u32, u32)) {
            let (t, v, bg) = create_resources(
                device,
                &self.bind_group_layout,
                Some(&self.sampler_state.sampler),
                wh.0,
                wh.1,
                self.format,
                self.usage,
                "resized",
            );
            self.texture = t;
            self.view = v;
            self.bind_group = bg;
        }
    }
}

pub mod sample_render {
    use crate::the_code::wgpu_app::texture::{GpuTexture, HasSampler};
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
