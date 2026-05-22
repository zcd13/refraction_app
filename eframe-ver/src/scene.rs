use std::f32::consts::PI;
use eframe::wgpu;
use eframe::wgpu::wgt::{PollType, SamplerDescriptor};
use eframe::wgpu::{include_wgsl, BindGroup, BindGroupLayout, BlendComponent, BlendFactor, BlendOperation, BlendState, ColorTargetState, ColorWrites, CommandEncoder, Device, FragmentState, FrontFace, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPipeline, RenderPipelineDescriptor, Texture, TextureDimension, TextureFormat, TextureUsages, TextureView, VertexState};
use std::sync::Arc;
use std::time::Instant;
use crate::wgpu_res::{BigBuffer, GpuTexture, LightRay};

pub struct TestSceneRenderer {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    current_size: (u32, u32),
    texture_view: Option<wgpu::TextureView>,
    render_pipeline: wgpu::RenderPipeline,
}
impl TestSceneRenderer {
    pub fn init(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        // 1. A basic WGSL shader that generates 3 vertices and colors them red
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Debug Triangle Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(
                r#"
                @vertex
                fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> @builtin(position) vec4<f32> {
                    var pos = array<vec2<f32>, 3>(
                        vec2<f32>(0.0, 1.),   // Top
                        vec2<f32>(-1., -1.), // Bottom Left
                        vec2<f32>(1., -1.)   // Bottom Right
                    );
                    return vec4<f32>(pos[in_vertex_index], 0.0, 1.0);
                }

                @fragment
                fn fs_main() -> @location(0) vec4<f32> {
                    return vec4<f32>(1.0, 0.0, 0.0, 1.0); // Red color
                }
                "#
            )),
        });

        // 2. Create the pipeline layout (empty because we have no uniforms/bind groups yet)
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });

        // 3. Create the actual render pipeline
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[], // Empty because vertices are hardcoded in the shader
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                // This MUST match the format of the texture we create in `resize`
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            cache: None,
        });

        Self {
            device,
            queue,
            current_size: (0, 0),
            texture_view: None,
            render_pipeline,
        }
    }

    /// Resizes the internal WGPU texture if the dimensions have changed
    pub fn resize(&mut self, width: u32, height: u32) {
        if self.current_size == (width, height) {
            return;
        }
        self.current_size = (width, height);

        if width == 0 || height == 0 {
            self.texture_view = None;
            return;
        }

        let texture_desc = wgpu::TextureDescriptor {
            label: Some("Background Render Target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };

        let texture = self.device.create_texture(&texture_desc);
        self.texture_view = Some(texture.create_view(&wgpu::TextureViewDescriptor::default()));
    }

    /// Executes the WGPU render pass and returns the view to be registered with egui
    pub fn render(&self) -> Option<&wgpu::TextureView> {
        let view = self.texture_view.as_ref()?;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            // CHANGED: made `render_pass` mutable so we can call methods on it
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Custom Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Dark gray background
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // ADDED: Tell WGPU to use our pipeline and draw 3 vertices
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.draw(0..3, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));

        Some(view)
    }
}

pub struct Scene {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    current_size: (u32, u32),
    display_texture: DisplayTexture,

    // target for drawing lines, format = float32x4, same size as render texture
    line_draw_texture: GpuTexture,
    line_render_pipeline: RenderPipeline,

    ray_buffer: BigBuffer<LightRay>,

    last_frame_time: Instant,
    pub current_fps: f32,
}

const INIT_SIZE: [u32; 2] = [500; 2];
impl Scene {
    pub fn init(device: Arc<Device>, queue: Arc<wgpu::Queue>) -> Self {
        println!("Reached");

        let draw_texture = GpuTexture::new(
            &device,
            INIT_SIZE[0],
            INIT_SIZE[1],
            TextureFormat::Rgba32Float,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            "draw_texture",
            Some(SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            }),
            false,
        );

        let debug_ray_count = 50_000_000;
        let mut data: Vec<LightRay> = Vec::with_capacity(debug_ray_count);
        for i in 0..debug_ray_count {
            let f = i as f32 / debug_ray_count as f32;
            let dir =  PI * 2.0 * f;
            let dir_v = [dir.cos(), dir.sin()];
            let w = (700.0 - 380.0) * f + 380.0;

            let x = (f - 0.5) * 2.0;
            data.push(LightRay {
                position: [x, -1.0],
                draw_last_position: [x, 1.0],
                // draw_last_position: [dir_v[0] * 100.0, dir_v[1] * 100.0],
                wavelength: w,
                strength: 1.0,
                ray_status: 0,
                _pad1: 0,
                direction: [0.0; 2],
                current_ior: 0.0,
                _pad2: 0,
            })
        }
        let debug_rays = BigBuffer::init_storage_with_data(&device, &data);

