use std::collections::{HashMap, HashSet};

use camino::Utf8Path;
use glam::{Affine2, Vec2};

use crate::{
    animations::Animation, backend, node::{Anchor, NodeParent, ObservedNode, PropagationArgs}, template::{LayoutTemplate, NodeImplTemplate, NodeTemplate}, EnvyBackend, NodeItem, NodeTransform, SublayoutNode
};

pub struct LayoutRoot<B: EnvyBackend> {
    root_layout: LayoutTree<B>,
    root_template: LayoutTemplate,
    templates: HashMap<String, LayoutTemplate>,
}

impl<B: EnvyBackend> LayoutRoot<B> {
    fn validate_template_recursive(template: &NodeTemplate, templates: &HashMap<String, LayoutTemplate>, visited_layouts: &mut HashSet<String>) {
        if let NodeImplTemplate::Sublayout(sublayout) = &template.implementation {
            if visited_layouts.contains(&sublayout.sublayout_name) {
                panic!("Template {} is recursive", sublayout.sublayout_name);
            }

            visited_layouts.insert(sublayout.sublayout_name.clone());
            let layout = templates.get(&sublayout.sublayout_name).unwrap_or_else(|| panic!("Template {} is missing", sublayout.sublayout_name));
            for node in layout.root_nodes.iter() {
                Self::validate_template_recursive(node, templates, visited_layouts);
            }
            let _ = visited_layouts.remove(&sublayout.sublayout_name);
        }

        for child in template.children.iter() {
            Self::validate_template_recursive(child, templates, visited_layouts);
        }
    }

    pub fn from_root_template(template: LayoutTemplate, templates: impl IntoIterator<Item = (String, LayoutTemplate)>) -> Self {
        let mut this = Self {
            root_layout: LayoutTree::new(),
            root_template: template,
            templates: templates.into_iter().collect(),
        };

        this.templates.insert("".to_string(), LayoutTemplate::default());
        let tree = LayoutTree::from_template(&this.root_template, &this);
        this.root_layout = tree;

        let mut visited = HashSet::new();

        for template in [&this.root_template].into_iter().chain(this.templates.values()) {
            for node in template.root_nodes.iter() {
                Self::validate_template_recursive(node, &this.templates, &mut visited);
            }
        }

        this
    }

    pub fn new() -> Self {
        Self::from_root_template(LayoutTemplate::default(), [])
    }

    fn validate_template_on_insert(&self, node: &NodeTemplate) {
        if let NodeImplTemplate::Sublayout(sublayout) = &node.implementation {
            if !self.templates.contains_key(&sublayout.sublayout_name) {
                panic!("Sublayout template cannot reference other template which does not exist: {}", sublayout.sublayout_name);
            }
        }

        for child in node.children.iter() {
            self.validate_template_on_insert(child);
        }
    }

    fn sync_template_inner(tree: &mut LayoutTree<B>, template: &LayoutTemplate, templates: &HashMap<String, LayoutTemplate>, backend: &mut B) {
        tree.visit_roots_mut(|root| root.release(backend));
        *tree = LayoutTree::from_template_with_root_templates(template, templates);
        tree.visit_roots_mut(|root| root.setup(backend));
    }

    fn sync_template_inner_by_path(tree: &mut LayoutTree<B>, template: &LayoutTemplate, templates: &HashMap<String, LayoutTemplate>, path: &Utf8Path, backend: &mut B) {
        let node = tree.get_node_by_path_mut(path).unwrap();
        let template_node = template.get_node_by_path(path).unwrap();
        node.release(backend);
        *node = NodeItem::from_template_with_root_templates(template_node, templates);
        node.setup(backend);
    }

    pub fn sync_root_template(&mut self, backend: &mut B) {
        Self::sync_template_inner(&mut self.root_layout, &self.root_template, &self.templates, backend)
    }

    pub fn sync_root_template_by_path(&mut self, path: impl AsRef<Utf8Path>, backend: &mut B) {
        Self::sync_template_inner_by_path(&mut self.root_layout, &self.root_template, &self.templates, path.as_ref(), backend)
    }

    pub fn sync_template(&mut self, template_name: impl AsRef<str>, backend: &mut B) {
        let name = template_name.as_ref();
        let template = self.templates.get(name).unwrap();

        self.root_layout.walk_tree_mut(|node| {
            if let Some(sublayout) = node.downcast_mut::<SublayoutNode<B>>() {
                if sublayout.reference() == name {
                    Self::sync_template_inner(sublayout.as_layout_mut(), template, &self.templates, backend);
                }
            }
        });
    }

    pub fn sync_template_by_path(&mut self, template_name: impl AsRef<str>, path: impl AsRef<Utf8Path>, backend: &mut B) {
        let name = template_name.as_ref();
        let path = path.as_ref();
        let template = self.templates.get(name).unwrap();

        self.root_layout.walk_tree_mut(|node| {
            if let Some(sublayout) = node.downcast_mut::<SublayoutNode<B>>() {
                if sublayout.reference() == name {
                    Self::sync_template_inner_by_path(sublayout.as_layout_mut(), template, &self.templates, path, backend);
                }
            }
        });
    }

    pub fn root_template(&self) -> &LayoutTemplate {
        &self.root_template
    }

