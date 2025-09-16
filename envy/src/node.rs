use crate::{template::{NodeImplTemplate, NodeTemplate}, EnvyBackend, EnvyMaybeSendSync, LayoutRoot, LayoutTemplate, LayoutTree};
use glam::{Affine2, Mat4, Vec2, Vec4};
use serde::{Deserialize, Serialize};
use std::{
    any::Any, collections::HashMap, marker::PhantomData, ops::Deref, sync::atomic::{AtomicUsize, Ordering}
};

mod image;
mod text;
mod sublayout;

pub use image::ImageNode;
pub use text::TextNode;
pub use sublayout::SublayoutNode;

#[doc(hidden)]
mod __sealed {
    #[doc(hidden)]
    pub trait Sealed {}
}

/// Args used to prepare a node's uniform buffer
pub struct PreparationArgs<'a> {
    pub(crate) transform: &'a NodeTransform,
    pub(crate) affine: &'a Affine2,
    pub(crate) color: Vec4,
}

/// Trait that defines the base operations for nodes
///
/// This trait can only be implemented by the `envy` crate for now.
pub trait Node<B: EnvyBackend>: EnvyMaybeSendSync + __sealed::Sealed + 'static {
    /// Allows the node to be downcasted into it's implementors
    ///
    /// This allows more complex operations to be done to the UI Tree at runtime, such as
    /// referencing the texture a node points to
    fn as_any(&self) -> &dyn Any;

    /// Allows the node to be downcasted into it's implementors
    ///
    /// This allows more complex operations to be done to the UI Tree at runtime, such as
    /// changing the texture a node points to
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Initializes the resources required for the node to render and update
    ///
    /// It is an error to call either [`Node::prepare`] or [`Node::render`] before calling
    /// [`Node::setup_resources`]
    fn setup_resources(&mut self, backend: &mut B);

    /// Releases the resources
    ///
    /// It is still expected that calls to this node's methods succeed even if the resources have
    /// been released
    fn release_resources(&mut self, backend: &mut B);

    /// Prepares the render resources required for this node.
    ///
    /// This should be called after propagation. It is not an error to call them out of order,
    /// but the effects of propagation will be delayed by one render cycle if so
    fn prepare(&mut self, args: PreparationArgs<'_>, backend: &mut B);

    /// Renders this node to the screen
    fn render(&self, backend: &B, pass: &mut B::RenderPass<'_>);
}

#[derive(Debug, Copy, Clone)]
pub struct EmptyNode;

impl __sealed::Sealed for EmptyNode {}

impl<B: EnvyBackend> Node<B> for EmptyNode {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn setup_resources(&mut self, _: &mut B) {}

    fn release_resources(&mut self, _: &mut B) {}

    fn prepare(&mut self, _: PreparationArgs<'_>, _: &mut B) {}

    fn render(&self, _: &B, _: &mut <B as EnvyBackend>::RenderPass<'_>) {}
}

/// Positional anchor for a UI node
///
/// This is based on traditional screen coordinates, so "up" is `-Y` and right is `+X`.
///
/// For example [`Anchor::TopLeft`], when converted to a vector, is `[-0.5, -0.5]`
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum Anchor {
    #[default]
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
/// A node's positional information
#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
pub struct NodeTransform {
    /// Clockwise rotation of the node in degrees
    ///
    /// Rotation will happen around the center of the node, regardless of the node's anchor
    pub angle: f32,

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
}

impl NodeTransform {
    pub fn from_angle(angle: f32) -> Self {
        Self {
            angle,
            ..Default::default()
        }
    }

    pub fn from_size(size: Vec2) -> Self {
        Self {
            size,
            ..Default::default()
        }
    }

    pub fn from_scale(scale: Vec2) -> Self {
        Self {
            scale,
            ..Default::default()
        }
    }

    pub fn from_anchor(anchor: Anchor) -> Self {
        Self {
            anchor,
            ..Default::default()
        }
    }

    pub fn from_xy(x: f32, y: f32) -> Self {
        Self {
            position: Vec2::new(x, y),
            ..Default::default()
        }
    }

    pub fn from_position(pos: Vec2) -> Self {
        Self {
            position: pos,
            ..Default::default()
        }
    }

    pub fn with_xy(self, x: f32, y: f32) -> Self {
        Self {
            position: Vec2::new(x, y),
            ..self
        }
    }

    pub fn with_size(self, size: Vec2) -> Self {
        Self { size, ..self }
    }

    pub fn with_size_xy(self, w: f32, h: f32) -> Self {
        Self {
            size: Vec2::new(w, h),
            ..self
        }
    }

    pub fn with_angle(self, angle: f32) -> Self {
        Self { angle, ..self }
    }

    pub fn with_scale(self, scale: Vec2) -> Self {
        Self { scale, ..self }
    }

    pub fn with_scale_xy(self, scale_x: f32, scale_y: f32) -> Self {
        Self {
            scale: Vec2::new(scale_x, scale_y),
            ..self
        }
    }

    pub fn with_anchor(self, anchor: Anchor) -> Self {
        Self { anchor, ..self }
    }
}

impl Default for NodeTransform {
    fn default() -> Self {
        Self {
            angle: 0.0,
            position: Vec2::ZERO,
            size: Vec2::splat(50.0),
            scale: Vec2::ONE,
            anchor: Anchor::default(),
        }
    }
}

pub(crate) struct PropagationArgs<'a> {
    pub(crate) transform: &'a NodeTransform,
    pub(crate) affine: &'a Affine2,
    pub(crate) changed: bool,
}

pub(crate) struct ObservedNode<B: EnvyBackend> {
    pub(crate) node: NodeItem<B>,
    read_count: AtomicUsize,
    is_write: bool,
}

impl<B: EnvyBackend> ObservedNode<B> {
    pub fn new(node: NodeItem<B>) -> Self {
        Self {
            node,
            read_count: AtomicUsize::new(0),
            is_write: false,
        }
    }
}

pub struct ObservedRef<'a, B: EnvyBackend> {
    node: &'a NodeItem<B>,
    read_count: &'a AtomicUsize,
}

impl<B: EnvyBackend> Deref for ObservedRef<'_, B> {
    type Target = NodeItem<B>;

    fn deref(&self) -> &Self::Target {
        self.node
    }
}

impl<B: EnvyBackend> Drop for ObservedRef<'_, B> {
    fn drop(&mut self) {
        self.read_count.fetch_sub(1, Ordering::SeqCst);
    }
}

