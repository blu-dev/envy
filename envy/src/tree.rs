use std::collections::HashMap;

use camino::Utf8Path;
use glam::{Affine2, Vec2};

use crate::{
    animations::Animation, node::{Anchor, NodeParent, ObservedNode, PropagationArgs}, EmptyNode, EnvyBackend, ImageNode, Node, NodeItem, NodeTransform, SublayoutNode, TextNode
};

pub enum NodeImplTemplate {
    Empty,
    Image {
        texture_name: String,
    },
    Text {
        font_name: String,
        text: String,
        font_size: f32,
        line_height: f32,
    },
    Sublayout {
        sublayout_reference: String,
    }
}

impl NodeImplTemplate {
    fn instantiate_node_impl<B: EnvyBackend>(&self, root: &LayoutRoot<B>) -> Box<dyn Node<B>> {
        match self {
            Self::Empty => Box::new(EmptyNode),
            Self::Image { texture_name } => Box::new(ImageNode::new(texture_name)),
            Self::Text { font_name, text, font_size, line_height } => Box::new(TextNode::new(font_name, *font_size, *line_height, text)),
            Self::Sublayout { sublayout_reference } => Box::new(SublayoutNode::new(sublayout_reference, root.instantiate_tree_from_template(sublayout_reference).unwrap_or_else(|| {
                panic!("Failed to instantiate sublayout {sublayout_reference} -- missing");
            }))),
        }
    }
}

pub struct NodeTemplate {
    name: String,
    children: Vec<NodeTemplate>,
    transform: NodeTransform,
    color: [u8; 4],
    node: NodeImplTemplate,
}

impl NodeTemplate {
    pub fn new(name: impl Into<String>, transform: NodeTransform, color: [u8; 4], node: NodeImplTemplate) -> Self {
        Self {
            name: name.into(),
            children: vec![],
            transform,
            color,
            node,
        }
    }

    pub fn add_child(&mut self, node: NodeTemplate) {
        assert!(!self.children.iter().any(|other_node| node.name == other_node.name), "{} already exists as child of {}", node.name, self.name);

        self.children.push(node);
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn children(&self) -> &[NodeTemplate] {
        &self.children
    }

    pub fn transform(&self) -> NodeTransform {
        self.transform
    }

    pub fn color(&self) -> [u8; 4] {
        self.color
    }

    pub fn implementation(&self) -> &NodeImplTemplate {
        &self.node
    }

    fn instantiate_node<B: EnvyBackend>(&self, root: &LayoutRoot<B>) -> NodeItem<B> {
        let mut node = NodeItem::new_boxed(
            &self.name,
            self.transform,
            self.color,
            self.node.instantiate_node_impl(root),
        );

        for child in self.children.iter() {
            assert!(node.add_child(child.instantiate_node(root)));
        }

        node
    }
}

pub struct LayoutTemplate {
    animations: HashMap<String, Animation>,
    nodes: Vec<NodeTemplate>,
}

impl LayoutTemplate {
    pub fn new() -> Self {
        Self {
            animations: HashMap::new(),
            nodes: vec![],
        }
    }

    pub fn root_nodes(&self) -> &[NodeTemplate] {
        &self.nodes
    }

    pub fn animations(&self) -> &HashMap<String, Animation> {
        &self.animations
    }

    pub fn add_root_node(&mut self, node: NodeTemplate) {
        assert!(!self.nodes.iter().any(|other_node| node.name == other_node.name), "{} already exists in template root", node.name);

        self.nodes.push(node);
    }

    pub fn add_animation(&mut self, name: impl Into<String>, animation: Animation) {
        self.animations.insert(name.into(), animation);
    }
}


impl Default for LayoutTemplate {
    fn default() -> Self {
        LayoutTemplate::new()
    }
}

pub struct LayoutRoot<B: EnvyBackend> {
    root_layout: LayoutTree<B>,
    templates: HashMap<String, LayoutTemplate>,
}

impl<B: EnvyBackend> LayoutRoot<B> {
    pub fn new() -> Self {
        Self {
            root_layout: LayoutTree::new(),
            templates: HashMap::new(),
        }
    }

    fn validate_template_on_insert(&self, node: &NodeTemplate) {
        if let NodeImplTemplate::Sublayout { sublayout_reference } = &node.node {
            if !self.templates.contains_key(sublayout_reference) {
                panic!("Sublayout template cannot reference other template which does not exist: {sublayout_reference}");
            }
        }

        for child in node.children.iter() {
            self.validate_template_on_insert(child);
        }
    }

    pub fn templates(&self) -> impl IntoIterator<Item = (&str, &LayoutTemplate)> {
        self.templates.iter().map(|(name, template)| (name.as_str(), template))
    }

    pub fn add_template(&mut self, template_name: impl Into<String>, template: LayoutTemplate) {
        for node in template.nodes.iter() {
            self.validate_template_on_insert(node);
        }

        self.templates.insert(template_name.into(), template);
    }

    pub fn instantiate_tree_from_template(&self, reference: impl AsRef<str>) -> Option<LayoutTree<B>> {
        let template = self.templates.get(reference.as_ref())?;

        let mut tree = LayoutTree {
            animations: template.animations.clone(),
            playing_animations: HashMap::new(),
            root_children: Vec::with_capacity(template.nodes.len()),
        };

        for node in template.nodes.iter() {
            tree.root_children.push(ObservedNode::new(node.instantiate_node(self)));
        }

        Some(tree)
    }

