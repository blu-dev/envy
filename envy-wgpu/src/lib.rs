use std::{
    borrow::Cow,
    num::NonZeroU64,
    ops::{Index, IndexMut, Range},
    sync::Arc,
};

use bitvec::vec::BitVec;
use bytemuck::{Pod, Zeroable};
use camino::Utf8Path;
use cosmic_text::{
    CacheKey, Command, Family, FontSystem, Metrics, SwashCache,
    fontdb::{FaceInfo, Source},
};
use envy::{
    DrawUniform, EnvyBackend, PreparedGlyph, TextLayoutArgs, ViewUniform, asset::EnvyAssetProvider,
};
use glam::{Mat4, Vec3, Vec4};
use image::{ImageEncoder, codecs::png::PngEncoder};
use indexmap::IndexMap;
use lyon::{
    math::point,
    path::FillRule,
    tessellation::{
        FillGeometryBuilder, FillOptions, FillTessellator, FillVertex, GeometryBuilder,
        GeometryBuilderError, VertexId,
    },
};
use wgpu::{RenderPass, util::DeviceExt};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct WgpuTextureHandle(usize);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct WgpuUniformHandle(usize);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct WgpuFontHandle(usize);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct WgpuGlyphHandle(usize);

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct TextureVertex {
    pos: glam::Vec3,
    tex: glam::Vec2,
}

#[rustfmt::skip]
impl TextureVertex {
    const TOP_LEFT:     Self = Self { pos: glam::Vec3::new(-0.5, -0.5, 0.0), tex: glam::Vec2::ZERO };
    const TOP_RIGHT:    Self = Self { pos: glam::Vec3::new( 0.5, -0.5, 0.0), tex: glam::Vec2::X };
    const BOTTOM_LEFT:  Self = Self { pos: glam::Vec3::new(-0.5,  0.5, 0.0), tex: glam::Vec2::Y };
    const BOTTOM_RIGHT: Self = Self { pos: glam::Vec3::new( 0.5,  0.5, 0.0), tex: glam::Vec2::ONE };
}

struct ReservedTexture {
    texture: wgpu::Texture,
    sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
}

struct TextureBackend {
    image_cache: IndexMap<Cow<'static, str>, wgpu::Texture>,
    textures: Vec<ReservedTexture>,
    texture_slots: BitVec,
    texture_bgl: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
}

impl TextureBackend {
    #[rustfmt::skip]
    const TEXTURE_VERTICES: &[TextureVertex] = &[
        TextureVertex::TOP_LEFT, TextureVertex::BOTTOM_LEFT, TextureVertex::TOP_RIGHT,
        TextureVertex::TOP_RIGHT, TextureVertex::BOTTOM_LEFT, TextureVertex::BOTTOM_RIGHT
    ];

    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view_bgl: &wgpu::BindGroupLayout,
        draw_bgl: &wgpu::BindGroupLayout,
        render_target_format: wgpu::TextureFormat,
        sample_count: usize,
    ) -> Self {
        let mut default_texture_bytes = vec![0u8; 4 * 40 * 40];
        for y in 0..40 {
            for x in 0..40 {
                let start = (x + y * 40) * 4;
                let bytes = if (x / 10) % 2 == (y / 10) % 2 {
                    [0xFF, 0x00, 0xFF, 0xFF]
                } else {
                    [0x00, 0x00, 0x00, 0xFF]
                };
                default_texture_bytes[start..start + 4].copy_from_slice(&bytes);
            }
        }

        let texture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("envy_texture_pipeline_layout_texture_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let default_texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("envy_default_texture"),
                size: wgpu::Extent3d {
                    width: 40,
                    height: 40,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
            wgpu::wgt::TextureDataOrder::MipMajor,
            &default_texture_bytes,
        );

        let texture_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("envy_texture_pipeline_layout"),
                bind_group_layouts: &[&view_bgl, &draw_bgl, &texture_bgl],
                push_constant_ranges: &[],
            });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("envy_texture_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("./shaders/texture.wgsl").into()),
        });

        let texture_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("envy_texture_pipeline"),
            layout: Some(&texture_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: wgpu::VertexFormat::Float32x3.size()
                        + wgpu::VertexFormat::Float32x2.size(),
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: wgpu::VertexFormat::Float32x3.size(),
                            shader_location: 1,
                        },
                    ],
                }],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: sample_count as u32,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: render_target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::all(),
                })],
            }),
            multiview: None,
            cache: None,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("envy_texture_vertex_buffer"),
            contents: bytemuck::cast_slice(Self::TEXTURE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let mut image_cache = IndexMap::new();
        image_cache.insert("".into(), default_texture.clone());
        Self {
            image_cache,
            textures: vec![],
            texture_slots: BitVec::new(),
            texture_bgl,
            pipeline: texture_pipeline,
            vertex_buffer,
        }
    }

    fn reset(&mut self) {
        self.image_cache.clear();
        self.textures.clear();
    }
}

