use std::fmt;
use std::fmt::Debug;
use bytemuck::Pod;
use bytemuck::Zeroable;
use eframe::egui::Vec2;
use eframe::wgpu;
use eframe::wgpu::util::{BufferInitDescriptor, DeviceExt};
use eframe::wgpu::VertexFormat::Uint32;
use eframe::wgpu::{BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, Buffer, BufferAddress, BufferDescriptor, BufferUsages, Device, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, TextureUsages, VertexAttribute, VertexFormat};
use std::marker::PhantomData;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex(pub [f32; 2]);
impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[VertexAttribute {
                format: VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        }
    }
}

impl From<Vec2> for Vertex {
    fn from(value: Vec2) -> Self {
        Self([value.x, value.y])
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightRay {
    pub position: [f32; 2],           // Offset 0
    pub draw_last_position: [f32; 2], // Offset 8
    pub wavelength: f32,              // Offset 16
    pub strength: f32,                // Offset 20
    pub ray_status: u32,              // Offset 24
    pub _pad1: u32,                   // Offset 28 <--- ADD THIS
    pub direction: [f32; 2],          // Offset 32 (Now matches WGSL alignment)
    pub current_ior: f32,             // Offset 40
    pub _pad2: u32,                   // Offset 44 <--- ADD THIS to make total 48
}

#[test]
fn test() {
    println!("{}", size_of::<LightRay>());
}

impl LightRay {
    // only position, draw last pos, wavelength, and strength
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<LightRay>() as BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // position
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x2,
                },
                // draw_last_position
                VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: VertexFormat::Float32x2,
                },
                // wavelength
                VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: VertexFormat::Float32,
                },
                // strength
                VertexAttribute {
                    offset: 20,
                    shader_location: 3,
                    format: VertexFormat::Float32,
                },
                // status
                VertexAttribute {
                    offset: 24,
                    shader_location: 4,
                    format: Uint32,
                }, // rest are of nil use
            ],
        }
    }
}


pub struct BigBufPack {
    pub buffer: Buffer,
    pub size_elements: u64,
    pub size_bytes: u64,
    pub read_only: BindGroup,
    pub read_write: BindGroup,
}

impl Debug for BigBufPack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {

        #[derive(Debug)]
        struct BufDebug {
            size_elements: u64,
            size_bytes: u64,
        }

        let s = BufDebug {
            size_elements: self.size_elements,
            size_bytes: self.size_bytes,
        };

        fmt::Debug::fmt(&s, f)
    }
}

// big storage buffer
pub struct BigBuffer<Data: Pod + Zeroable> {
    pub max_buffer_size_elements: usize,
    data_type: PhantomData<Data>,
    // buffer and len
    buffers: Vec<BigBufPack>,

