#![allow(dead_code, clippy::too_many_arguments)]

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