struct BufferVec<T: Pod + Zeroable> {
    buffer: Option<wgpu::Buffer>,
    cpu: Vec<T>,
    usages: wgpu::BufferUsages,
    dirty: bool,
}

impl<T: Pod + Zeroable> BufferVec<T> {
    pub fn new(usages: wgpu::BufferUsages) -> Self {
        Self {
            buffer: None,
            cpu: vec![],
            usages: usages | wgpu::BufferUsages::COPY_DST,
            dirty: false,
        }
    }

    pub fn len(&self) -> usize {
        self.cpu.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn push(&mut self, value: T) {
        self.cpu.push(value);
        self.dirty = true;
    }

    pub fn extend(&mut self, values: impl IntoIterator<Item = T>) {
        self.cpu.extend(values);
        self.dirty = true;
    }

    pub fn truncate(&mut self, new_len: usize) {
        // No reason do dirty here, if all we do is truncate then we aren't modifying data
        self.cpu.truncate(new_len);
    }

    pub fn buffer(&self) -> Option<&wgpu::Buffer> {
        self.buffer.as_ref()
    }

    pub fn flush(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> bool {
        if !self.dirty {
            return false;
        }
        self.dirty = false;

        let bytes: &[u8] = bytemuck::cast_slice(&self.cpu);
        if self.buffer.is_none() {
            self.buffer = Some(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytes,
                    usage: self.usages,
                }),
            );
            return true;
        }

        let buffer = self.buffer.as_ref().unwrap();

        let mut needs_recreate_bind_group = false;

        let buffer = if buffer.size() <= bytes.len() as u64 {
            let new_size = u64::max(buffer.size() * 2, bytes.len() as u64);
            let new_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: new_size,
                usage: self.usages,
                mapped_at_creation: false,
            });
            self.buffer = Some(new_buffer);
            needs_recreate_bind_group = true;
            self.buffer.as_ref().unwrap()
        } else {
            buffer
        };

        queue.write_buffer(buffer, 0, bytes);
        needs_recreate_bind_group
    }
}

impl<T: Pod + Zeroable> Index<usize> for BufferVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.cpu[index]
    }
}

impl<T: Pod + Zeroable> IndexMut<usize> for BufferVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.dirty = true;
        &mut self.cpu[index]
    }
}

struct WgpuFontBackend {
    constant_pipeline: wgpu::RenderPipeline,
    system: FontSystem,
    swash: SwashCache,
    index_ranges: IndexMap<CacheKey, Range<u32>>,
    vertices: BufferVec<Vec3>,
    indices: BufferVec<i32>,
    loaded_fonts: IndexMap<String, FaceInfo>,
}