// Safety notes:
// This struct does not allow mutable access to the node's `name` field. This is because that is accessed
// by the disjoint access struct to allow tree traversal while updating nodes.
// Everything else is allowed to be accessed, by disallowing the access to write the `name`, we can avoid creating
// read references to the node when traversing (this is why we don't use RefCell here).
// We also want to be able to reference children here, so we don't mutably access those here (or anywhere)

pub struct ObservedMut<'a, B: EnvyBackend> {
    node: *mut NodeItem<B>,
    is_write: *mut bool,
    _phantom: PhantomData<&'a ()>,
}

impl<'a, B: EnvyBackend> ObservedMut<'a, B> {
    pub fn name(&self) -> &str {
        self.deref().name.as_str()
    }

    pub fn transform(&self) -> &NodeTransform {
        &self.deref().transform
    }

    pub fn transform_mut(&mut self) -> &mut NodeTransform {
        // SAFETY: We do not access `name`, so this is safe
        unsafe { &mut (*self.node).transform }
    }

    pub fn color(&self) -> [u8; 4] {
        self.deref().color
    }

    pub fn color_mut(&mut self) -> &mut [u8; 4] {
        // SAFETY: We do not access `name`, so this is safe
        unsafe { &mut (*self.node).color }
    }

    pub fn mark_changed(&mut self) {
        // SAFETY: We do not access `name`, so this is safe
        unsafe {
            (*self.node).was_changed = true;
        }
    }

    pub fn downcast<T: Node<B>>(&self) -> Option<&T> {
        self.deref().downcast::<T>()
    }

    pub fn downcast_mut<T: Node<B>>(&mut self) -> Option<&mut T> {
        // SAFETY: We do not access `name`, so this is safe
        unsafe { (*self.node).downcast_mut::<T>() }
    }
}

