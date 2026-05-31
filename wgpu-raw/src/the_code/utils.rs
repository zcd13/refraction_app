use bytemuck::{cast_slice, Pod, Zeroable};
use encase::internal::WriteInto;
use encase::{ShaderType, UniformBuffer};
use std::marker::PhantomData;
use web_time::{Duration, Instant};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::wgt::BufferDescriptor;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, Buffer,
    BufferBindingType, BufferUsages, Device, Queue, ShaderStages,
};

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

    pub fn data(&self) -> &T {
        &self.data
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
    pub fn init_with(device: &Device, data: &[T]) -> Self {
        let size_bytes = size_of_val(data);
        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: cast_slice(data),
            usage: BufferUsages::STORAGE | BufferUsages::VERTEX | BufferUsages::COPY_DST,
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
        let raw_bytes = size * size_of::<T>();
        let aligned_bytes = raw_bytes.next_multiple_of(4);


        let buffer = device.create_buffer(&BufferDescriptor {
            label: None,
            size: aligned_bytes as u64,
            usage: BufferUsages::VERTEX | BufferUsages::STORAGE | BufferUsages::COPY_DST,
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
        let read_write_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let read_only_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT | ShaderStages::VERTEX | ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
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

    pub fn write_with(&self, queue: &Queue, data: &[T]) {
        assert_eq!(data.len(), self.size_elements as usize);
        queue.write_buffer(&self.buffer, 0, cast_slice(data));
    }
}

pub struct FpsCounter {
    print_interval: Duration,
    last_print_time: Instant,
    frame_count: u32,
    last_fps: f64,
}
impl FpsCounter {
    pub fn new(print_interval: Duration) -> Self {
        Self {
            print_interval,
            last_print_time: Instant::now(),
            frame_count: 0,
            last_fps: 0.0,
        }
    }

    pub fn update(&mut self) -> f64 {
        self.frame_count += 1;

        let elapsed = self.last_print_time.elapsed();

        if elapsed >= self.print_interval {
            self.last_fps = self.frame_count as f64 / elapsed.as_secs_f64();
            self.frame_count = 0;
            self.last_print_time = Instant::now();
        };

        self.last_fps
    }
}