    pub fn as_layout(&self) -> &LayoutTree<B> {
        &self.root_layout
    }

    pub fn as_layout_mut(&mut self) -> &mut LayoutTree<B> {
        &mut self.root_layout
    }

    pub fn setup(&mut self, backend: &mut B) {
        self
            .root_layout
            .root_children
            .iter_mut()
            .for_each(|child| child.node.setup(backend));
    }

    pub fn update(&mut self) {
        NodeItem::update_batch(&mut self.root_layout.root_children, NodeParent::Root);
    }

    pub fn prepare(&mut self, backend: &mut B) {
        self.root_layout
            .root_children
            .iter_mut()
            .for_each(|child| child.node.prepare(backend));
    }

    pub fn render(&self, backend: &B, render_pass: &mut B::RenderPass<'_>) {
        self.root_layout
            .root_children
            .iter()
            .for_each(|child| child.node.render(backend, render_pass));
    }
}

impl<B: EnvyBackend> Default for LayoutRoot<B> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LayoutTree<B: EnvyBackend> {
    // TODO: Should this be pub? Seems fine to provid direct access
    pub animations: HashMap<String, Animation>,
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

    pub fn propagate_with_root_transform(&mut self, transform: &NodeTransform, changed: bool) {
        let position = transform.position + -transform.anchor.as_vec() * transform.scale * transform.size;
        let affine = Affine2::from_scale_angle_translation(transform.scale, transform.angle, position);

        self.root_children.iter_mut().for_each(|child| {
            child.node.propagate(PropagationArgs {
                transform,
                affine: &affine,
                changed
            })
        });
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

    pub fn get_node_by_path(&self, path: impl AsRef<Utf8Path>) -> Option<&NodeItem<B>> {
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

    pub fn get_node_by_path_mut(
        &mut self,
        path: impl AsRef<Utf8Path>,
    ) -> Option<&mut NodeItem<B>> {
        Self::get_node_by_path_mut_impl(&mut self.root_children, path.as_ref())
    }

    pub fn has_root(&self, name: impl AsRef<str>) -> bool {
        let name = name.as_ref();
        self.root_children
            .iter()
            .any(|node| node.node.name() == name)
    }

    pub fn walk_tree(&self, mut f: impl FnMut(&NodeItem<B>)) {
        fn walk_node_recursive<B: EnvyBackend>(
            node: &NodeItem<B>,
            f: &mut dyn FnMut(&NodeItem<B>),
        ) {
            f(node);
            node.visit_children(|child| {
                walk_node_recursive(child, f);
            });
        }

        self.visit_roots(|node| walk_node_recursive(node, &mut f));
    }

    pub fn walk_tree_mut(&mut self, mut f: impl FnMut(&mut NodeItem<B>)) {
        fn walk_node_recursive<B: EnvyBackend>(
            node: &mut NodeItem<B>,
            f: &mut dyn FnMut(&mut NodeItem<B>),
        ) {
            f(node);
            node.visit_children_mut(|child| {
                walk_node_recursive(child, f);
            });
        }

        self.visit_roots_mut(|node| walk_node_recursive(node, &mut f));
    }

    pub fn visit_roots<'a>(&'a self, f: impl FnMut(&'a NodeItem<B>)) {
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

    #[must_use = "This method can fail if one or more of the nodes in the path are missing"]
    pub fn move_node_backward_by_path(&mut self, path: impl AsRef<Utf8Path>) -> bool {
        let path = path.as_ref();

        let parent_path = match path.parent() {
            // Special case root
            Some(path) if matches!(path.as_str(), "/" | "") => None,
            other => other,
        };

        // TODO: Validate that all node paths actually have a name. Not sure how to do this other than runtime checks
        // and real error messages
        let Some(name) = path.file_name() else {
            return false;
        };

        match parent_path {
            Some(parent_path) => {
                let Some(parent_node) = self.get_node_by_path_mut(parent_path) else {
                    return false;
                };

                parent_node.move_child_backward(name)
            }
            None => NodeItem::move_child_backward_impl(&mut self.root_children, name),
        }
    }

    #[must_use = "This method can fail if one or more of the nodes in the path are missing"]
    pub fn move_node_forward_by_path(&mut self, path: impl AsRef<Utf8Path>) -> bool {
        let path = path.as_ref();

        let parent_path = match path.parent() {
            // Special case root
            Some(path) if matches!(path.as_str(), "/" | "") => None,
            other => other,
        };

        // TODO: Validate that all node paths actually have a name. Not sure how to do this other than runtime checks
        // and real error messages
        let Some(name) = path.file_name() else {
            return false;
        };

        match parent_path {
            Some(parent_path) => {
                let Some(parent_node) = self.get_node_by_path_mut(parent_path) else {
                    return false;
                };

                parent_node.move_child_forward(name)
            }
            None => NodeItem::move_child_forward_impl(&mut self.root_children, name),
        }
    }
}

impl<B: EnvyBackend> Default for LayoutTree<B> {
    fn default() -> Self {
        Self::new()
    }
}
