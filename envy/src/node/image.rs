use glam::Affine2;

use crate::{
    node::{affine2_to_mat4, PreparationArgs}, DrawTextureArgs, DrawUniform, EnvyBackend, Node
};

pub struct ImageNode<B: EnvyBackend> {
    name: String,
    mask_texture_name: Option<String>,
    uniform: Option<B::UniformHandle>,
    texture: Option<B::TextureHandle>,
    mask_texture: Option<B::TextureHandle>,
}

impl<B: EnvyBackend> ImageNode<B> {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            mask_texture_name: None,
            uniform: None,
            texture: None,
            mask_texture: None,
        }
    }

    pub fn resource_name(&self) -> &str {
        self.name.as_str()
    }

    pub fn set_resource_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
        self.invalidate_image_handle();
    }

    pub fn mask_texture_name(&self) -> Option<&str> {
        self.mask_texture_name.as_deref()
    }

    pub fn set_mask_texture_name(&mut self, name: impl Into<Option<String>>) {
        self.mask_texture_name = name.into();
        self.invalidate_mask_handle();
    }

    pub fn invalidate_image_handle(&mut self) {
        self.texture = None;
    }

    pub fn invalidate_mask_handle(&mut self) {
        self.mask_texture = None;
    }
}

impl<B: EnvyBackend> super::__sealed::Sealed for ImageNode<B> {}

impl<B: EnvyBackend> Node<B> for ImageNode<B> {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn setup_resources(&mut self, backend: &mut B) {
        if self.uniform.is_none() {
            self.uniform = backend.request_new_uniform();
        }

        if self.texture.is_none() {
            self.texture = backend.request_texture_by_name(&self.name);
        }

        if let Some(name) = self.mask_texture_name.as_ref() {
            self.mask_texture = backend.request_texture_by_name(name);
        }

        if self.uniform.is_none() {
            log::warn!(
                "ImageNode::setup_resources failed to acquire uniform buffer from backend (image '{}')",
                self.name
            );
        }

        if self.texture.is_none() {
            log::warn!(
                "ImageNode::setup_resources failed to acquire texture from backend (image '{}')",
                self.name
            );
        }

        if let Some(name) = self.mask_texture_name.as_ref() {
            if self.mask_texture.is_none() {
                log::warn!(
                    "ImageNode::setup_resources failed to acquire mask texture from backend (image '{}')",
                    name
                );
            }
        }
    }

    fn release_resources(&mut self, backend: &mut B) {
        if let Some(uniform) = self.uniform.take() {
            backend.release_uniform(uniform);
        }

        if let Some(texture) = self.texture.take() {
            backend.release_texture(texture);
        }
    }

    fn prepare(&mut self, args: PreparationArgs<'_>, backend: &mut B) {
        if self.texture.is_none() {
            self.texture = backend.request_texture_by_name(&self.name);

            if self.texture.is_none() {
                log::warn!(
                    "ImageNode::setup_resources failed to acquire texture from backend (image '{}')",
                    self.name
                );
            }
        }

        let Some(uniform) = self.uniform else {
            log::error!(
                "ImageNode::prepare called without uniform buffer being set (image '{}')",
                self.name
            );
            return;
        };

        let matrix = affine2_to_mat4(*args.affine * Affine2::from_scale(args.transform.size));
        backend.update_uniform(uniform, DrawUniform::new(matrix, args.color));
    }

    fn render(&self, backend: &B, pass: &mut <B as EnvyBackend>::RenderPass<'_>) {
        let Some(uniform) = self.uniform else {
            log::error!(
                "ImageNode::render called without uniform buffer being set (image '{}')",
                self.name
            );
            return;
        };

        let Some(texture) = self.texture else {
            log::error!(
                "ImageNode::render called without texture being set (image '{}')",
                self.name
            );
            return;
        };

        let mask_texture = if let Some(mask_image_name) = self.mask_texture_name.as_ref() {
            let Some(mask_texture) = self.mask_texture else {
                log::error!("ImageNode::render called without mask texture being set (image '{}')", mask_image_name);
                return;
            };

            Some(mask_texture)
        } else {
            None
        };

        backend.draw_texture_ext(uniform, DrawTextureArgs {
            texture,
            mask_texture,
        }, pass);
    }
}