impl WgpuFontBackend {
    pub fn new(
        device: &wgpu::Device,
        view_bgl: &wgpu::BindGroupLayout,
        draw_bgl: &wgpu::BindGroupLayout,
        render_target_format: wgpu::TextureFormat,
        sample_count: usize,
    ) -> Self {
        let system = FontSystem::new_with_locale_and_db(
            "".to_string(),
            cosmic_text::fontdb::Database::new(),
        );

        let constant_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("envy_constant_pipeline_layout"),
                bind_group_layouts: &[view_bgl, draw_bgl],
                push_constant_ranges: &[],
            });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("envy_constant_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("./shaders/constant.wgsl").into()),
        });

        let constant_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("envy_constant_pipeline"),
            layout: Some(&constant_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: wgpu::VertexFormat::Float32x3.size(),
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    }],
                }],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: sample_count as u32,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: render_target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::all(),
                })],
            }),
            multiview: None,
            cache: None,
        });

        Self {
            system,
            swash: SwashCache::new(),
            index_ranges: IndexMap::new(),
            vertices: BufferVec::new(wgpu::BufferUsages::VERTEX),
            indices: BufferVec::new(wgpu::BufferUsages::INDEX),
            loaded_fonts: IndexMap::new(),
            constant_pipeline: constant_pipeline,
        }
    }

    pub fn reset(&mut self) {
        self.index_ranges.clear();
        self.vertices.truncate(0);
        self.indices.truncate(0);
        self.loaded_fonts.clear();
        self.system = FontSystem::new_with_locale_and_db(
            "".to_string(),
            cosmic_text::fontdb::Database::new(),
        );
    }

    pub fn add_font(&mut self, name: impl Into<String>, font_data: Vec<u8>) -> FaceInfo {
        let ids = self
            .system
            .db_mut()
            .load_font_source(Source::Binary(Arc::new(font_data)));
        let face = self.system.db().face(ids[0]).unwrap();
        self.loaded_fonts.insert(name.into(), face.clone());
        face.clone()
    }

    fn prepare_glyph(&mut self, key: CacheKey, width: f32, height: f32) -> WgpuGlyphHandle {
        if let Some((idx, _, _)) = self.index_ranges.get_full(&key) {
            return WgpuGlyphHandle(idx);
        }

        let commands = self
            .swash
            .get_outline_commands(&mut self.system, key)
            .unwrap();

        let mut builder = lyon::path::Path::builder().with_svg();

        let mut is_open = false;

        let center_x = width / 2.0;
        let center_y = height / 2.0;
        let norm_point = |x: f32, y: f32| point(x - center_x, y - center_y);

        for command in commands.iter() {
            match command {
                Command::MoveTo(p) => {
                    if is_open {
                        builder.close();
                    }
                    is_open = true;

                    builder.move_to(norm_point(p.x, -p.y));
                }
                Command::Close => {
                    if is_open {
                        builder.close();
                    }
                    is_open = false;
                }
                Command::LineTo(p) => {
                    is_open = true;
                    builder.line_to(norm_point(p.x, -p.y));
                }
                Command::QuadTo(ctrl, p) => {
                    is_open = true;
                    builder.quadratic_bezier_to(norm_point(ctrl.x, -ctrl.y), norm_point(p.x, -p.y));
                }
                Command::CurveTo(ctrl_a, ctrl_b, p) => {
                    is_open = true;
                    builder.cubic_bezier_to(
                        norm_point(ctrl_a.x, -ctrl_a.y),
                        norm_point(ctrl_b.x, -ctrl_b.y),
                        norm_point(p.x, -p.y),
                    );
                }
            }
        }

        let path = builder.build();
        let start = self.indices.len() as u32;
        let mut fill_tesselator = FillTessellator::new();
        let mut builder = InPlaceBufferBuilders {
            vertex_start: self.vertices.len(),
            index_start: self.indices.len(),
            vertex_buffer: &mut self.vertices,
            index_buffer: &mut self.indices,
        };
        fill_tesselator
            .tessellate_path(
                &path,
                &FillOptions::tolerance(0.02).with_fill_rule(FillRule::NonZero),
                &mut builder,
            )
            .unwrap();

        let index = self.index_ranges.len();
        self.index_ranges
            .insert(key, start..self.indices.len() as u32);
        WgpuGlyphHandle(index)
    }

    pub fn layout(
        &mut self,
        mut new_uniform: impl FnMut() -> WgpuUniformHandle,
        args: TextLayoutArgs<'_, WgpuBackend>,
    ) -> Vec<PreparedGlyph<WgpuBackend>> {
        let face = &self.loaded_fonts[args.handle.0];

        let metrics = Metrics::new(args.font_size, args.line_height);
        let mut buffer = cosmic_text::Buffer::new(&mut self.system, metrics);
        let mut buffer = buffer.borrow_with(&mut self.system);
        buffer.set_size(Some(args.buffer_size.x), Some(args.buffer_size.y));
        buffer.set_text(
            args.text.as_ref(),
            &cosmic_text::Attrs {
                family: Family::Name(&face.families[0].0),
                stretch: face.stretch,
                style: face.style,
                weight: face.weight,
                ..cosmic_text::Attrs::new()
            },
            cosmic_text::Shaping::Basic,
        );

        let mut glyphs = vec![];

        for run in buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                glyphs.push((
                    CacheKey::new(
                        glyph.font_id,
                        glyph.glyph_id,
                        glyph.font_size,
                        (0.0, 0.0),
                        glyph.cache_key_flags,
                    )
                    .0,
                    glyph.w,
                    run.line_height,
                    glyph.x + glyph.x_offset * glyph.font_size,
                    glyph.y + glyph.y_offset * glyph.font_size + run.line_y,
                ));
            }
        }

        drop(buffer);

        let mut prepared_glyphs = vec![];
        for (key, w, h, x, y) in glyphs {
            let handle = self.prepare_glyph(key, w, h);
            prepared_glyphs.push(PreparedGlyph {
                glyph_handle: handle,
                uniform_handle: new_uniform(),
                offset_in_buffer: glam::Vec2::new(x, y),
                size: glam::Vec2::new(w, h),
            });
        }

        prepared_glyphs
    }
}

