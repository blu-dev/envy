use glam::Affine2;

use crate::{EnvyBackend, LayoutTree, Node, NodeTransform};

pub struct SublayoutNode<B: EnvyBackend> {
    reference: String,
    tree: LayoutTree<B>,
}

impl<B: EnvyBackend> SublayoutNode<B> {
    pub fn new(reference: impl Into<String>, tree: LayoutTree<B>) -> Self {
        Self {
            reference: reference.into(),
            tree
        }
    }

    pub fn set_reference_no_update(&mut self, new_reference: impl Into<String>) {
        self.reference = new_reference.into();
    }

    pub fn reference(&self) -> &str {
        self.reference.as_str()
    }

    pub fn as_layout(&self) -> &LayoutTree<B> {
        &self.tree
    }

    pub fn as_layout_mut(&mut self) -> &mut LayoutTree<B> {
        &mut self.tree
    }

    pub(crate) fn propagate_with_root_transform(&mut self, transform: &NodeTransform, affine: &Affine2, changed: bool) {
        self.tree.propagate_with_root_transform(transform, affine, changed);
    }
}

impl<B: EnvyBackend> super::__sealed::Sealed for SublayoutNode<B> {}

impl<B: EnvyBackend> Node<B> for SublayoutNode<B> {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn setup_resources(&mut self, backend: &mut B) {
        self.tree.visit_roots_mut(|node| node.setup(backend));
    }

    fn release_resources(&mut self, backend: &mut B) {
        self.tree.visit_roots_mut(|node| node.release(backend));
    }

    fn prepare(&mut self, _args: super::PreparationArgs<'_>, backend: &mut B) {
        self.tree.visit_roots_mut(|node| node.prepare(backend));
    }

    fn render(&self, backend: &B, pass: &mut <B as EnvyBackend>::RenderPass<'_>) {
        self.tree.visit_roots(|node| node.render(backend, pass));
    }
}
