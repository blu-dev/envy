use std::{any::Any, borrow::Cow};

use camino::Utf8Path;
use glam::{Affine2, Vec2};
use serde::{Deserialize, Serialize};

use crate::DrawUniform;

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    Custom(Vec2),
}

impl Anchor {
    pub const fn as_vec(self) -> Vec2 {
        match self {
            Self::TopLeft => Vec2::splat(-0.5),
            Self::TopCenter => Vec2::new(0.0, -0.5),
            Self::TopRight => Vec2::new(0.5, -0.5),
            Self::CenterLeft => Vec2::new(-0.5, 0.0),
            Self::Center => Vec2::ZERO,
            Self::CenterRight => Vec2::new(0.5, 0.0),
            Self::BottomLeft => Vec2::new(-0.5, 0.5),
            Self::BottomCenter => Vec2::new(0.0, 0.5),
            Self::BottomRight => Vec2::splat(0.5),
            Self::Custom(custom) => custom,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Deserialize, Serialize)]
pub struct NodeSettings {
    /// Clockwise rotation of the node in degrees
    ///
    /// Rotation will happen around the center of the node, regardless of the node's anchor
    pub rotation: f32,

    /// Position of the node's anchor point
    pub position: Vec2,

    /// Size of the node, this defines the bounding box
    ///
    /// It is not an error for this to be zero, but for nodes that render, setting the size to zero
    /// can have unexpected consequences (like not being able to see the node)
    pub size: Vec2,

    /// Scale of the node
    pub scale: Vec2,

    /// The anchor point of this node
    pub anchor: Anchor,

    /// Premultiplied color passed directly to shaders
    pub color: [u8; 4],
}

impl NodeSettings {
    pub(crate) const ROOT: Self = Self {
        rotation: 0.0,
        position: Vec2::new(960.0, 540.0),
        size: Vec2::new(1920.0, 1080.0),
        scale: Vec2::ONE,
        anchor: Anchor::Center,
        color: [255; 4],
    };
}

pub struct TextLayoutArgs<'a, R: RenderBackend> {
    pub handle: R::FontHandle,
    pub font_size: f32,
    pub line_height: f32,
    pub buffer_size: Vec2,
    pub text: &'a str,
}

pub struct PreparedGlyph<R: RenderBackend> {
    pub handle: R::GlyphHandle,
    pub pos: Vec2,
    pub size: Vec2,
}

pub struct PreparationArgs<'a> {
    pub settings: &'a NodeSettings,
    pub local_affine: &'a Affine2,
    pub absolute_affine: &'a Affine2,
}

pub trait RenderBackend: Sized + 'static {
    type FontHandle: Copy + Clone + Send + Sync + 'static;
    type GlyphHandle: Copy + Clone + Send + Sync + 'static;
    type TextureHandle: Copy + Clone + Send + Sync + 'static;
    type UniformHandle: Copy + Clone + Send + Sync + 'static;

    type RenderPass<'a>;

    fn request_uniform(&mut self) -> Option<Self::UniformHandle>;
    fn request_texture_by_name(&mut self, name: impl AsRef<str>) -> Option<Self::TextureHandle>;
    fn request_font_by_name(&mut self, name: impl AsRef<str>) -> Option<Self::FontHandle>;
    fn update_uniform(&mut self, uniform: Self::UniformHandle, data: DrawUniform);
    fn update_texture_by_name(&mut self, handle: Self::TextureHandle, name: impl AsRef<str>);
    fn layout_text(&mut self, args: TextLayoutArgs<'_, Self>) -> Vec<PreparedGlyph<Self>>;

    fn draw_texture(
        &self,
        uniform: Self::UniformHandle,
        texture: Self::TextureHandle,
        pass: &mut Self::RenderPass<'_>,
    );
    fn draw_glyph(
        &self,
        uniform: Self::UniformHandle,
        glyph: Self::GlyphHandle,
        pass: &mut Self::RenderPass<'_>,
    );
}