const fn align_up(value: usize, align: usize) -> usize {
    (value + (align - 1)) & !(align - 1)
}

struct InPlaceBufferBuilders<'a> {
    vertex_buffer: &'a mut BufferVec<Vec3>,
    index_buffer: &'a mut BufferVec<i32>,
    vertex_start: usize,
    index_start: usize,
}

impl GeometryBuilder for InPlaceBufferBuilders<'_> {
    fn begin_geometry(&mut self) {
        self.vertex_start = self.vertex_buffer.len();
        self.index_start = self.index_buffer.len();
    }
    fn add_triangle(&mut self, a: VertexId, b: VertexId, c: VertexId) {
        debug_assert!(a != b);
        debug_assert!(a != c);
        debug_assert!(b != c);
        debug_assert!(a != VertexId::INVALID);
        debug_assert!(b != VertexId::INVALID);
        debug_assert!(c != VertexId::INVALID);

        self.index_buffer
            .extend([a, b, c].map(|vertex| u32::from(vertex) as i32));
    }

    fn abort_geometry(&mut self) {
        self.vertex_buffer.truncate(self.vertex_start);
        self.index_buffer.truncate(self.index_start);
    }
}

impl FillGeometryBuilder for InPlaceBufferBuilders<'_> {
    fn add_fill_vertex(&mut self, vertex: FillVertex) -> Result<VertexId, GeometryBuilderError> {
        let length = self.vertex_buffer.len();
        if length >= u32::MAX as usize {
            return Err(GeometryBuilderError::TooManyVertices);
        }
        self.vertex_buffer
            .push(Vec3::from(vertex.position().to_3d().to_array()));

        Ok(VertexId(length as u32))
    }
}
pub struct WgpuBackend {
    device: wgpu::Device,
    queue: wgpu::Queue,
    view_group: wgpu::BindGroup,
    view_buffer: wgpu::Buffer,
    draw_bgl: wgpu::BindGroupLayout,
    uniforms: BufferVec<DrawUniform>,
    uniform_slots: BitVec,
    textures: TextureBackend,
    fonts: WgpuFontBackend,
    uniform_bind_group: Option<wgpu::BindGroup>,
}

