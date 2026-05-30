use std::marker::PhantomData;
use bytemuck::{bytes_of, cast_slice, Pod, Zeroable};
use encase::{ShaderType, UniformBuffer};
use encase::internal::WriteInto;
use wgpu::{include_wgsl, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendState, Buffer, BufferBindingType, BufferUsages, ColorTargetState, ColorWrites, CommandEncoder, Device, FragmentState, FrontFace, MultisampleState, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, Queue, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, ShaderStages, TextureFormat, TextureView, VertexState};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::wgt::BufferDescriptor;
use crate::the_code::texture::{GpuTexture, HasSampler};

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


pub struct AutoUniform<T: ShaderType + WriteInto> {
    data: T,
    buffer: Buffer,
    pub bind_group: BindGroup,
    pub bind_group_layout: BindGroupLayout,
}
impl<T: ShaderType + WriteInto> AutoUniform<T> {
    pub fn init(device: &Device, data: T) -> Self {
        let mut cpu_buffer = UniformBuffer::new(Vec::<u8>::new());
        cpu_buffer.write(&data).unwrap();
        let byte_data: Vec<u8> = cpu_buffer.into_inner();


        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Uniform"),
            contents: &byte_data,
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

        let mut cpu_buffer = UniformBuffer::new(Vec::<u8>::new());
        cpu_buffer.write(&self.data).unwrap();
        let byte_data: Vec<u8> = cpu_buffer.into_inner();

        queue.write_buffer(&self.buffer, 0, &byte_data);
    }

    pub fn write_current_data(&mut self, queue: &Queue) {
        let mut cpu_buffer = UniformBuffer::new(Vec::<u8>::new());
        cpu_buffer.write(&self.data).unwrap();
        let byte_data: Vec<u8> = cpu_buffer.into_inner();
        queue.write_buffer(&self.buffer, 0, &byte_data);
    }

    pub fn mod_data(&mut self) -> &mut T {
        &mut self.data
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
        let size_bytes = std::mem::size_of_val(data);
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