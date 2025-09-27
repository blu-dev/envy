use camino::Utf8Path;

use crate::{Animation, NodeTransform};

#[cfg_attr(feature = "asset", derive(bincode::Encode, bincode::Decode))]
#[derive(Copy, Clone, Default, Debug)]
pub enum ImageScalingMode {
    #[default]
    Stretch,
    Tiling
}

#[derive(Clone)]
pub struct ImageNodeTemplate {
    pub texture_name: String,
    pub mask_texture_name: Option<String>,
    pub image_scaling_mode_x: ImageScalingMode,
    pub image_scaling_mode_y: ImageScalingMode,
    pub uv_offset: glam::Vec2,
    pub uv_scale: glam::Vec2,
}

#[cfg(feature = "asset")]
const _: () = {
    use bincode::{BorrowDecode, Decode, Encode};

    impl Encode for ImageNodeTemplate {
        fn encode<E: bincode::enc::Encoder>(&self, encoder: &mut E) -> Result<(), bincode::error::EncodeError> {
            self.texture_name.encode(encoder)?;
            self.mask_texture_name.encode(encoder)?;
            self.image_scaling_mode_x.encode(encoder)?;
            self.image_scaling_mode_y.encode(encoder)?;
            self.uv_offset.to_array().encode(encoder)?;
            self.uv_scale.to_array().encode(encoder)
        }
    }

    impl<'de, C> BorrowDecode<'de, C> for ImageNodeTemplate {
        fn borrow_decode<D: bincode::de::BorrowDecoder<'de, Context = C>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            Decode::decode(decoder)
        }
    }

    impl<C> Decode<C> for ImageNodeTemplate {
        fn decode<D: bincode::de::Decoder<Context = C>>(decoder: &mut D) -> Result<Self, bincode::error::DecodeError> {
            Ok(Self {
                texture_name: Decode::decode(decoder)?,
                mask_texture_name: Decode::decode(decoder)?,
                image_scaling_mode_x: Decode::decode(decoder)?,
                image_scaling_mode_y: Decode::decode(decoder)?,
                uv_offset: <[f32; 2]>::decode(decoder)?.into(),
                uv_scale: <[f32; 2]>::decode(decoder)?.into(),
            })
        }
    }
};


#[cfg_attr(feature = "asset", derive(bincode::Encode, bincode::Decode))]
#[derive(Clone)]
pub struct TextNodeTemplate {
    pub font_name: String,
    pub text: String,
    pub font_size: f32,
    pub line_height: f32,
}

#[cfg_attr(feature = "asset", derive(bincode::Encode, bincode::Decode))]
#[derive(Clone)]
pub struct SublayoutNodeTemplate {
    pub sublayout_name: String,
}

#[cfg_attr(feature = "asset", derive(bincode::Encode, bincode::Decode))]
#[derive(Clone)]
pub enum NodeImplTemplate {
    Empty,
    Image(ImageNodeTemplate),
    Text(TextNodeTemplate),
    Sublayout(SublayoutNodeTemplate),
}

#[cfg_attr(feature = "asset", derive(bincode::Encode, bincode::Decode))]
#[derive(Debug, Copy, Clone, Default)]
pub enum NodeVisibility {
    Hidden,
    #[default]
    Inherited,
    Visible,
}

#[cfg_attr(feature = "asset", derive(bincode::Encode, bincode::Decode))]
#[derive(Clone)]
pub struct NodeTemplate {
    pub name: String,
    pub transform: NodeTransform,
    pub color: [u8; 4],
    pub visibility: NodeVisibility,
    pub children: Vec<NodeTemplate>,
    pub implementation: NodeImplTemplate,
}

impl NodeTemplate {
    pub fn has_child(&self, name: impl AsRef<str>) -> bool {
        let name = name.as_ref();
        self.children.iter().any(|child| child.name.eq(name))
    }

    pub fn child(&self, name: impl AsRef<str>) -> Option<&Self> {
        let name = name.as_ref();
        self.children.iter().find(|child| child.name.eq(name))
    }

