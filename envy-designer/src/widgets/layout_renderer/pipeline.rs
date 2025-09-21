
/// Sample count for the WgpuBackend render target
///
/// This is used for MSAA on the rendering to reduce hard edges for fonts
pub const SAMPLE_COUNT: u32 = 4;

pub struct CopyTexturePipeline {
    render_target: wgpu::TextureView,
    resolved_target: Option<wgpu::TextureView>,
    copy_texture_bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl CopyTexturePipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("copy_texture.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into())
        });

        let render_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("CopyTexturePipeline.render_target"),
            size: wgpu::Extent3d {
                width: 1920,
                height: 1080,
                depth_or_array_layers: 1
            },
            mip_level_count: 1,
            sample_count: SAMPLE_COUNT,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | if SAMPLE_COUNT == 1 { wgpu::TextureUsages::TEXTURE_BINDING } else { wgpu::TextureUsages::empty() },
            view_formats: &[format.remove_srgb_suffix(), format.add_srgb_suffix()]
        });

        let render_target = render_target.create_view(&Default::default());

        let resolved_target = if SAMPLE_COUNT != 1 {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("CopyTexturePipeline.resolved_target"),
                size: wgpu::Extent3d {
                    width: 1920,
                    height: 1080,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[format.remove_srgb_suffix(), format.add_srgb_suffix()]
            });

            Some(texture.create_view(&Default::default()))
        } else {
            None
        };

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("CopyTexturePipeline.copy_texture_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false
                    },
                    count: None,
                }
            ]
        });

        let copy_texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("CopyTexturePipeline.copy_texture_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&device.create_sampler(&wgpu::SamplerDescriptor {
                        mag_filter: wgpu::FilterMode::Linear,
                        min_filter: wgpu::FilterMode::Linear,
                        ..Default::default()
                    })),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(resolved_target.as_ref().unwrap_or(&render_target)),
                }
            ]
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("CopyTexturePipeline.copy_texture_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[]
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("CopyTexturePipeline.copy_texture_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[]
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::all()
                })]
            }),
            multiview: None,
            cache: None
        });

        Self {
            render_target,
            resolved_target,
            copy_texture_bind_group,
            pipeline
        }
    }

    pub fn render_to_texture(&self, encoder: &mut wgpu::CommandEncoder, f: impl FnOnce(&mut wgpu::RenderPass)) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("CopyTexturePipeline.render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.render_target,
                resolve_target: self.resolved_target.as_ref(),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.3,
                        g: 0.3,
                        b: 0.3,
                        a: 1.0
                    }),
                    store: wgpu::StoreOp::Store,
                }
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None
        });

        f(&mut rpass);
    }

    pub fn render_texture(&self, pass: &mut wgpu::RenderPass) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.copy_texture_bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}