impl WgpuBackend {
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        render_target_format: wgpu::TextureFormat,
        sample_count: usize,
    ) -> Self {
        let view_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("envy_view_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(std::mem::size_of::<ViewUniform>() as u64),
                },
                count: None,
            }],
        });

        let view_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("envy_view_buffer"),
            contents: bytemuck::bytes_of(&ViewUniform::new(
                glam::Mat4::IDENTITY,
                glam::Mat4::orthographic_rh(0.0, 1920.0, 1080.0, 0.0, 0.0, 1.0),
            )),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let view_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &view_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: view_buffer.as_entire_binding(),
            }],
        });

        let draw_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("envy_draw_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: NonZeroU64::new(std::mem::size_of::<DrawUniform>() as u64),
                },
                count: None,
            }],
        });

        let textures = TextureBackend::new(
            &device,
            &queue,
            &view_bgl,
            &draw_bgl,
            render_target_format,
            sample_count,
        );

        let fonts = WgpuFontBackend::new(
            &device,
            &view_bgl,
            &draw_bgl,
            render_target_format,
            sample_count,
        );

        Self {
            device,
            queue,
            view_buffer,
            view_group,
            draw_bgl,
            uniforms: BufferVec::new(wgpu::BufferUsages::UNIFORM),
            uniform_slots: BitVec::new(),
            textures,
            fonts,
            uniform_bind_group: None,
        }
    }

    pub fn clear(&mut self) {
        self.uniforms.truncate(0);
        self.textures.reset();
        self.fonts.reset();
    }

    pub fn add_texture(
        &mut self,
        name: impl Into<Cow<'static, str>>,
        image: &[u8],
    ) -> wgpu::Texture {
        let image = image::load_from_memory(image).unwrap().to_rgba8();

        let texture = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: image.width(),
                    height: image.height(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
            wgpu::wgt::TextureDataOrder::LayerMajor,
            image.as_raw(),
        );

        if let Some(prev) = self
            .textures
            .image_cache
            .insert(name.into(), texture.clone())
        {
            self.textures.textures.iter_mut().for_each(|reserved| {
                if reserved.texture == prev {
                    reserved.texture = texture.clone();
                    reserved.bind_group =
                        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: None,
                            layout: &self.textures.texture_bgl,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::Sampler(&reserved.sampler),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::TextureView(
                                        &texture.create_view(&Default::default()),
                                    ),
                                },
                            ],
                        });
                }
            });
        }
        texture
    }

    pub fn load_textures_from_bytes<'a>(
        &mut self,
        names_and_bytes: impl IntoIterator<Item = (&'a str, Cow<'a, [u8]>)>,
    ) {
        for (name, bytes) in names_and_bytes {
            let image = image::load_from_memory(&bytes).unwrap().to_rgba8();

            let texture = self.device.create_texture_with_data(
                &self.queue,
                &wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d {
                        width: image.width(),
                        height: image.height(),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                },
                wgpu::wgt::TextureDataOrder::LayerMajor,
                image.as_raw(),
            );

            self.textures
                .image_cache
                .insert(name.to_string().into(), texture);
        }
    }

    pub fn load_textures_from_paths<'a>(
        &mut self,
        names_and_paths: impl IntoIterator<Item = (&'a str, &'a Utf8Path)>,
    ) {
        self.load_textures_from_bytes(
            names_and_paths
                .into_iter()
                .map(|(name, path)| (name, std::fs::read(path).unwrap().into())),
        )
    }

    pub fn add_font(&mut self, name: impl Into<String>, font: Vec<u8>) -> FaceInfo {
        self.fonts.add_font(name, font)
    }

    pub fn get_texture(&self, name: impl AsRef<str>) -> Option<&wgpu::Texture> {
        self.textures.image_cache.get(name.as_ref())
    }

    pub fn get_font_face_info(&self, name: impl AsRef<str>) -> Option<&FaceInfo> {
        self.fonts.loaded_fonts.get(name.as_ref())
    }

    pub fn load_fonts_from_bytes<'a>(
        &mut self,
        names_and_bytes: impl IntoIterator<Item = (&'a str, Vec<u8>)>,
    ) {
        for (name, font) in names_and_bytes {
            self.fonts.add_font(name, font);
        }
    }

    pub fn prep_render(&self, pass: &mut RenderPass<'_>) {
        pass.set_bind_group(0, &self.view_group, &[]);
    }

    pub fn update(&mut self) {
        if self.uniforms.flush(&self.device, &self.queue) {
            self.uniform_bind_group =
                Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &self.draw_bgl,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: self.uniforms.buffer.as_ref().unwrap(),
                            offset: 0,
                            size: NonZeroU64::new(std::mem::size_of::<DrawUniform>() as u64),
                        }),
                    }],
                }));
        }
        self.fonts.vertices.flush(&self.device, &self.queue);
        self.fonts.indices.flush(&self.device, &self.queue);
    }

    pub fn rename_texture(&mut self, old: &str, new: impl Into<Cow<'static, str>>) {
        let Some(index) = self.textures.image_cache.get_index_of(old) else {
            return;
        };

        let texture = self
            .textures
            .image_cache
            .shift_remove_index(index)
            .unwrap()
            .1;

        self.textures
            .image_cache
            .insert_before(index, new.into(), texture);
    }

    pub fn rename_font(&mut self, old: &str, new: impl Into<String>) {
        let Some(index) = self.fonts.loaded_fonts.get_index_of(old) else {
            return;
        };

        let face_name = self.fonts.loaded_fonts.shift_remove_index(index).unwrap().1;

        self.fonts
            .loaded_fonts
            .insert_before(index, new.into(), face_name);
    }

    pub fn remove_font(&mut self, name: &str) {
        let _ = self.fonts.loaded_fonts.shift_remove(name);
    }

    pub fn remove_texture(&mut self, name: &str) {
        let _ = self.textures.image_cache.shift_remove(name);
    }

    pub fn iter_texture_names<'a>(&'a self) -> impl Iterator<Item = &'a str> {
        self.textures
            .image_cache
            .keys()
            .map(|key| key.as_ref())
            .filter(|k| k.ne(&""))
    }

    pub fn iter_font_names<'a>(&'a self) -> impl Iterator<Item = &'a str> {
        self.fonts.loaded_fonts.keys().map(|key| key.as_ref())
    }

    pub fn dump_fonts(&self) -> Vec<(String, Vec<u8>)> {
        self.fonts
            .loaded_fonts
            .iter()
            .map(|(name, face_name)| {
                let source = self
                    .fonts
                    .system
                    .db()
                    .faces()
                    .find(|face| {
                        face.families
                            .iter()
                            .any(|(family_name, _)| family_name.eq(&face.families[0].0))
                    })
                    .map(|face| {
                        let Source::Binary(binary) = &face.source else {
                            unimplemented!()
                        };

                        let bytes: &[u8] = (**binary).as_ref();
                        bytes.to_vec()
                    })
                    .unwrap();

                (name.clone(), source)
            })
            .collect()
    }

    pub fn dump_textures(&self) -> Vec<(String, Vec<u8>)> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let mut buffers = vec![];
        for (name, texture) in self.textures.image_cache.iter() {
            let size = texture.size();
            let stride = size.width as u64 * 4;
            let real_stride = align_up(stride as usize, 0x100) as u64;
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: real_stride * size.height as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(real_stride as u32),
                        rows_per_image: None,
                    },
                },
                size,
            );

            buffers.push((name, buffer, size));
        }

        let index = self.queue.submit([encoder.finish()]);
        buffers.iter().for_each(|(_, buffer, _)| {
            buffer.map_async(wgpu::MapMode::Read, .., |res| res.unwrap())
        });
        self.device
            .poll(wgpu::PollType::WaitForSubmissionIndex(index))
            .unwrap();

        let mut out_images = vec![];
        for (name, buffer, size) in buffers {
            let stride = (size.width * 4) as usize;
            let real_stride = align_up(stride, 0x100);
            let bytes = buffer.get_mapped_range(..);

            let mut image_bytes = vec![0u8; stride * size.height as usize];
            for y in 0..size.height as usize {
                image_bytes[y * stride..(y + 1) * stride]
                    .copy_from_slice(&bytes[y * real_stride..y * real_stride + stride]);
            }

            let mut out = std::io::Cursor::new(vec![]);
            {
                let encoder = PngEncoder::new_with_quality(
                    &mut out,
                    image::codecs::png::CompressionType::Best,
                    image::codecs::png::FilterType::Adaptive,
                );
                encoder
                    .write_image(
                        &image_bytes,
                        size.width,
                        size.height,
                        image::ExtendedColorType::Rgba8,
                    )
                    .unwrap();
            }

            out_images.push((name.to_string(), out.into_inner()));
        }

        out_images
    }
}