    pub fn child_mut(&mut self, name: impl AsRef<str>) -> Option<&mut Self> {
        let name = name.as_ref();
        self.children.iter_mut().find(|child| child.name.eq(name))
    }

    pub fn visit_children(&self, f: impl FnMut(&Self)) {
        self.children.iter().for_each(f);
    }

    pub fn visit_children_mut(&mut self, f: impl FnMut(&mut Self)) {
        self.children.iter_mut().for_each(f);
    }

    #[must_use = "This method can fail if there is another child with the same name"]
    pub fn insert_child_first(&mut self, node: Self) -> bool {
        if self.children.iter().any(|child| child.name == node.name) {
            return false;
        }

        self.children.insert(0, node);

        true
    }

    #[must_use = "This method can fail if there is another child with the same name, or no node with the target name"]
    pub fn insert_child_before(&mut self, node: Self, target: &str) -> bool {
        if self.children.iter().any(|child| child.name == node.name) {
            return false;
        }

        let Some(pos) = self.children.iter().position(|child| child.name == target) else {
            return false;
        };

        self.children.insert(pos, node);

        true
    }

    #[must_use = "This method can fail if there is another child with the same name, or no node with the target name"]
    pub fn insert_child_after(&mut self, node: Self, target: &str) -> bool {
        if self.children.iter().any(|child| child.name == node.name) {
            return false;
        }

        let Some(pos) = self.children.iter().position(|child| child.name == target) else {
            return false;
        };

        self.children.insert(pos + 1, node);

        true
    }

    #[must_use = "This method can fail if there is another child with the same name"]
    pub fn add_child(&mut self, new_node: Self) -> bool {
        if self
            .children
            .iter()
            .any(|child| child.name == new_node.name)
        {
            return false;
        }

        self.children.push(new_node);

        true
    }

    // crate private so that user must go through layout to ensure all things are properly update
    pub(crate) fn remove_child(&mut self, name: &str) -> Option<Self> {
        Self::remove_child_impl(&mut self.children, name)
    }