impl<B: EnvyBackend> Deref for ObservedMut<'_, B> {
    type Target = NodeItem<B>;

    fn deref(&self) -> &Self::Target {
        // SAFETY: We ensure through refcounting that this is unique
        unsafe { &*self.node }
    }
}

impl<B: EnvyBackend> Drop for ObservedMut<'_, B> {
    fn drop(&mut self) {
        // SAFETY: Only we have access to this pointer, and it was constructed from a reference
        // so this write is safe
        unsafe {
            *self.is_write = false;
        }
    }
}

pub(crate) enum NodeParent<'a, B: EnvyBackend> {
    Root,
    Node(&'a NodeDisjointAccessor<'a, B>),
}

impl<'a, B: EnvyBackend> Clone for NodeParent<'a, B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, B: EnvyBackend> Copy for NodeParent<'a, B> {}

pub struct NodeDisjointAccessor<'a, B: EnvyBackend> {
    parent: NodeParent<'a, B>,
    node_group: *mut [ObservedNode<B>],
    idx: usize,
}

impl<'a, B: EnvyBackend> Clone for NodeDisjointAccessor<'a, B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, B: EnvyBackend> Copy for NodeDisjointAccessor<'a, B> {}

impl<'a, B: EnvyBackend> NodeDisjointAccessor<'a, B> {
    pub fn self_ref(&self) -> ObservedRef<'a, B> {
        // SAFETY: We refcount below this
        let node = unsafe { &(*self.node_group)[self.idx] };

        // NOTE: (for this impl and all below it) accessing name is read only across both the ObservedRef and ObservedMut
        if node.is_write {
            panic!(
                "Cannot acquire read reference for node {} because there is a mutable reference",
                node.node.name
            );
        }

        node.read_count.fetch_add(1, Ordering::SeqCst);

        ObservedRef {
            node: &node.node,
            read_count: &node.read_count,
        }
    }

    pub fn self_mut(&self) -> ObservedMut<'a, B> {
        // SAFETY: We refcount below this
        let node = unsafe { &mut (*self.node_group)[self.idx] };

        if node.is_write {
            panic!(
                "Cannot acquire mutable reference for node {} because there is another write reference",
                node.node.name
            );
        } else if node.read_count.load(Ordering::SeqCst) > 0 {
            panic!(
                "Cannot acquire mutable reference for node {} because there is one or more read references",
                node.node.name
            );
        }

        node.is_write = true;

        ObservedMut {
            node: &mut node.node,
            is_write: &mut node.is_write,
            _phantom: PhantomData,
        }
    }

    pub fn parent(&self) -> Option<NodeDisjointAccessor<'a, B>> {
        match self.parent {
            NodeParent::Root => None,
            NodeParent::Node(parent) => Some(*parent),
        }
    }

    pub fn parent_ref(&self) -> Option<ObservedRef<'a, B>> {
        self.parent().map(|parent| parent.self_ref())
    }

    pub fn parent_mut(&self) -> Option<ObservedMut<'a, B>> {
        self.parent().map(|parent| parent.self_mut())
    }

    pub fn child(&self, name: impl AsRef<str>) -> Option<NodeDisjointAccessor<'_, B>> {
        // SAFETY: We restrict access to the node children so this is safe to be read only
        let node = unsafe { &(*self.node_group)[self.idx].node.children };

        let name = name.as_ref();
        for child in node.iter() {
            if child.node.name.eq(name) {
                let accessor = NodeDisjointAccessor {
                    parent: NodeParent::Node(self),
                    // SAFETY: We get a mut ptr here for the ObservedMut accessors,
                    //      but we never allow access
                    node_group: unsafe {
                        (*self.node_group)[self.idx].node.children.as_mut_slice()
                    },
                    idx: self.idx,
                };

                return Some(accessor);
            }
        }

        None
    }

    pub fn child_ref(&self, name: impl AsRef<str>) -> Option<ObservedRef<'_, B>> {
        self.child(name).map(|child| child.self_ref())
    }

    pub fn child_mut(&self, name: impl AsRef<str>) -> Option<ObservedMut<'_, B>> {
        self.child(name).map(|child| child.self_mut())
    }

    pub fn sibling(&self, name: impl AsRef<str>) -> Option<NodeDisjointAccessor<'a, B>> {
        let name = name.as_ref();
        unsafe {
            let idx = (*self.node_group)
                .iter()
                .position(|node| node.node.name.eq(name))?;
            Some(NodeDisjointAccessor {
                parent: self.parent,
                node_group: self.node_group,
                idx,
            })
        }
    }

    pub fn sibling_ref(&self, name: impl AsRef<str>) -> Option<ObservedRef<'_, B>> {
        self.sibling(name).map(|sibling| sibling.self_ref())
    }

    pub fn sibling_mut(&self, name: impl AsRef<str>) -> Option<ObservedMut<'_, B>> {
        self.sibling(name).map(|sibling| sibling.self_mut())
    }
}