// Todo: abstract the args to this to be a generic render provider
pub trait NodeImpl<R: RenderBackend>: Send + Sync + 'static {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn setup_resources(&mut self, backend: &mut R);
    fn prepare(&mut self, args: PreparationArgs<'_>, backend: &mut R);
    fn render(&self, backend: &R, pass: &mut R::RenderPass<'_>);
}

pub struct EmptyNode;

impl<R: RenderBackend> NodeImpl<R> for EmptyNode {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn prepare(&mut self, _args: PreparationArgs<'_>, _backend: &mut R) {}

    fn render(&self, _backend: &R, _pass: &mut <R as RenderBackend>::RenderPass<'_>) {}

    fn setup_resources(&mut self, _backend: &mut R) {}
}

pub struct TextureNode<R: RenderBackend> {
    texture_name: Cow<'static, str>,
    uniform: Option<R::UniformHandle>,
    texture: Option<R::TextureHandle>,
}

impl<R: RenderBackend> TextureNode<R> {
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self {
            texture_name: name.into(),
            uniform: None,
            texture: None,
        }
    }

    pub fn texture_name(&self) -> &str {
        self.texture_name.as_ref()
    }

    pub fn update_texture(&mut self, backend: &mut R, name: impl Into<Cow<'static, str>>) {
        let new_name = name.into();
        if self.texture_name == new_name {
            return;
        }

        self.texture_name = new_name;
        if let Some(texture) = self.texture {
            backend.update_texture_by_name(texture, self.texture_name.as_ref());
        } else {
            self.setup_resources(backend);
        }
    }
}

impl<R: RenderBackend> NodeImpl<R> for TextureNode<R> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn setup_resources(&mut self, backend: &mut R) {
        if self.uniform.is_none() {
            self.uniform = Some(
                backend
                    .request_uniform()
                    .expect("rendering backend did not give us uniform"),
            );
        }

        if self.texture.is_none() {
            self.texture = Some(
                backend
                    .request_texture_by_name(self.texture_name.as_ref())
                    .expect("rendering backend did not provide texture"),
            );
        }
    }

    fn prepare(&mut self, args: PreparationArgs<'_>, backend: &mut R) {
        let Some(handle) = self.uniform else {
            unimplemented!()
        };

        let affine2 = args.absolute_affine * Affine2::from_scale(args.settings.size);
        let affine3 = glam::Affine3A::from_mat3_translation(
            glam::Mat3::from_mat2(affine2.matrix2),
            affine2.translation.extend(0.0),
        );
        let matrix = glam::Mat4::from(affine3);
        let color = glam::Vec4::from_array(args.settings.color.map(|c| c as f32 / 255.0));

        backend.update_uniform(
            handle,
            DrawUniform {
                model_matrix: matrix,
                base_color: color,
                model_inverse_matrix: matrix.inverse(),
                padding: [0u8; 0x70],
            },
        );
    }

    fn render(&self, backend: &R, pass: &mut R::RenderPass<'_>) {
        let Some(buffer) = self.uniform else {
            unimplemented!()
        };

        let Some(texture) = self.texture else {
            unimplemented!()
        };

        backend.draw_texture(buffer, texture, pass);
    }
}

pub struct TextNode<R: RenderBackend> {
    pub(crate) font_name: String,
    pub(crate) font_size: f32,
    pub(crate) line_height: f32,
    pub(crate) text: String,
    font_handle: Option<R::FontHandle>,
    glyphs: Vec<PreparedGlyph<R>>,
    uniforms: Vec<R::UniformHandle>,
    cached_buffer_size: Vec2,
    dirty: bool,
}

impl<R: RenderBackend> TextNode<R> {
    pub fn new(name: impl Into<String>, size: f32, height: f32, text: impl Into<String>) -> Self {
        Self {
            font_name: name.into(),
            font_size: size,
            line_height: height,
            text: text.into(),
            font_handle: None,
            glyphs: vec![],
            uniforms: vec![],
            cached_buffer_size: Vec2::ZERO,
            dirty: true,
        }
    }

    pub fn font_name(&self) -> &str {
        self.font_name.as_str()
    }

    pub fn update_font_name(&mut self, backend: &mut R, name: impl Into<String>) {
        let name = name.into();
        if name == self.font_name {
            return;
        }
        self.font_name = name;
        self.font_handle = Some(
            backend
                .request_font_by_name(&self.font_name)
                .expect("render backend does not have font"),
        );
        self.dirty = true;
    }

    pub fn update_font_size(&mut self, size: f32) {
        if self.font_size == size {
            return;
        }

        self.font_size = size;
        self.dirty = true;
    }

    pub fn update_line_height(&mut self, height: f32) {
        if self.line_height == height {
            return;
        }

        self.line_height = height;
        self.dirty = true;
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        let text: String = text.into();
        if text == self.text {
            return;
        }

        self.text = text;
        self.dirty = true;
    }

    pub fn set_dirty(&mut self) {
        self.dirty = true;
    }
}

impl<R: RenderBackend> NodeImpl<R> for TextNode<R> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn setup_resources(&mut self, backend: &mut R) {
        if self.font_handle.is_none() {
            self.font_handle = Some(
                backend
                    .request_font_by_name(&self.font_name)
                    .expect("render backend does not have font"),
            );
        }
    }

    fn prepare(&mut self, args: PreparationArgs<'_>, backend: &mut R) {
        if self.dirty || self.cached_buffer_size != args.settings.size {
            self.glyphs = backend.layout_text(TextLayoutArgs {
                handle: self.font_handle.unwrap(),
                font_size: self.font_size,
                line_height: self.line_height,
                buffer_size: args.settings.size,
                text: &self.text,
            });

            for _ in self.uniforms.len()..self.glyphs.len() {
                self.uniforms.push(backend.request_uniform().unwrap());
            }
            self.dirty = false;
            self.cached_buffer_size = args.settings.size;
        }
        let color = glam::Vec4::from_array(args.settings.color.map(|c| c as f32 / 255.0));
        for (glyph, uniform) in self.glyphs.iter().zip(self.uniforms.iter()) {
            let center_x = -args.settings.size.x / 2.0 + glyph.pos.x + glyph.size.x / 2.0;
            let center_y = -args.settings.size.y / 2.0 + glyph.pos.y + glyph.size.y / 2.0;

            let affine2 =
                args.absolute_affine * Affine2::from_translation(Vec2::new(center_x, center_y));
            let affine3 = glam::Affine3A::from_mat3_translation(
                glam::Mat3::from_mat2(affine2.matrix2),
                affine2.translation.extend(0.0),
            );
            let matrix = glam::Mat4::from(affine3);

            backend.update_uniform(
                *uniform,
                DrawUniform {
                    model_matrix: matrix,
                    base_color: color,
                    model_inverse_matrix: matrix.inverse(),
                    padding: [0u8; 0x70],
                },
            );
        }
    }

    fn render(&self, backend: &R, pass: &mut R::RenderPass<'_>) {
        for (glyph, uniform) in self.glyphs.iter().zip(self.uniforms.iter()) {
            backend.draw_glyph(*uniform, glyph.handle, pass);
        }
    }
}