impl EnvyBackend for WgpuBackend {
    type TextureHandle = WgpuTextureHandle;
    type UniformHandle = WgpuUniformHandle;
    type FontHandle = WgpuFontHandle;
    type GlyphHandle = WgpuGlyphHandle;

    type RenderPass<'a> = wgpu::RenderPass<'a>;

    fn request_texture_by_name(&mut self, name: impl AsRef<str>) -> Option<Self::TextureHandle> {
        let texture = self.textures.image_cache.get(name.as_ref())?.clone();
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.textures.texture_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &texture.create_view(&Default::default()),
                    ),
                },
            ],
        });

        let texture = ReservedTexture {
            texture,
            sampler: self.device.create_sampler(&Default::default()),
            bind_group,
        };

        if let Some(first_available) = self.textures.texture_slots.first_zero() {
            self.textures.texture_slots.set(first_available, true);
            self.textures.textures[first_available] = texture;
            Some(WgpuTextureHandle(first_available))
        } else {
            let handle = WgpuTextureHandle(self.textures.textures.len());
            self.textures.texture_slots.push(true);
            self.textures.textures.push(texture);
            Some(handle)
        }
    }

    fn request_font_by_name(&mut self, name: impl AsRef<str>) -> Option<Self::FontHandle> {
        self.fonts
            .loaded_fonts
            .get_index_of(name.as_ref())
            .map(WgpuFontHandle)
    }

    fn request_new_uniform(&mut self) -> Option<Self::UniformHandle> {
        if let Some(first_available) = self.uniform_slots.first_zero() {
            self.uniform_slots.set(first_available, true);
            Some(WgpuUniformHandle(first_available))
        } else {
            let len = self.uniforms.len();
            self.uniform_slots.push(true);
            self.uniforms.push(DrawUniform::new(glam::Mat4::IDENTITY, glam::Vec4::ONE));
            Some(WgpuUniformHandle(len))
        }
    }

    fn update_uniform(&mut self, uniform: Self::UniformHandle, data: crate::DrawUniform) {
        self.uniforms[uniform.0] = data;
    }

    fn layout_text(&mut self, args: TextLayoutArgs<'_, Self>) -> Vec<PreparedGlyph<Self>> {
        self.fonts.layout(|| {
            if let Some(first_available) = self.uniform_slots.first_zero() {
                self.uniform_slots.set(first_available, true);
                WgpuUniformHandle(first_available)
            } else {
                let len = self.uniforms.len();
                self.uniform_slots.push(true);
                self.uniforms.push(DrawUniform::new(glam::Mat4::IDENTITY, glam::Vec4::ONE));
                WgpuUniformHandle(len)
            }
        }, args)
    }

    fn draw_texture(
        &self,
        uniform: Self::UniformHandle,
        texture: Self::TextureHandle,
        pass: &mut Self::RenderPass<'_>,
    ) {
        pass.set_pipeline(&self.textures.pipeline);
        pass.set_bind_group(
            1,
            self.uniform_bind_group.as_ref().unwrap(),
            &[(uniform.0 * std::mem::size_of::<DrawUniform>()) as wgpu::DynamicOffset],
        );
        pass.set_bind_group(2, &self.textures.textures[texture.0].bind_group, &[]);
        pass.set_vertex_buffer(0, self.textures.vertex_buffer.slice(..));
        pass.draw(0..6, 0..1);
    }

    fn draw_glyph(
        &self,
        uniform: Self::UniformHandle,
        glyph: Self::GlyphHandle,
        pass: &mut Self::RenderPass<'_>,
    ) {
        let index_range = self
            .fonts
            .index_ranges
            .get_index(glyph.0)
            .unwrap()
            .1
            .clone();
        pass.set_pipeline(&self.fonts.constant_pipeline);
        pass.set_bind_group(
            1,
            self.uniform_bind_group.as_ref().unwrap(),
            &[(uniform.0 * std::mem::size_of::<DrawUniform>()) as wgpu::DynamicOffset],
        );
        pass.set_vertex_buffer(0, self.fonts.vertices.buffer().unwrap().slice(..));
        pass.set_index_buffer(
            self.fonts.indices.buffer().unwrap().slice(..),
            wgpu::IndexFormat::Uint32,
        );
        pass.draw_indexed(index_range, 0, 0..1);
    }

    fn draw_glyphs(
        &self,
        uniforms_and_glyphs: impl IntoIterator<Item = (Self::UniformHandle, Self::GlyphHandle)>,
        pass: &mut Self::RenderPass<'_>,
    ) {
        for (uniform, glyph) in uniforms_and_glyphs {
            self.draw_glyph(uniform, glyph, pass);
        }
    }

    fn release_font(&mut self, _handle: Self::FontHandle) {}

    fn release_texture(&mut self, handle: Self::TextureHandle) {
        self.textures.texture_slots.set(handle.0, false);
    }

    fn release_uniform(&mut self, handle: Self::UniformHandle) {
        self.uniform_slots.set(handle.0, false);
    }

    // fn update_texture_by_name(&mut self, handle: Self::TextureHandle, name: impl AsRef<str>) {
    //     let Some(texture) = self.textures.image_cache.get(name.as_ref()).cloned() else {
    //         return;
    //     };

    //     let existing_texture = &mut self.textures.textures[handle.0];
    //     let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
    //         label: None,
    //         layout: &self.textures.texture_bgl,
    //         entries: &[
    //             wgpu::BindGroupEntry {
    //                 binding: 0,
    //                 resource: wgpu::BindingResource::Sampler(&existing_texture.sampler),
    //             },
    //             wgpu::BindGroupEntry {
    //                 binding: 1,
    //                 resource: wgpu::BindingResource::TextureView(
    //                     &texture.create_view(&Default::default()),
    //                 ),
    //             },
    //         ],
    //     });

    //     existing_texture.texture = texture.clone();
    //     existing_texture.bind_group = bind_group;
    // }
}