pub trait NodeUpdateCallback<B: EnvyBackend>: EnvyMaybeSendSync + 'static {
    fn update(&mut self, node: NodeDisjointAccessor<'_, B>);
}

impl<B: EnvyBackend, F: for<'a> FnMut(NodeDisjointAccessor<'a, B>) + EnvyMaybeSendSync + 'static>
    NodeUpdateCallback<B> for F
{
    fn update(&mut self, node: NodeDisjointAccessor<'_, B>) {
        (self)(node)
    }
}

pub struct NodeItem<B: EnvyBackend> {
    name: String,
    children: Vec<ObservedNode<B>>,
    transform: NodeTransform,
    color: [u8; 4],
    affine: Affine2,
    was_changed: bool,
    node: Box<dyn Node<B>>,
    update: Vec<Box<dyn NodeUpdateCallback<B>>>,
}

impl<B: EnvyBackend> NodeItem<B> {
    pub(crate) fn from_template_with_root_templates(template: &NodeTemplate, templates: &HashMap<String, LayoutTemplate>) -> Self {
        Self {
            name: template.name.clone(),
            children: template.children.iter().map(|child| ObservedNode::new(NodeItem::from_template_with_root_templates(child, templates))).collect::<Vec<_>>(),
            transform: template.transform,
            color: template.color,
            affine: Affine2::IDENTITY,
            was_changed: true,
            node: match &template.implementation {
                NodeImplTemplate::Empty => Box::new(EmptyNode),
                NodeImplTemplate::Image(image) => Box::new(ImageNode::new(&image.texture_name)),
                NodeImplTemplate::Text(text) => Box::new(TextNode::new(&text.font_name, text.font_size, text.line_height, &text.text)),
                NodeImplTemplate::Sublayout(sublayout) => Box::new(SublayoutNode::new(&sublayout.sublayout_name, LayoutTree::from_template_with_root_templates(templates.get(&sublayout.sublayout_name).unwrap(), templates))),
            },
            update: vec![]
        }
    }

    pub fn from_template(template: &NodeTemplate, root: &LayoutRoot<B>) -> Self {
        Self {
            name: template.name.clone(),
            children: template.children.iter().map(|child| ObservedNode::new(NodeItem::from_template(child, root))).collect::<Vec<_>>(),
            transform: template.transform,
            color: template.color,
            affine: Affine2::IDENTITY,
            was_changed: true,
            node: match &template.implementation {
                NodeImplTemplate::Empty => Box::new(EmptyNode),
                NodeImplTemplate::Image(image) => Box::new(ImageNode::new(&image.texture_name)),
                NodeImplTemplate::Text(text) => Box::new(TextNode::new(&text.font_name, text.font_size, text.line_height, &text.text)),
                NodeImplTemplate::Sublayout(sublayout) => Box::new(SublayoutNode::new(&sublayout.sublayout_name, root.instantiate_tree_from_template(&sublayout.sublayout_name).unwrap())),
            },
            update: vec![]
        }
    }

    pub fn new_boxed(
        name: impl Into<String>,
        transform: NodeTransform,
        color: [u8; 4],
        node: Box<dyn Node<B>>,
    ) -> Self {
        Self {
            name: name.into(),
            children: vec![],
            transform,
            color,
            affine: Affine2::IDENTITY,
            was_changed: true,
            node,
            update: vec![],
        }
    }