pub struct Node<R: RenderBackend> {
    pub(crate) name: String,
    pub(crate) children: Vec<Node<R>>,
    pub(crate) settings: NodeSettings,
    pub(crate) changed: bool,
    local: Affine2,
    absolute: Affine2,
    pub(crate) implementation: Box<dyn NodeImpl<R>>,
}

pub(crate) struct PropagationArgs<'a> {
    parent_affine: &'a Affine2,
    parent_settings: &'a NodeSettings,
    parent_changed: bool,
}

impl PropagationArgs<'_> {
    const ROOT_AFFINE: Affine2 = Affine2 {
        matrix2: glam::Mat2::IDENTITY,
        translation: glam::Vec2::ZERO,
    };

    pub const fn root_node() -> Self {
        Self {
            parent_affine: &Self::ROOT_AFFINE,
            parent_settings: &NodeSettings::ROOT,
            parent_changed: false,
        }
    }
}

impl<R: RenderBackend> Node<R> {
    pub fn new(
        name: impl Into<String>,
        pos: Vec2,
        size: Vec2,
        implementation: impl NodeImpl<R>,
    ) -> Self {
        Self::new_boxed(name, pos, size, Box::new(implementation))
    }

    pub fn new_boxed(
        name: impl Into<String>,
        pos: Vec2,
        size: Vec2,
        implementation: Box<dyn NodeImpl<R>>,
    ) -> Self {
        Self {
            name: name.into(),
            children: vec![],
            settings: NodeSettings {
                rotation: 0.0,
                position: pos,
                size,
                scale: Vec2::ONE,
                anchor: Anchor::TopLeft,
                color: [255; 4],
            },
            local: Affine2::IDENTITY,
            absolute: Affine2::IDENTITY,
            changed: true,
            implementation,
        }
    }