    read_only_layout: BindGroupLayout,
    read_write_layout: BindGroupLayout,
}
impl<Data: Pod + Zeroable> BigBuffer<Data> {
    pub fn init_storage_with_data(device: &Device, data: &[Data]) -> Self {
        let max_size_bytes = device.limits().max_buffer_size as usize;
        // let max_size_bytes = size_of::<Data>() * 100_000;
        let max_size_elements = max_size_bytes / size_of::<Data>();

        let mut buffers = vec![];

        let num_inter = data.len() / max_size_elements;
        let excess = data.len() % max_size_elements;

        for i in 0..num_inter {
            let st = i * max_size_elements;
            let end = st + max_size_elements;
            println!("{st}..{end}");
            buffers.push((
                device.create_buffer_init(&BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&data[st..end]),
                    usage: BufferUsages::STORAGE | BufferUsages::VERTEX,
                }),
                max_size_elements,
            ));
        }

        if excess > 0 {
            let st = num_inter * max_size_elements;
            let end = st + excess;
            println!("Ex. {st}..{end}");
            buffers.push((
                device.create_buffer_init(&BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&data[st..end]),
                    usage: BufferUsages::STORAGE | BufferUsages::VERTEX,
                }),
                excess,
            ));
        }

        let (bb, write, read) = Self::create_bind(buffers, device);

        Self {
            max_buffer_size_elements: max_size_elements,
            data_type: PhantomData,
            buffers: bb,
            read_only_layout: read,
            read_write_layout: write,
        }
    }

    pub fn create_storage_empty(device: &Device, size: usize) -> Self {
        let max_size_bytes = device.limits().max_buffer_size as usize;
        let max_size_elements = max_size_bytes / size_of::<Data>();

        let mut buffers = vec![];

        let num_inter = size / max_size_elements;
        let excess = size % max_size_elements;

        for _ in 0..num_inter {
            buffers.push((
                device.create_buffer(&BufferDescriptor {
                    label: None,
                    size: (max_size_elements * size_of::<Data>()) as u64,
                    usage: BufferUsages::STORAGE | BufferUsages::VERTEX,
                    mapped_at_creation: false,
                }),
                max_size_elements,
            ));
        }

        if excess > 0 {
            buffers.push((
                device.create_buffer(&BufferDescriptor {
                    label: None,
                    size: (excess * size_of::<Data>()) as u64,
                    usage: BufferUsages::STORAGE | BufferUsages::VERTEX,
                    mapped_at_creation: false,
                }),
                excess,
            ));
        }

        let (bb, write, read) = Self::create_bind(buffers, device);

        Self {
            max_buffer_size_elements: max_size_elements,
            data_type: PhantomData,
            buffers: bb,
            read_only_layout: read,
            read_write_layout: write,
        }
    }

    pub fn create_bind(
        buffers: Vec<(Buffer, usize)>,
        device: &Device,
    ) -> (Vec<BigBufPack>, BindGroupLayout, BindGroupLayout) {
        let read_write_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT | ShaderStages::COMPUTE,
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

        let bb = buffers
            .into_iter()
            .map(|(b, l)| {
                let read_only = device.create_bind_group(&BindGroupDescriptor {
                    label: None,
                    layout: &read_only_layout,
                    entries: &[BindGroupEntry {
                        binding: 0,
                        resource: b.as_entire_binding(),
                    }],
                });

                let read_write = device.create_bind_group(&BindGroupDescriptor {
                    label: None,
                    layout: &read_write_layout,
                    entries: &[BindGroupEntry {
                        binding: 0,
                        resource: b.as_entire_binding(),
                    }],
                });

                BigBufPack {
                    buffer: b,
                    size_elements: l as u64,
                    size_bytes: (l * size_of::<Data>()) as u64,
                    read_only,
                    read_write,
                }
            })
            .collect::<Vec<BigBufPack>>();

        (bb, read_write_layout, read_only_layout)
    }

    pub fn buffers(&self) -> &[BigBufPack] {
        &self.buffers
    }

    pub fn read_only(&self) -> &BindGroupLayout {
        &self.read_only_layout
    }

    pub fn read_write(&self) -> &BindGroupLayout {
        &self.read_write_layout
    }
}

pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: Option<wgpu::Sampler>,
    pub bind_group_layout: BindGroupLayout,
    pub bind_group: BindGroup,
    format: wgpu::TextureFormat,
    usage: TextureUsages,
}
impl GpuTexture {
    pub fn new(
        device: &Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        label: &str,
        sampler_desc: Option<SamplerDescriptor>,
        is_sampler_filtering: bool,
    ) -> Self {
        let sampler = sampler_desc.map(|f| device.create_sampler(&f) );

        // 2. Create the Layout (Lives forever)
        let mut e = vec![wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: is_sampler_filtering },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }];
        if let Some(_) = sampler {
            e.push(wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Sampler(if is_sampler_filtering {
                    SamplerBindingType::Filtering
                } else {
                    SamplerBindingType::NonFiltering
                }),
                count: None,
            });
        }
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("{}_layout", label)),
            entries: &e,
        });

        // 3. Create the initial texture resources
        let (texture, view, bind_group) = Self::create_resources(
            device,
            &bind_group_layout,
            &sampler,
            width,
            height,
            format,
            usage,
            label,
        );

        Self {
            texture,
            view,
            sampler,
            bind_group_layout,
            bind_group,
            format,
            usage,
        }
    }

    /// Internal helper to recreate the volatile parts of the texture
    fn create_resources(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &Option<Sampler>,
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

        let mut e = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&view),
        }];
        if let Some(s) = sampler {
            e.push(wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(s),
            });
        }
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &e,
            label: Some(&format!("{}_bind_group", label)),
        });

        (texture, view, bind_group)
    }

    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) {
        // We reuse the existing layout and sampler, but replace the rest
        let (texture, view, bind_group) = Self::create_resources(
            device,
            &self.bind_group_layout,
            &self.sampler,
            width,
            height,
            self.format,
            self.usage,
            "resized_texture",
        );

        self.texture = texture;
        self.view = view;
        self.bind_group = bind_group;
    }
}