        let shader_module = device.create_shader_module(include_wgsl!("../shaders/draw_lines.wgsl"));

        let line_render_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("line_render_pipeline_layout"),
            bind_group_layouts: &[Some(debug_rays.read_only())],
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
            multisample: Default::default(),
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[
                    Some(ColorTargetState {
                        format: TextureFormat::Rgba32Float,
                        blend: None,
                        // blend: Some(BlendState {
                        //     color: BlendComponent {
                        //         src_factor: BlendFactor::One,
                        //         dst_factor: BlendFactor::One,
                        //         operation: BlendOperation::Add,
                        //     },
                        //     alpha: BlendComponent {
                        //         src_factor: BlendFactor::One,
                        //         dst_factor: BlendFactor::One,
                        //         operation: BlendOperation::Add,
                        //     },
                        // }),
                        write_mask: ColorWrites::ALL,
                    })
                ],
            }),
            multiview_mask: None,
            cache: None,
        });

        let display_texture = DisplayTexture::new(&device, &draw_texture.bind_group_layout);


        println!("Made");
        Self {
            device,
            queue,
            current_size: (500, 500),
            display_texture,
            line_draw_texture: draw_texture,
            line_render_pipeline,
            ray_buffer: debug_rays,


            last_frame_time: Instant::now(),
            current_fps: 0.0,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if self.current_size == (width, height) {
            return;
        }
        self.current_size = (width, height);

        if width == 0 || height == 0 {
            return;
        }

        self.display_texture.resize(width, height, &self.device);
        self.line_draw_texture.resize(&self.device, width, height);
    }

    pub fn render(&mut self) -> Option<&TextureView> {
        self.update_fps();

        println!("{:.2} FPS {:?} buffs elements = {}", self.current_fps, self.ray_buffer.buffers(), self.ray_buffer.max_buffer_size_elements);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let tex_to_display = {

            // draw lines
            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Line Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.line_draw_texture.view,
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


                render_pass.set_pipeline(&self.line_render_pipeline);

                for buffer in self.ray_buffer.buffers() {
                    let num_rays = buffer.size_elements as u32;
                    render_pass.set_bind_group(0, &buffer.read_only, &[]);
                    render_pass.draw(0..2, 0..num_rays);
                }

            }

            self.display_texture.render(&mut encoder, &self.line_draw_texture.bind_group)
        };
        let command_buffer = encoder.finish();
        self.queue.submit(Some(command_buffer));
        self.device.poll(PollType::Wait { submission_index: None, timeout: None }).unwrap();


        Some(tex_to_display)
    }

    pub fn update_fps(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;

        // Apply a simple exponential moving average to smooth the number
        // 0.1 means it takes ~10 frames to fully react to a speed change
        if dt > 0.0 {
            let instant_fps = 1.0 / dt;
            self.current_fps = self.current_fps * 0.9 + instant_fps * 0.1;
        }
    }
}


struct DisplayTexture {
    display_tex_view: wgpu::TextureView,
    pipeline: RenderPipeline,
}
impl DisplayTexture {
    pub fn new(device: &Device, float_tex_bind_layout: &BindGroupLayout) -> Self {
        let texture = Self::create_render_tex(INIT_SIZE[0], INIT_SIZE[1], device);
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let shader_module = device.create_shader_module(include_wgsl!("../shaders/display_to_srgb.wgsl"));

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Tone map Pipeline"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[Some(float_tex_bind_layout)],
                ..Default::default()
            })),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[], // No vertex buffer needed due to the triangle trick
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb, // Your target format
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            cache: None,
        });

        Self {
            display_tex_view: texture_view,
            pipeline,
        }
    }

    pub fn create_render_tex(w: u32, h: u32, device: &Device) -> Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Background Render Target"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    pub fn resize(&mut self, w: u32, h: u32, device: &Device) {
        let texture = Self::create_render_tex(w, h, device);
        self.display_tex_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    }

    pub fn render(&self, encoder: &mut CommandEncoder, float_tex_bind: &BindGroup) -> &TextureView {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.display_tex_view, // The 8-bit target
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, float_tex_bind, &[]);
        rpass.draw(0..3, 0..1); // Draws the full-screen triangle

        &self.display_tex_view
    }
}
