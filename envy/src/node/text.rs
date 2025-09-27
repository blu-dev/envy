use std::any::Any;

use glam::{Affine2, Vec2};

use crate::{
    node::{affine2_to_mat4, PreparationArgs},
    DrawUniform, EnvyBackend, Node, PreparedGlyph, TextLayoutArgs,
};

pub struct TextNode<B: EnvyBackend> {
    font_name: String,
    font_size: f32,
    line_height: f32,
    text: String,
    font: Option<B::FontHandle>,
    glyphs: Vec<PreparedGlyph<B>>,
    needs_compute: bool,
    outline_thickness: f32,
    outline_color: [u8; 4],
}

impl<B: EnvyBackend> TextNode<B> {
    pub fn new(
        font_name: impl Into<String>,
        font_size: f32,
        line_height: f32,
        text: impl Into<String>,
    ) -> Self {
        Self {
            font_name: font_name.into(),
            font_size,
            line_height,
            text: text.into(),
            font: None,
            glyphs: vec![],
            needs_compute: true,
            outline_thickness: 0.0,
            outline_color: [255; 4]
        }
    }

    pub fn font_name(&self) -> &str {
        self.font_name.as_str()
    }

    pub fn set_font_name(&mut self, name: impl Into<String>) {
        self.font_name = name.into();
        self.invalidate_font_handle();
    }

    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    pub fn set_font_size(&mut self, font_size: f32) {
        if self.font_size != font_size {
            self.font_size = font_size;
            self.needs_compute = true;
        }
    }

    pub fn line_height(&self) -> f32 {
        self.line_height
    }

    pub fn set_line_height(&mut self, line_height: f32) {
        if self.line_height != line_height {
            self.line_height = line_height;
            self.needs_compute = true;
        }
    }

    pub fn text(&self) -> &str {
        self.text.as_str()
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    pub fn outline_thickness(&self) -> f32 {
        self.outline_thickness
    }

    pub fn set_outline_thickness(&mut self, thickness: f32) {
        self.outline_thickness = thickness;
        self.needs_compute = true;
    }

    pub fn outline_color(&self) -> [u8; 4] {
        self.outline_color
    }

    pub fn set_outline_color(&mut self, color: [u8; 4]) {
        self.outline_color = color;
    }

    pub fn invalidate_font_handle(&mut self) {
        self.font = None;
        self.needs_compute = true;
    }
}

impl<B: EnvyBackend> super::__sealed::Sealed for TextNode<B> {}

impl<B: EnvyBackend> Node<B> for TextNode<B> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn setup_resources(&mut self, backend: &mut B) {
        if self.font.is_none() {
            self.font = backend.request_font_by_name(&self.font_name);
        }

        if self.font.is_none() {
            log::warn!(
                "TextNode::setup_resources failed to acquire font (font '{}')",
                self.font_name
            );
        }
    }

    fn release_resources(&mut self, backend: &mut B) {
        if let Some(font) = self.font.take() {
            backend.release_font(font);
        }

        for glyph in self.glyphs.drain(..) {
            backend.release_uniform(glyph.uniform_handle);
        }
    }

    fn prepare(&mut self, args: PreparationArgs<'_>, backend: &mut B) {
        if self.needs_compute {
            if self.font.is_none() {
                self.font = backend.request_font_by_name(&self.font_name);
            }

            let Some(font_handle) = self.font else {
                log::error!(
                    "TextNode::prepare called without font handle set (font '{}')",
                    self.font_name
                );
                return;
            };

            if self.font_size <= 0.0
                || self.line_height <= 0.0
                || args.transform.size.cmple(Vec2::ZERO).any()
            {
                log::info!(
                    "TextNode::prepare skipping text layout since one or more required parameters are <= 0.0"
                );
                return;
            }

            for glyph in self.glyphs.drain(..) {
                backend.release_uniform(glyph.uniform_handle);
            }

            self.glyphs = backend.layout_text(TextLayoutArgs {
                handle: font_handle,
                font_size: self.font_size,
                line_height: self.line_height,
                buffer_size: args.transform.size,
                text: &self.text,
                outline_thickness: self.outline_thickness,
            });

            self.needs_compute = false;
        }

        for glyph in self.glyphs.iter() {
            let center = (-args.transform.size / 2.0) + glyph.offset_in_buffer + glyph.size / 2.0;

            let matrix = affine2_to_mat4(*args.affine * Affine2::from_translation(center));
            backend.update_uniform(glyph.uniform_handle, DrawUniform::new(matrix, args.color));

            if let Some(handle) = glyph.outline_uniform_handle {
                backend.update_uniform(handle, DrawUniform::new(matrix, glam::Vec4::from_array(self.outline_color.map(|c| c as f32 / 255.0))));
            }
        }
    }

    fn render(&self, backend: &B, pass: &mut <B as EnvyBackend>::RenderPass<'_>) {
        backend.draw_glyphs(
            self.glyphs
                .iter()
                .map(|glyph| (glyph.uniform_handle, glyph.outline_uniform_handle, glyph.glyph_handle)),
            pass,
        );
    }
}