    pub fn with_settings(mut self, f: impl FnOnce(&mut NodeSettings)) -> Self {
        f(&mut self.settings);
        self
    }

    pub fn with_child(mut self, child: Node<R>) -> Self {
        self.children.push(child);
        self
    }

    pub fn try_downcast<T: NodeImpl<R>>(&self) -> Option<&T> {
        self.implementation.as_any().downcast_ref::<T>()
    }

    pub fn try_downcast_mut<T: NodeImpl<R>>(&mut self) -> Option<&mut T> {
        self.implementation.as_any_mut().downcast_mut::<T>()
    }

    pub fn set_impl(&mut self, backend: &mut R, node: impl NodeImpl<R>) {
        self.implementation = Box::new(node);
        self.implementation.setup_resources(backend);
    }

    pub(crate) fn setup(&mut self, backend: &mut R) {
        self.implementation.setup_resources(backend);
        self.children
            .iter_mut()
            .for_each(|child| child.setup(backend));
    }

    pub(crate) fn prepare(&mut self, backend: &mut R) {
        self.implementation.prepare(
            PreparationArgs {
                settings: &self.settings,
                local_affine: &self.local,
                absolute_affine: &self.absolute,
            },
            backend,
        );
        self.children
            .iter_mut()
            .for_each(|child| child.prepare(backend));
    }

    pub(crate) fn render(&self, backend: &R, pass: &mut R::RenderPass<'_>) {
        self.implementation.render(backend, pass);
        self.children
            .iter()
            .for_each(|child| child.render(backend, pass));
    }

    pub(crate) fn propagate(&mut self, args: PropagationArgs<'_>) {
        let did_change = self.changed || args.parent_changed;
        if did_change {
            self.changed = false;
            let actual_size = self.settings.size * self.settings.scale;
            let parent_anchor_to_origin =
                args.parent_settings.size * args.parent_settings.anchor.as_vec();
            let self_translation = parent_anchor_to_origin + self.settings.position;
            let center = self_translation + -self.settings.anchor.as_vec() * actual_size;
            self.local = Affine2::from_scale_angle_translation(
                self.settings.scale,
                self.settings.rotation.to_radians(),
                center,
            );

            self.absolute = *args.parent_affine * self.local;
        }

        self.children.iter_mut().for_each(|child| {
            child.propagate(PropagationArgs {
                parent_affine: &self.absolute,
                parent_settings: &self.settings,
                parent_changed: did_change,
            });
        });
    }
}

pub struct UiTree<R: RenderBackend> {
    pub(crate) root_children: Vec<Node<R>>,
    pub(crate) images: Vec<(String, image::RgbaImage)>,
}

impl<R: RenderBackend> UiTree<R> {
    pub(crate) fn propagate(&mut self) {
        self.root_children.iter_mut().for_each(|child| {
            child.propagate(PropagationArgs::root_node());
        });
    }

    pub(crate) fn setup(&mut self, backend: &mut R) {
        self.root_children.iter_mut().for_each(|child| {
            child.setup(backend);
        });
    }

    pub(crate) fn prepare(&mut self, backend: &mut R) {
        self.root_children.iter_mut().for_each(|child| {
            child.prepare(backend);
        });
    }

