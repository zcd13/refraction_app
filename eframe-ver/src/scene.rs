use crate::geometry::Polygon;
use eframe::wgpu;
use eframe::wgpu::{include_wgsl, BindGroup, BindGroupDescriptor, BindGroupEntry, Buffer, BufferUsages, RenderPipeline, ShaderModuleDescriptor};
use std::sync::Arc;
use eframe::wgpu::util::{BufferInitDescriptor, DeviceExt};

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
    texture_view: Option<wgpu::TextureView>,
    pipeline: RenderPipeline,
    poly_buffer: Buffer,
    poly_bind: BindGroup
}

impl Scene {
    pub fn init(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        println!("Reached");
        let texture_desc = wgpu::TextureDescriptor {
            label: Some("Background Render Target"),
            size: wgpu::Extent3d {
                width: 500,
                height: 500,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = device.create_texture(&texture_desc);
        let texture_view = Some(texture.create_view(&wgpu::TextureViewDescriptor::default()));

        let mut geometry = Polygon::tri();
        let ver = geometry.cast_worldspace();

        let poly_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Polygon buffer"),
            contents: bytemuck::cast_slice(ver),
            usage: wgpu::BufferUsages::VERTEX | BufferUsages::STORAGE,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT, // Make it visible to Fragment
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
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
                resource: poly_buffer.as_entire_binding(),
            }],
        });

        let shader_module = device.create_shader_module(include_wgsl!("../shaders/frag.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Fullscreen Layout"),
            bind_group_layouts: (&[Some(&bind_group_layout)]),
            immediate_size: 0,
        });

        let render = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Fullscreen Storage Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[], // No vertex buffers used!
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: texture.format(),
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // No culling needed for fullscreen
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            cache: None,
            multiview_mask: None,
        });

        println!("Made");
        Self {
            device,
            queue,
            current_size: (500, 500),
            texture_view,
            pipeline: render,
            poly_bind: bind_group,
            poly_buffer,
        }
    }

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

    pub fn render(&self) -> Option<&wgpu::TextureView> {
        let view = self.texture_view.as_ref()?;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            // 3. Begin the Render Pass
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            // 4. Bind the pipeline and our storage data
            render_pass.set_pipeline(&self.pipeline);

            // This is where your 'vertex array' (storage buffer) enters the shader
            render_pass.set_bind_group(0, &self.poly_bind, &[]);

            // 5. Draw 3 vertices for the fullscreen triangle
            render_pass.draw(0..3, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));

        Some(view)
    }
}