    pub fn root_template_mut(&mut self) -> &mut LayoutTemplate {
        &mut self.root_template
    }

    pub fn template(&self, name: impl AsRef<str>) -> Option<&LayoutTemplate> {
        self.templates.get(name.as_ref())
    }

    pub fn template_mut(&mut self, name: impl AsRef<str>) -> Option<&mut LayoutTemplate> {
        self.templates.get_mut(name.as_ref())
    }

    pub fn templates(&self) -> impl IntoIterator<Item = (&str, &LayoutTemplate)> {
        self.templates.iter().map(|(name, template)| (name.as_str(), template))
    }

    pub fn add_template(&mut self, template_name: impl Into<String>, template: LayoutTemplate) {
        for node in template.root_nodes.iter() {
            self.validate_template_on_insert(node);
        }

        self.templates.insert(template_name.into(), template);
    }

    fn rename_sublayout_reference(node: &mut NodeItem<B>, old_name: &str, new_name: &str) {
        if let Some(sublayout) = node.downcast_mut::<SublayoutNode<B>>() {
            if sublayout.reference() == old_name {
                sublayout.set_reference_no_update(new_name);
            } else {
                sublayout.as_layout_mut().visit_roots_mut(|root| {
                    Self::rename_sublayout_reference(root, old_name, new_name);
                });
            }
        }

        node.visit_children_mut(|child| Self::rename_sublayout_reference(child, old_name, new_name));
    }

    fn rename_sublayout_reference_in_template(node: &mut NodeTemplate, old_name: &str, new_name: &str) {
        if let NodeImplTemplate::Sublayout(sublayout) = &mut node.implementation {
            if sublayout.sublayout_name == old_name {
                sublayout.sublayout_name = new_name.to_string();
            }
        }

        for child in node.children.iter_mut() {
            Self::rename_sublayout_reference_in_template(child, old_name, new_name);
        }
    }

    pub fn rename_template(&mut self, old_name: impl AsRef<str>, new_name: impl Into<String>) {
        let old = old_name.as_ref();
        let new_name: String = new_name.into();
        if let Some(template) = self.templates.remove(old_name.as_ref()) {
            self.root_layout.visit_roots_mut(|root| {
                Self::rename_sublayout_reference(root, old, &new_name);
            });

            for (name, template) in self.templates.iter_mut() {
                if name != old {
                    for node in template.root_nodes.iter_mut() {
                        Self::rename_sublayout_reference_in_template(node, old, &new_name);
                    }
                }
            }

            self.templates.insert(new_name, template);
        }
    }

    pub fn instantiate_tree_from_template(&self, reference: impl AsRef<str>) -> Option<LayoutTree<B>> {
        let template = self.templates.get(reference.as_ref())?;

        let mut tree = LayoutTree {
            animations: template.animations.iter().map(|(name, animation)| (name.clone(), animation.clone())).collect(),
            playing_animations: HashMap::new(),
            root_children: Vec::with_capacity(template.root_nodes.len()),
        };

        for node in template.root_nodes.iter() {
            tree.root_children.push(ObservedNode::new(NodeItem::from_template(node, self)));
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
    animations: HashMap<String, Animation>,
    playing_animations: HashMap<String, f32>,
    root_children: Vec<ObservedNode<B>>,
}

impl<B: EnvyBackend> LayoutTree<B> {
    pub(crate) fn from_template_with_root_templates(template: &LayoutTemplate, templates: &HashMap<String, LayoutTemplate>) -> Self {
        Self {
            animations: template.animations.iter().map(|(name, anim)| (name.clone(), anim.clone())).collect(),
            playing_animations: HashMap::new(),
            root_children: template.root_nodes.iter().map(|template| ObservedNode::new(NodeItem::from_template_with_root_templates(template, templates))).collect(),
        }
    }

    pub fn sync_to_template(&mut self, template: &LayoutTemplate, root: &LayoutRoot<B>, backend: &mut B) {
        self.visit_roots_mut(|root| root.release(backend));
        *self = Self::from_template(template, root);
        self.setup(backend);
    }

    pub fn from_template(template: &LayoutTemplate, root: &LayoutRoot<B>) -> Self {
        Self::from_template_with_root_templates(template, &root.templates)
    }

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
        self
            .root_children
            .iter_mut()
            .for_each(|child| child.node.setup(backend));
    }

    pub fn update(&mut self) {
        NodeItem::update_batch(&mut self.root_children, NodeParent::Root);
    }

    pub fn prepare(&mut self, backend: &mut B) {
        self
            .root_children
            .iter_mut()
            .for_each(|child| child.node.prepare(backend));
    }

    pub fn render(&self, backend: &B, render_pass: &mut B::RenderPass<'_>) {
        self
            .root_children
            .iter()
            .for_each(|child| child.node.render(backend, render_pass));
    }

    pub fn update_animations(&mut self) {
        self.playing_animations.retain(|key, progress| {
            *progress += 1.0;
            if let Some(animation) = self.animations.get(key) {
                let mut should_keep = false;
                for node_anim in animation.node_animations.iter() {
                    let Some(node) = Self::get_node_by_path_mut_impl(
                        &mut self.root_children,
                        Utf8Path::new(&node_anim.node_path),
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
                    anim.node_path = new_path.to_string();
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
