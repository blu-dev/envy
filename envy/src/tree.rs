use glam::{Affine2, Vec2};

use crate::{
    EnvyBackend, NodeItem, NodeTransform,
    node::{Anchor, NodeParent, ObservedNode, PropagationArgs},
};

pub struct LayoutTree<B: EnvyBackend> {
    root_children: Vec<ObservedNode<B>>,
}

impl<B: EnvyBackend> LayoutTree<B> {
    pub fn new() -> Self {
        Self {
            root_children: vec![],
        }
    }

    pub fn add_child(&mut self, node: NodeItem<B>) {
        self.root_children.push(ObservedNode::new(node));
    }

    pub fn with_child(mut self, node: NodeItem<B>) -> Self {
        self.root_children.push(ObservedNode::new(node));
        self
    }

    pub fn setup(&mut self, backend: &mut B) {
        self.root_children
            .iter_mut()
            .for_each(|child| child.node.setup(backend));
    }

    pub fn update(&mut self) {
        NodeItem::update_batch(&mut self.root_children, NodeParent::Root);
    }

    pub fn propagate(&mut self) {
        const TRANSFORM: NodeTransform = NodeTransform {
            angle: 0.0,
            position: Vec2::new(960.0, 540.0),
            size: Vec2::new(1920.0, 1080.0),
            scale: Vec2::ONE,
            anchor: Anchor::Center,
        };

        const AFFINE: Affine2 = Affine2::IDENTITY;

        self.root_children.iter_mut().for_each(|child| {
            child.node.propagate(PropagationArgs {
                transform: &TRANSFORM,
                affine: &AFFINE,
                changed: false,
            });
        });
    }

    pub fn prepare(&mut self, backend: &mut B) {
        self.root_children
            .iter_mut()
            .for_each(|child| child.node.prepare(backend));
    }

    pub fn render(&self, backend: &B, render_pass: &mut B::RenderPass<'_>) {
        self.root_children
            .iter()
            .for_each(|child| child.node.render(backend, render_pass));
    }
}