    // crate private so that user must go through layout to ensure all things are properly update
    pub(crate) fn remove_child_impl(group: &mut Vec<Self>, name: &str) -> Option<Self> {
        let pos = group.iter().position(|node| node.name.eq(name))?;
        Some(group.remove(pos))
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
    pub(crate) fn move_child_backward_impl(group: &mut [Self], name: &str) -> bool {
        let Some(pos) = group.iter().position(|node| node.name.eq(name)) else {
            return false;
        };

        if pos > 0 {
            group.swap(pos, pos - 1);
        }

        true
    }

    #[must_use = "This method can fail if the child with the specified name was not found"]
    pub(crate) fn move_child_forward_impl(group: &mut [Self], name: &str) -> bool {
        let Some(pos) = group.iter().position(|node| node.name.eq(name)) else {
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
    pub(crate) fn rename_child_impl(group: &mut [Self], old_name: &str, new_name: String) -> bool {
        if group.iter().any(|node| node.name.eq(&new_name)) {
            return false;
        }

        let Some(child) = group.iter_mut().find(|node| node.name.eq(old_name)) else {
            return false;
        };

        child.name = new_name;
        true
    }
}

#[cfg(feature = "asset")]
const _: () = {
    use crate::node::Anchor;

    #[derive(bincode::Encode, bincode::Decode)]
    struct NodeTransformRepr {
        angle: f32,
        position: [f32; 2],
        size: [f32; 2],
        scale: [f32; 2],
        anchor: AnchorRepr,
    }

    #[derive(bincode::Encode, bincode::Decode)]
    enum AnchorRepr {
        TopLeft,
        TopCenter,
        TopRight,
        CenterLeft,
        Center,
        CenterRight,
        BottomLeft,
        BottomCenter,
        BottomRight,
        Custom([f32; 2]),
    }

    impl From<Anchor> for AnchorRepr {
        fn from(value: Anchor) -> Self {
            match value {
                Anchor::TopLeft => Self::TopLeft,
                Anchor::TopCenter => Self::TopCenter,
                Anchor::TopRight => Self::TopRight,
                Anchor::CenterLeft => Self::CenterLeft,
                Anchor::Center => Self::Center,
                Anchor::CenterRight => Self::CenterRight,
                Anchor::BottomLeft => Self::BottomLeft,
                Anchor::BottomCenter => Self::BottomCenter,
                Anchor::BottomRight => Self::BottomRight,
                Anchor::Custom(custom) => Self::Custom(custom.into()),
            }
        }
    }

    impl From<AnchorRepr> for Anchor {
        fn from(value: AnchorRepr) -> Self {
            match value {
                AnchorRepr::TopLeft => Self::TopLeft,
                AnchorRepr::TopCenter => Self::TopCenter,
                AnchorRepr::TopRight => Self::TopRight,
                AnchorRepr::CenterLeft => Self::CenterLeft,
                AnchorRepr::Center => Self::Center,
                AnchorRepr::CenterRight => Self::CenterRight,
                AnchorRepr::BottomLeft => Self::BottomLeft,
                AnchorRepr::BottomCenter => Self::BottomCenter,
                AnchorRepr::BottomRight => Self::BottomRight,
                AnchorRepr::Custom(custom) => Self::Custom(custom.into()),
            }
        }
    }

    impl bincode::Encode for Anchor {
        fn encode<E: bincode::enc::Encoder>(
            &self,
            encoder: &mut E,
        ) -> Result<(), bincode::error::EncodeError> {
            AnchorRepr::from(*self).encode(encoder)
        }
    }

    impl<'de, C> bincode::BorrowDecode<'de, C> for Anchor {
        fn borrow_decode<D: bincode::de::BorrowDecoder<'de, Context = C>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            bincode::Decode::decode(decoder)
        }
    }

    impl<C> bincode::Decode<C> for Anchor {
        fn decode<D: bincode::de::Decoder<Context = C>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            AnchorRepr::decode(decoder).map(Self::from)
        }
    }

    impl From<NodeTransform> for NodeTransformRepr {
        fn from(value: NodeTransform) -> Self {
            Self {
                angle: value.angle,
                position: value.position.into(),
                size: value.size.into(),
                scale: value.scale.into(),
                anchor: value.anchor.into(),
            }
        }
    }

    impl From<NodeTransformRepr> for NodeTransform {
        fn from(value: NodeTransformRepr) -> Self {
            Self {
                angle: value.angle,
                position: value.position.into(),
                size: value.size.into(),
                scale: value.scale.into(),
                anchor: value.anchor.into(),
            }
        }
    }

    impl bincode::Encode for NodeTransform {
        fn encode<E: bincode::enc::Encoder>(
            &self,
            encoder: &mut E,
        ) -> Result<(), bincode::error::EncodeError> {
            NodeTransformRepr::from(*self).encode(encoder)
        }
    }

    impl<'de, C> bincode::BorrowDecode<'de, C> for NodeTransform {
        fn borrow_decode<D: bincode::de::BorrowDecoder<'de, Context = C>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            bincode::Decode::decode(decoder)
        }
    }

    impl<C> bincode::Decode<C> for NodeTransform {
        fn decode<D: bincode::de::Decoder<Context = C>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            NodeTransformRepr::decode(decoder).map(Self::from)
        }
    }
};

