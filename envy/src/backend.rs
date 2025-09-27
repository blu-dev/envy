use crate::{DrawUniform, EnvyMaybeSendSync, ImageScalingMode};

pub struct TextureRequestArgs {
    pub scaling_x: ImageScalingMode,
    pub scaling_y: ImageScalingMode,
}

/// Arguments passed to an [`EnvyBackend`] implementor to layout text
pub struct TextLayoutArgs<'a, R: EnvyBackend> {
    /// Handle to the font to generate the layout for
    pub handle: R::FontHandle,

    /// Size of the font
    pub font_size: f32,

    /// Height of the lines in the layout
    pub line_height: f32,

    /// Size of the buffer to lay the text out inside of
    pub buffer_size: glam::Vec2,

    /// Text to render, there is no support for rich text at this time
    pub text: &'a str,

    /// Thickness of the outline
    pub outline_thickness: f32,
}

/// Glyph information prepared by the render backend
pub struct PreparedGlyph<R: EnvyBackend> {
    /// The handle to inform the render backend what glyph is being rendered in a call to [`EnvyBackend::draw_glyph`]
    pub glyph_handle: R::GlyphHandle,

    /// The handle to the uniform buffer for the glyph
    pub uniform_handle: R::UniformHandle,

    /// The handle to the outline uniform, should basically just be used for the color
    pub outline_uniform_handle: Option<R::UniformHandle>,

    /// Offset of the glyph's origin point (top left) in the text buffer
    pub offset_in_buffer: glam::Vec2,

    /// Size of the glyph
    pub size: glam::Vec2,
}

pub struct DrawTextureArgs<B: EnvyBackend> {
    pub texture: B::TextureHandle,
    pub mask_texture: Option<B::TextureHandle>,
}

/// Abstractions over rendering APIs
///
/// This trait allows `envy` layouts to be agnostic to the rendering APIs being used behind the scenes. This trait is
/// merely a way for the [`UiTree`](crate::UiTree) to initialize GPU data. It does not drive a renderer, and it is up
/// to the user of the backend to ensure that data is properly flushed to the GPU before rendering.
///
/// The design of this trait was originally based on the limitations of `envy`'s integration with Super Smash Bros.
/// Ultimate's pre-built shaders, hence some of the strange API decisions.
///
/// IMPLEMENTOR NOTE: There are some expectations when it comes to the way vertex buffers are staged
/// - Vertex buffers for textures are assumed to have all vertices in the range of `[-0.5, 0.5]` on both axes
/// - Vertex buffers for font glyphs are assumed that all vertices are in the range of `[-w / 2.0, w / 2.0]` and
///   `[-h / 2.0, h / 2.0]`, where `w` and `h` are the width and height of the glyph
pub trait EnvyBackend: Sized + EnvyMaybeSendSync + 'static {
    /// Handle for accessing textures
    type TextureHandle: Copy + Clone + EnvyMaybeSendSync + 'static;

    /// Handle for accesing uniforms
    type UniformHandle: Copy + Clone + EnvyMaybeSendSync + 'static;

    /// Handle for referencing font data
    type FontHandle: Copy + Clone + EnvyMaybeSendSync + 'static;

    /// Handle for referencing glyphs
    type GlyphHandle: Copy + Clone + EnvyMaybeSendSync + 'static;

    /// Type passed through to the backend for calls to draw functions
    type RenderPass<'a>;

    /// Requests a handle for the texture with the given `name`
    fn request_texture_by_name(&mut self, name: impl AsRef<str>, args: TextureRequestArgs) -> Option<Self::TextureHandle>;

    /// Requests a handle for the font with the given `name`
    fn request_font_by_name(&mut self, name: impl AsRef<str>) -> Option<Self::FontHandle>;

    /// Request a handle for a currently unused uniform buffer.
    fn request_new_uniform(&mut self) -> Option<Self::UniformHandle>;

    /// Informs the backend that it can release the specified texture, as it is no longer being used.
    ///
    /// IMPLEMENTOR NOTE: If the backend provides the same texture handle multiple times via [`EnvyBackend::request_texture_by_name`],
    /// then the UI tree can release the same texture as many times as it's given to the ui tree
    fn release_texture(&mut self, handle: Self::TextureHandle);

    /// Informs the backend that it can release the specified font, as it is no longe rbeing used.
    ///
    /// IMPLEMENTOR NOTE: If the backend provides the same font handle multiple times via [`EnvyBackend::request_font_by_name`],
    /// then the UI tree can release the same font as many times as it's given to the ui tree
    fn release_font(&mut self, handle: Self::FontHandle);

    /// Informs the backend that it can release the specified uniform, as it is no longer being used.
    ///
    /// IMPLEMENTOR NOTE: If the backend provides the same uniform handle multiple times via [`EnvyBackend::request_new_uniform`],
    /// then the UI tree can release the same uniform as many times as it's given to the ui tree
    fn release_uniform(&mut self, handle: Self::UniformHandle);

    /// Informs the backend that it should update the specified uniform.
    ///
    /// IMPLEMENTOR NOTE: This method is called in a separate stage of the UI Tree than the render step, so you can wait to
    /// flush the changes to the GPU until you are sure that it is safe to do so.
    fn update_uniform(&mut self, handle: Self::UniformHandle, uniform: DrawUniform);

    /// Informs the backend of the computed size of the texture, so that texture coords can be updated
    ///
    /// This is only useful if the [`ImageScalingMode`] is set to something other than [`ImageScalingMode::Stretch`]
    fn update_texture_scaling(&mut self, handle: Self::TextureHandle, uv_offset: glam::Vec2, uv_scale: glam::Vec2, size: glam::Vec2);

    /// Requests the backend to render the provided text
    ///
    /// IMPLEMENTOR NOTE: The instance of [`EnvyBackend::GlyphHandle`] can be shared across many invocations of this method,
    /// but the [`EnvyBackend::UniformHandle`] should be unique for each glyph laid out this way.
    fn layout_text(&mut self, args: TextLayoutArgs<'_, Self>) -> Vec<PreparedGlyph<Self>>;

    /// Draws a texture with the provided handle and uniform to the screen
    fn draw_texture(
        &self,
        uniform: Self::UniformHandle,
        handle: Self::TextureHandle,
        pass: &mut Self::RenderPass<'_>,
    );

    /// Draws a texture with advanced arguments
    fn draw_texture_ext(
        &self,
        uniform: Self::UniformHandle,
        args: DrawTextureArgs<Self>,
        pass: &mut Self::RenderPass<'_>,
    );

    /// Draws a glyph with the provided handle and uniform to the screen
    fn draw_glyph(
        &self,
        uniform: Self::UniformHandle,
        outline_uniform: Option<Self::UniformHandle>,
        handle: Self::GlyphHandle,
        pass: &mut Self::RenderPass<'_>,
    );

    /// Draws multiple glyphs to the screen at the same time
    ///
    /// The default implementation is ignorant of any optimizations that could be made by the backend
    fn draw_glyphs(
        &self,
        uniforms_and_glyphs: impl IntoIterator<Item = (Self::UniformHandle, Option<Self::UniformHandle>, Self::GlyphHandle)>,
        pass: &mut Self::RenderPass<'_>,
    ) {
        for (uniform, outline, glyph) in uniforms_and_glyphs {
            self.draw_glyph(uniform, outline, glyph, pass);
        }
    }
}