    pub(crate) fn render(&self, backend: &R, pass: &mut R::RenderPass<'_>) {
        self.root_children.iter().for_each(|child| {
            child.render(backend, pass);
        });
    }

    pub fn get_node_by_path<'a>(&'a self, path: &Utf8Path) -> Option<&'a Node<R>> {
        fn get_node_by_path_recursive<'b, R: RenderBackend>(
            iter: &mut dyn Iterator<Item = camino::Utf8Component<'_>>,
            current: &'b Node<R>,
        ) -> Option<&'b Node<R>> {
            if let Some(next) = iter.next() {
                for child in current.children.iter() {
                    if child.name == next.as_str() {
                        return get_node_by_path_recursive(iter, child);
                    }
                }

                None
            } else {
                Some(current)
            }
        }

        let mut iter = path.components();
        let next = iter.next()?;
        for child in self.root_children.iter() {
            if child.name == next.as_str() {
                return get_node_by_path_recursive(&mut iter, child);
            }
        }

        None
    }

    pub fn get_node_by_path_mut<'a>(&'a mut self, path: &Utf8Path) -> Option<&'a mut Node<R>> {
        fn get_node_by_path_recursive<'b, R: RenderBackend>(
            iter: &mut dyn Iterator<Item = camino::Utf8Component<'_>>,
            current: &'b mut Node<R>,
        ) -> Option<&'b mut Node<R>> {
            if let Some(next) = iter.next() {
                for child in current.children.iter_mut() {
                    if child.name == next.as_str() {
                        return get_node_by_path_recursive(iter, child);
                    }
                }

                None
            } else {
                Some(current)
            }
        }

        let mut iter = path.components();
        let next = iter.next()?;
        for child in self.root_children.iter_mut() {
            if child.name == next.as_str() {
                return get_node_by_path_recursive(&mut iter, child);
            }
        }

        None
    }

    pub fn remove_node_by_path(&mut self, path: &Utf8Path) {
        fn remove_by_path_recursive<R: RenderBackend>(
            iter: &mut dyn Iterator<Item = camino::Utf8Component<'_>>,
            current: &mut Node<R>,
        ) -> bool {
            if let Some(next) = iter.next() {
                let mut idx = 0;
                while idx < current.children.len() {
                    let child = &mut current.children[idx];
                    if child.name == next.as_str() {
                        if remove_by_path_recursive(iter, child) {
                            current.children.remove(idx);
                        }
                        break;
                    }
                    idx += 1;
                }
                false
            } else {
                true
            }
        }

        let mut iter = path.components();
        let Some(next) = iter.next() else {
            return;
        };

        let mut idx = 0;
        while idx < self.root_children.len() {
            let child = &mut self.root_children[idx];
            if child.name == next.as_str() {
                if remove_by_path_recursive(&mut iter, child) {
                    self.root_children.remove(idx);
                }
                break;
            }
            idx += 1;
        }
    }

    pub fn visit_all_nodes(&self, mut f: impl FnMut(&Node<R>)) {
        fn visit_recursive<R: RenderBackend>(node: &Node<R>, f: &mut dyn FnMut(&Node<R>)) {
            f(node);
            for child in node.children.iter() {
                visit_recursive(child, f);
            }
        }

        for child in self.root_children.iter() {
            visit_recursive(child, &mut f);
        }
    }

    pub fn visit_all_nodes_mut(&mut self, mut f: impl FnMut(&mut Node<R>)) {
        fn visit_recursive<R: RenderBackend>(node: &mut Node<R>, f: &mut dyn FnMut(&mut Node<R>)) {
            f(node);
            for child in node.children.iter_mut() {
                visit_recursive(child, f);
            }
        }

        for child in self.root_children.iter_mut() {
            visit_recursive(child, &mut f);
        }
    }

    pub fn new() -> Self {
        Self {
            root_children: vec![],
            images: vec![],
        }
    }

    pub fn with_child(mut self, child: Node<R>) -> Self {
        self.root_children.push(child);
        self
    }
}