impl EnvyAssetProvider for WgpuBackend {
    fn fetch_font_bytes_by_name<'a>(&'a self, name: &str) -> Cow<'a, [u8]> {
        self.fonts
            .loaded_fonts
            .get(name)
            .map(|face| match &face.source {
                Source::Binary(binary) => Cow::Borrowed((**binary).as_ref()),
                _ => unimplemented!(),
            })
            .unwrap()
    }

    fn fetch_image_bytes_by_name<'a>(&'a self, name: &str) -> Cow<'a, [u8]> {
        let texture = self.textures.image_cache.get(name).unwrap();

        let size = texture.size();
        let stride = size.width as u64 * 4;
        let real_stride = align_up(stride as usize, 0x100) as u64;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: real_stride * size.height as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("dump_texture_encoder"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(real_stride as u32),
                    rows_per_image: None,
                },
            },
            size,
        );

        let index = self.queue.submit([encoder.finish()]);
        buffer.map_async(wgpu::MapMode::Read, .., |res| res.unwrap());
        self.device
            .poll(wgpu::PollType::WaitForSubmissionIndex(index))
            .unwrap();

        let stride = (size.width * 4) as usize;
        let real_stride = align_up(stride, 0x100);
        let bytes = buffer.get_mapped_range(..);

        let mut image_bytes = vec![0u8; stride * size.height as usize];
        for y in 0..size.height as usize {
            image_bytes[y * stride..(y + 1) * stride]
                .copy_from_slice(&bytes[y * real_stride..y * real_stride + stride]);
        }

        let mut out = std::io::Cursor::new(vec![]);
        {
            let encoder = PngEncoder::new_with_quality(
                &mut out,
                image::codecs::png::CompressionType::Best,
                image::codecs::png::FilterType::Adaptive,
            );
            encoder
                .write_image(
                    &image_bytes,
                    size.width,
                    size.height,
                    image::ExtendedColorType::Rgba8,
                )
                .unwrap();
        }

        Cow::Owned(out.into_inner())
    }

    fn load_font_bytes_with_name(&mut self, name: String, bytes: Vec<u8>) {
        let _ = self.add_font(name, bytes);
    }

    fn load_image_bytes_with_name(&mut self, name: String, bytes: Vec<u8>) {
        let _ = self.add_texture(name, &bytes);
    }
}