    pub fn new(
        name: impl Into<String>,
        transform: NodeTransform,
        color: [u8; 4],
        node: impl Node<B>,
    ) -> Self {
        Self::new_boxed(name, transform, color, Box::new(node))
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn affine(&self) -> &Affine2 {
        &self.affine
    }

    pub fn transform(&self) -> &NodeTransform {
        &self.transform
    }

    pub fn transform_mut(&mut self) -> &mut NodeTransform {
        self.was_changed = true;
        &mut self.transform
    }

    pub fn color(&self) -> [u8; 4] {
        self.color
    }

    pub fn with_on_update(mut self, callback: impl NodeUpdateCallback<B>) -> Self {
        self.update.push(Box::new(callback));
        self
    }

    pub fn add_on_update(&mut self, callback: impl NodeUpdateCallback<B>) {
        self.update.push(Box::new(callback));
    }

    pub fn set_implementation(&mut self, node: impl Node<B>) {
        self.node = Box::new(node);
    }

    pub fn is<T: Node<B>>(&self) -> bool {
        self.node.as_any().is::<T>()
    }

    pub fn downcast<T: Node<B>>(&self) -> Option<&T> {
        self.node.as_any().downcast_ref::<T>()
    }

    pub fn downcast_mut<T: Node<B>>(&mut self) -> Option<&mut T> {
        self.node.as_any_mut().downcast_mut::<T>()
    }

    pub fn has_child(&self, name: impl AsRef<str>) -> bool {
        let name = name.as_ref();
        self.children.iter().any(|child| child.node.name.eq(name))
    }

    pub fn child(&self, name: impl AsRef<str>) -> Option<&NodeItem<B>> {
        let name = name.as_ref();
        self.children
            .iter()
            .find(|child| child.node.name.eq(name))
            .map(|node| &node.node)
    }

    pub fn child_mut(&mut self, name: impl AsRef<str>) -> Option<&mut NodeItem<B>> {
        let name = name.as_ref();
        self.children
            .iter_mut()
            .find(|child| child.node.name.eq(name))
            .map(|node| &mut node.node)
    }

    pub fn visit_children<'a>(&'a self, f: impl FnMut(&'a NodeItem<B>)) {
        self.children.iter().map(|node| &node.node).for_each(f);
    }

    pub fn visit_children_mut(&mut self, f: impl FnMut(&mut NodeItem<B>)) {
        self.children
            .iter_mut()
            .map(|node| &mut node.node)
            .for_each(f);
    }

    #[must_use = "This method can fail if there is another child with the same name"]
    pub fn add_child(&mut self, new_node: NodeItem<B>) -> bool {
        if self
            .children
            .iter()
            .any(|child| child.node.name == new_node.name)
        {
            return false;
        }

        self.children.push(ObservedNode::new(new_node));

        true
    }

    // crate private so that user must go through layout to ensure all things are properly update
    pub(crate) fn remove_child(&mut self, name: &str) -> Option<NodeItem<B>> {
        Self::remove_child_impl(&mut self.children, name)
    }

    // crate private so that user must go through layout to ensure all things are properly update
    pub(crate) fn remove_child_impl(
        group: &mut Vec<ObservedNode<B>>,
        name: &str,
    ) -> Option<NodeItem<B>> {
        let pos = group.iter().position(|node| node.node.name.eq(name))?;
        Some(group.remove(pos).node)
    }

    #[must_use = "This method can fail if the child with the specified name was not found"]
    pub(crate) fn move_child_backward(&mut self, name: &str) -> bool {
        Self::move_child_backward_impl(&mut self.children, name)
    }

    #[must_use = "This method can fail if the child with the specified name was not found"]
    pub(crate) fn move_child_forward(&mut self, name: &str) -> bool {
        Self::move_child_forward_impl(&mut self.children, name)
    }

    #[must_use = "This method can fail if the child with the specified name was not found"]
    pub(crate) fn move_child_backward_impl(group: &mut [ObservedNode<B>], name: &str) -> bool {
        let Some(pos) = group.iter().position(|node| node.node.name.eq(name)) else {
            return false;
        };

        if pos > 0 {
            group.swap(pos, pos - 1);
        }

        true
    }

    #[must_use = "This method can fail if the child with the specified name was not found"]
    pub(crate) fn move_child_forward_impl(group: &mut [ObservedNode<B>], name: &str) -> bool {
        let Some(pos) = group.iter().position(|node| node.node.name.eq(name)) else {
            return false;
        };

        if pos + 1 < group.len() {
            group.swap(pos, pos + 1);
        }

        true
    }

    // crate private so that user must go through layout to ensure all things are properly updated
    #[must_use = "This method can fail if the old name was not found or there is another child with the same name"]
    pub(crate) fn rename_child(&mut self, old_name: &str, new_name: String) -> bool {
        Self::rename_child_impl(&mut self.children, old_name, new_name)
    }

    // crate private so that user must go through layout to ensure all things are properly updated
    #[must_use = "This method can fail if the old name was not found or there is another child with the same name"]
    pub(crate) fn rename_child_impl(
        group: &mut [ObservedNode<B>],
        old_name: &str,
        new_name: String,
    ) -> bool {
        if group.iter().any(|node| node.node.name.eq(&new_name)) {
            return false;
        }

        let Some(child) = group.iter_mut().find(|node| node.node.name.eq(old_name)) else {
            return false;
        };

        child.node.name = new_name;
        true
    }

    pub(crate) fn setup(&mut self, backend: &mut B) {
        self.node.setup_resources(backend);
        self.children
            .iter_mut()
            .for_each(|child| child.node.setup(backend));
    }

    pub(crate) fn release(&mut self, backend: &mut B) {
        self.node.release_resources(backend);
        self.children.iter_mut().for_each(|child| child.node.release(backend));
    }

    pub(crate) fn propagate(&mut self, parent: PropagationArgs<'_>) {
        let did_change = self.was_changed || parent.changed;
        if did_change {
            self.was_changed = true;
            let actual_size = self.transform.size * self.transform.scale;
            let parent_anchor_to_origin = parent.transform.size * parent.transform.anchor.as_vec();
            let self_translation = parent_anchor_to_origin + self.transform.position;
            let center = self_translation + -self.transform.anchor.as_vec() * actual_size;

            self.affine = *parent.affine
                * Affine2::from_scale_angle_translation(
                    self.transform.scale,
                    self.transform.angle.to_radians(),
                    center,
                );
        }

        if let Some(sublayout) = self.node.as_any_mut().downcast_mut::<SublayoutNode<B>>() {
            sublayout.propagate_with_root_transform(&self.transform, did_change);
        }

        self.children.iter_mut().for_each(|child| {
            child.node.propagate(PropagationArgs {
                affine: &self.affine,
                transform: &self.transform,
                changed: did_change,
            });
        });
    }

    pub(crate) fn prepare(&mut self, backend: &mut B) {
        if self.was_changed {
            self.was_changed = false;
            self.node.prepare(
                PreparationArgs {
                    transform: &self.transform,
                    affine: &self.affine,
                    color: Vec4::from_array(self.color.map(|c| c as f32 / 255.0)),
                },
                backend,
            );
        }
        self.children
            .iter_mut()
            .for_each(|child| child.node.prepare(backend));
    }

    pub(crate) fn render(&self, backend: &B, render_pass: &mut B::RenderPass<'_>) {
        self.node.render(backend, render_pass);
        self.children
            .iter()
            .for_each(|child| child.node.render(backend, render_pass))
    }

    pub(crate) fn update_batch(group: &mut [ObservedNode<B>], parent: NodeParent<'_, B>) {
        for idx in 0..group.len() {
            let mut callbacks = std::mem::take(&mut group[idx].node.update);
            let accessor = NodeDisjointAccessor {
                parent,
                node_group: group,
                idx,
            };
            for callback in callbacks.iter_mut() {
                callback.update(accessor);
            }
            Self::update_batch(&mut group[idx].node.children, NodeParent::Node(&accessor));
            group[idx].node.update = callbacks;
        }
    }
}

fn affine2_to_mat4(affine: Affine2) -> Mat4 {
    glam::Mat4::from(glam::Affine3A::from_mat3_translation(
        glam::Mat3::from_mat2(affine.matrix2),
        affine.translation.extend(0.0),
    ))
}
