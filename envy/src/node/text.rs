use std::any::Any;

use glam::{Affine2, Vec2};

use crate::{
    DrawUniform, EnvyBackend, Node, PreparedGlyph, TextLayoutArgs,
    node::{PreparationArgs, affine2_to_mat4},
};

pub struct TextNode<B: EnvyBackend> {
    font_name: String,
    font_size: f32,
    line_height: f32,
    text: String,
    font: Option<B::FontHandle>,
    glyphs: Vec<PreparedGlyph<B>>,
    needs_compute: bool,
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

    fn prepare(&mut self, args: PreparationArgs<'_>, backend: &mut B) {
        if self.needs_compute {
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
            });

            self.needs_compute = false;
        }

        for glyph in self.glyphs.iter() {
            let center = -args.transform.size + glyph.offset_in_buffer + glyph.size / 2.0;

            let matrix = affine2_to_mat4(*args.affine * Affine2::from_translation(center));
            backend.update_uniform(glyph.uniform_handle, DrawUniform::new(matrix, args.color));
        }
    }

    fn render(&self, backend: &B, pass: &mut <B as EnvyBackend>::RenderPass<'_>) {
        backend.draw_glyphs(
            self.glyphs
                .iter()
                .map(|glyph| (glyph.uniform_handle, glyph.glyph_handle)),
            pass,
        );
    }
}
