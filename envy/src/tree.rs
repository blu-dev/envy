use std::collections::HashMap;

use camino::Utf8Path;
use glam::{Affine2, Vec2};

use crate::{
    EnvyBackend, NodeItem, NodeTransform,
    animations::Animation,
    node::{Anchor, NodeParent, ObservedNode, PropagationArgs},
};

pub struct LayoutTree<B: EnvyBackend> {
    animations: HashMap<String, Animation>,
    playing_animations: HashMap<String, f32>,
    root_children: Vec<ObservedNode<B>>,
}

impl<B: EnvyBackend> LayoutTree<B> {
    pub fn new() -> Self {
        Self {
            animations: HashMap::new(),
            playing_animations: HashMap::new(),
            root_children: vec![],
        }
    }

    pub fn play_animation(&mut self, name: impl AsRef<str>) {
        let name = name.as_ref();
        if self.animations.contains_key(name) {
            self.playing_animations.insert(name.to_string(), 0.0);
        }
    }

    pub fn add_animation(&mut self, name: impl Into<String>, animation: Animation) {
        self.animations.insert(name.into(), animation);
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

    pub fn update_animations(&mut self) {
        self.playing_animations.retain(|key, progress| {
            *progress += 1.0;
            if let Some(animation) = self.animations.get(key) {
                let mut should_keep = false;
                for node_anim in animation.node_animations.iter() {
                    let Some(node) = Self::get_node_by_path_mut_impl(
                        &mut self.root_children,
                        &node_anim.node_path,
                    ) else {
                        continue;
                    };

                    should_keep |= !node_anim.animate(*progress, node.transform_mut());
                }

                should_keep
            } else {
                false
            }
        });
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

    pub fn get_node_by_path<'a>(&'a self, path: impl AsRef<Utf8Path>) -> Option<&'a NodeItem<B>> {
        fn get_node_by_path_recursive<'a, B: EnvyBackend>(
            current: &'a NodeItem<B>,
            components: &mut dyn Iterator<Item = camino::Utf8Component<'_>>,
        ) -> Option<&'a NodeItem<B>> {
            let Some(next) = components.next() else {
                return Some(current);
            };

            let next = current.child(next.as_str())?;
            get_node_by_path_recursive(next, components)
        }

        let path = path.as_ref();
        let mut iter = path.components();
        let first = iter.next()?;

        for child in self.root_children.iter() {
            if child.node.name().eq(first.as_str()) {
                return get_node_by_path_recursive(&child.node, &mut iter);
            }
        }

        None
    }

    fn get_node_by_path_recursive<'a>(
        current: &'a mut NodeItem<B>,
        components: &mut dyn Iterator<Item = camino::Utf8Component<'_>>,
    ) -> Option<&'a mut NodeItem<B>> {
        let Some(next) = components.next() else {
            return Some(current);
        };

        let next = current.child_mut(next.as_str())?;
        Self::get_node_by_path_recursive(next, components)
    }

    #[inline(always)]
    fn get_node_by_path_mut_impl<'a>(
        nodes: &'a mut [ObservedNode<B>],
        path: &Utf8Path,
    ) -> Option<&'a mut NodeItem<B>> {
        let mut iter = path.components();
        let first = iter.next()?;

        for child in nodes.iter_mut() {
            if child.node.name().eq(first.as_str()) {
                return Self::get_node_by_path_recursive(&mut child.node, &mut iter);
            }
        }

        None
    }

    pub fn get_node_by_path_mut<'a>(
        &'a mut self,
        path: impl AsRef<Utf8Path>,
    ) -> Option<&'a mut NodeItem<B>> {
        Self::get_node_by_path_mut_impl(&mut self.root_children, path.as_ref())
    }

    pub fn has_root(&self, name: impl AsRef<str>) -> bool {
        let name = name.as_ref();
        self.root_children
            .iter()
            .any(|node| node.node.name() == name)
    }

    pub fn visit_roots(&self, f: impl FnMut(&NodeItem<B>)) {
        self.root_children.iter().map(|node| &node.node).for_each(f);
    }

    pub fn visit_roots_mut(&mut self, f: impl FnMut(&mut NodeItem<B>)) {
        self.root_children
            .iter_mut()
            .map(|node| &mut node.node)
            .for_each(f);
    }

    /// Renames the node in the layout
    ///
    /// This includes updating all existing animations that refer to this node to refer to the new name
    #[must_use = "This method can fail if the provided node was not found or if the new name was already in use"]
    pub fn rename_node(&mut self, path: impl AsRef<Utf8Path>, new_name: impl Into<String>) -> bool {
        let path = path.as_ref();
        let new_name = new_name.into();

        let parent_path = match path.parent() {
            // Special case root
            Some(path) if matches!(path.as_str(), "/" | "") => None,
            other => other,
        };

        // TODO: validate????
        let Some(old_name) = path.file_name() else {
            return false;
        };

        match parent_path {
            Some(parent_path) => {
                let Some(parent_node) = self.get_node_by_path_mut(parent_path) else {
                    return false;
                };

                if !parent_node.rename_child(old_name, new_name.clone()) {
                    return false;
                }
            }
            None => {
                if !NodeItem::rename_child_impl(&mut self.root_children, old_name, new_name.clone())
                {
                    return false;
                }
            }
        }

        let new_path = path.with_file_name(&new_name);
        for animation in self.animations.values_mut() {
            animation.node_animations.iter_mut().for_each(|anim| {
                if anim.node_path == path {
                    anim.node_path = new_path.clone();
                }
            });
        }

        true
    }

    pub fn remove_node(&mut self, path: impl AsRef<Utf8Path>) -> Option<NodeItem<B>> {
        let path = path.as_ref();

        let parent_path = match path.parent() {
            // Special case root
            Some(path) if matches!(path.as_str(), "/" | "") => None,
            other => other,
        };

        // TODO: Validate that all node paths actually have a name. Not sure how to do this other than runtime checks
        // and real error messages
        let name = path.file_name()?;

        let node = match parent_path {
            Some(parent_path) => {
                let parent_node = self.get_node_by_path_mut(parent_path)?;

                parent_node.remove_child(name)?
            }
            None => NodeItem::remove_child_impl(&mut self.root_children, name)?,
        };

        for animation in self.animations.values_mut() {
            animation
                .node_animations
                .retain(|animation| animation.node_path != path);
        }

        Some(node)
    }
}