pub enum MoveNodePosition<'a> {
    First,
    Before(&'a str),
    After(&'a str),
    Last,
}

#[cfg_attr(feature = "asset", derive(bincode::Encode, bincode::Decode))]
#[derive(Default, Clone)]
pub struct LayoutTemplate {
    pub canvas_size: [u32; 2],
    pub root_nodes: Vec<NodeTemplate>,
    pub animations: Vec<(String, Animation)>,
}

impl LayoutTemplate {
    pub fn add_animation(&mut self, name: impl Into<String>, animation: Animation) {
        self.animations.push((name.into(), animation));
    }

    #[must_use = "This method can fail if there is another child with the same name"]
    pub fn insert_child_first(&mut self, node: NodeTemplate) -> bool {
        if self.root_nodes.iter().any(|child| child.name == node.name) {
            return false;
        }

        self.root_nodes.insert(0, node);

        true
    }

    #[must_use = "This method can fail if there is another child with the same name, or no node with the target name"]
    pub fn insert_child_before(&mut self, node: NodeTemplate, target: &str) -> bool {
        if self.root_nodes.iter().any(|child| child.name == node.name) {
            return false;
        }

        let Some(pos) = self
            .root_nodes
            .iter()
            .position(|child| child.name == target)
        else {
            return false;
        };

        self.root_nodes.insert(pos, node);

        true
    }

    #[must_use = "This method can fail if there is another child with the same name, or no node with the target name"]
    pub fn insert_child_after(&mut self, node: NodeTemplate, target: &str) -> bool {
        if self.root_nodes.iter().any(|child| child.name == node.name) {
            return false;
        }

        let Some(pos) = self
            .root_nodes
            .iter()
            .position(|child| child.name == target)
        else {
            return false;
        };

        self.root_nodes.insert(pos + 1, node);

        true
    }

    pub fn add_child(&mut self, node: NodeTemplate) {
        self.root_nodes.push(node);
    }

    pub fn with_child(mut self, node: NodeTemplate) -> Self {
        self.root_nodes.push(node);
        self
    }

    pub fn get_node_by_path(&self, path: impl AsRef<Utf8Path>) -> Option<&NodeTemplate> {
        fn get_node_by_path_recursive<'a>(
            current: &'a NodeTemplate,
            components: &mut dyn Iterator<Item = camino::Utf8Component<'_>>,
        ) -> Option<&'a NodeTemplate> {
            let Some(next) = components.next() else {
                return Some(current);
            };

            let next = current.child(next.as_str())?;
            get_node_by_path_recursive(next, components)
        }

        let path = path.as_ref();
        let mut iter = path.components();
        let first = iter.next()?;

        for child in self.root_nodes.iter() {
            if child.name.eq(first.as_str()) {
                return get_node_by_path_recursive(child, &mut iter);
            }
        }

        None
    }

    fn get_node_by_path_recursive<'a>(
        current: &'a mut NodeTemplate,
        components: &mut dyn Iterator<Item = camino::Utf8Component<'_>>,
    ) -> Option<&'a mut NodeTemplate> {
        let Some(next) = components.next() else {
            return Some(current);
        };

        let next = current.child_mut(next.as_str())?;
        Self::get_node_by_path_recursive(next, components)
    }

    #[inline(always)]
    fn get_node_by_path_mut_impl<'a>(
        nodes: &'a mut [NodeTemplate],
        path: &Utf8Path,
    ) -> Option<&'a mut NodeTemplate> {
        let mut iter = path.components();
        let first = iter.next()?;

        for child in nodes.iter_mut() {
            if child.name.eq(first.as_str()) {
                return Self::get_node_by_path_recursive(child, &mut iter);
            }
        }

        None
    }

    pub fn get_node_by_path_mut(
        &mut self,
        path: impl AsRef<Utf8Path>,
    ) -> Option<&mut NodeTemplate> {
        Self::get_node_by_path_mut_impl(&mut self.root_nodes, path.as_ref())
    }

    pub fn has_root(&self, name: impl AsRef<str>) -> bool {
        let name = name.as_ref();
        self.root_nodes.iter().any(|node| node.name == name)
    }

    pub fn walk_tree(&self, mut f: impl FnMut(&NodeTemplate)) {
        fn walk_node_recursive(node: &NodeTemplate, f: &mut dyn FnMut(&NodeTemplate)) {
            f(node);
            node.visit_children(|child| {
                walk_node_recursive(child, f);
            });
        }

        self.visit_roots(|node| walk_node_recursive(node, &mut f));
    }

    pub fn walk_tree_mut(&mut self, mut f: impl FnMut(&mut NodeTemplate)) {
        fn walk_node_recursive(node: &mut NodeTemplate, f: &mut dyn FnMut(&mut NodeTemplate)) {
            f(node);
            node.visit_children_mut(|child| {
                walk_node_recursive(child, f);
            });
        }

        self.visit_roots_mut(|node| walk_node_recursive(node, &mut f));
    }

    pub fn visit_roots<'a>(&'a self, f: impl FnMut(&'a NodeTemplate)) {
        self.root_nodes.iter().for_each(f);
    }

    pub fn visit_roots_mut(&mut self, f: impl FnMut(&mut NodeTemplate)) {
        self.root_nodes.iter_mut().for_each(f);
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
                if !NodeTemplate::rename_child_impl(
                    &mut self.root_nodes,
                    old_name,
                    new_name.clone(),
                ) {
                    return false;
                }
            }
        }

        let new_path = path.with_file_name(&new_name);
        for (_, animation) in self.animations.iter_mut() {
            animation.node_animations.iter_mut().for_each(|anim| {
                if anim.node_path == path {
                    anim.node_path = new_path.to_string();
                }
            });
        }

        true
    }

    pub fn move_node(
        &mut self,
        old_path: impl AsRef<Utf8Path>,
        new_path: impl AsRef<Utf8Path>,
        pos: MoveNodePosition,
    ) {
        let old_path = old_path.as_ref();
        let new_path = new_path.as_ref();

        let old_parent_path = match old_path.parent() {
            // Special case root
            Some(path) if matches!(path.as_str(), "/" | "") => None,
            other => other,
        };

        let new_parent_path = match new_path.parent() {
            // Special case root
            Some(path) if matches!(path.as_str(), "/" | "") => None,
            other => other,
        };

        // TODO: Validate that all node paths actually have a name. Not sure how to do this other than runtime checks
        // and real error messages
        let name = old_path.file_name().unwrap();

        let node = match old_parent_path {
            Some(parent_path) => {
                let parent_node = self.get_node_by_path_mut(parent_path).unwrap();

                parent_node.remove_child(name).unwrap()
            }
            None => NodeTemplate::remove_child_impl(&mut self.root_nodes, name).unwrap(),
        };

        match new_parent_path {
            Some(parent_path) => {
                let parent_node = self.get_node_by_path_mut(parent_path).unwrap();

                match pos {
                    MoveNodePosition::First => assert!(parent_node.insert_child_first(node)),
                    MoveNodePosition::Before(name) => {
                        assert!(parent_node.insert_child_before(node, name))
                    }
                    MoveNodePosition::After(name) => {
                        assert!(parent_node.insert_child_after(node, name))
                    }
                    MoveNodePosition::Last => assert!(parent_node.add_child(node)),
                }
            }
            None => match pos {
                MoveNodePosition::First => assert!(self.insert_child_first(node)),
                MoveNodePosition::Before(name) => assert!(self.insert_child_before(node, name)),
                MoveNodePosition::After(name) => assert!(self.insert_child_after(node, name)),
                MoveNodePosition::Last => self.add_child(node),
            },
        }

        for (_, animation) in self.animations.iter_mut() {
            animation.node_animations.iter_mut().for_each(|anim| {
                if anim.node_path == old_path.as_str() {
                    anim.node_path = new_path.to_string();
                }
            });
        }
    }

    pub fn remove_node(&mut self, path: impl AsRef<Utf8Path>) -> Option<NodeTemplate> {
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
            None => NodeTemplate::remove_child_impl(&mut self.root_nodes, name)?,
        };

        for (_, animation) in self.animations.iter_mut() {
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
            None => NodeTemplate::move_child_backward_impl(&mut self.root_nodes, name),
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
            None => NodeTemplate::move_child_forward_impl(&mut self.root_nodes, name),
        }
    }
}
