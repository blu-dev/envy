use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

use bincode::{Decode, Encode};
use camino::Utf8PathBuf;

use crate::{EnvyBackend, NodeItem};

#[derive(Decode, Encode, Debug, Copy, Clone, PartialEq, Eq)]
struct Version {
    major: u8,
    minor: u8,
    patch: u16,
}

impl Version {
    const fn new(major: u8, minor: u8, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    const fn current() -> Self {
        Self::new(0, 1, 0)
    }
}

#[derive(Decode, Encode)]
enum Anchor {
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

impl From<crate::node::Anchor> for Anchor {
    fn from(value: crate::node::Anchor) -> Self {
        use crate::node::Anchor as A;
        match value {
            A::TopLeft => Self::TopLeft,
            A::TopCenter => Self::TopCenter,
            A::TopRight => Self::TopRight,
            A::CenterLeft => Self::CenterLeft,
            A::Center => Self::Center,
            A::CenterRight => Self::CenterRight,
            A::BottomLeft => Self::BottomLeft,
            A::BottomCenter => Self::BottomCenter,
            A::BottomRight => Self::BottomRight,
            A::Custom(custom) => Self::Custom(custom.to_array()),
        }
    }
}

impl From<Anchor> for crate::node::Anchor {
    fn from(value: Anchor) -> Self {
        use Anchor as A;
        match value {
            A::TopLeft => Self::TopLeft,
            A::TopCenter => Self::TopCenter,
            A::TopRight => Self::TopRight,
            A::CenterLeft => Self::CenterLeft,
            A::Center => Self::Center,
            A::CenterRight => Self::CenterRight,
            A::BottomLeft => Self::BottomLeft,
            A::BottomCenter => Self::BottomCenter,
            A::BottomRight => Self::BottomRight,
            A::Custom(custom) => Self::Custom(glam::Vec2::from_array(custom)),
        }
    }
}

#[derive(Decode, Encode)]
struct NodeTransform {
    angle: f32,
    position: [f32; 2],
    size: [f32; 2],
    scale: [f32; 2],
    anchor: Anchor,
}

impl From<crate::node::NodeTransform> for NodeTransform {
    fn from(value: crate::node::NodeTransform) -> Self {
        Self {
            angle: value.angle,
            position: value.position.to_array(),
            size: value.size.to_array(),
            scale: value.scale.to_array(),
            anchor: value.anchor.into(),
        }
    }
}

impl From<NodeTransform> for crate::node::NodeTransform {
    fn from(value: NodeTransform) -> Self {
        Self {
            angle: value.angle,
            position: glam::Vec2::from_array(value.position),
            size: glam::Vec2::from_array(value.size),
            scale: glam::Vec2::from_array(value.scale),
            anchor: value.anchor.into(),
        }
    }
}

#[derive(Decode, Encode)]
struct ImageNode {
    resource_name: String,
}

#[derive(Decode, Encode)]
struct TextNode {
    font_size: f32,
    line_height: f32,
    font_name: String,
    text: String,
}

#[derive(Decode, Encode)]
enum NodeImplementation {
    Empty,
    Image(ImageNode),
    Text(TextNode),
}

#[derive(Decode, Encode)]
struct Node {
    name: String,
    transform: NodeTransform,
    color: [u8; 4],
    implementation: NodeImplementation,
    children: Vec<Node>,
}

#[derive(Decode, Encode)]
enum AnimationTransformStep {
    Linear,
}

impl From<crate::animations::TransformStep> for AnimationTransformStep {
    fn from(value: crate::animations::TransformStep) -> Self {
        use crate::animations::TransformStep as T;
        match value {
            T::Linear => Self::Linear,
        }
    }
}

impl From<AnimationTransformStep> for crate::animations::TransformStep {
    fn from(value: AnimationTransformStep) -> Self {
        match value {
            AnimationTransformStep::Linear => Self::Linear,
        }
    }
}

#[derive(Decode, Encode)]
struct AnimationTransform<T> {
    end: T,
    duration: f32,
    first_step: AnimationTransformStep,
    additional_steps: Vec<AnimationTransformStep>,
}

impl<T> AnimationTransform<T> {
    fn from_map_t<U>(value: &crate::animations::AnimationTransform<U>, f: fn(&U) -> T) -> Self {
        Self {
            end: f(&value.end),
            duration: value.duration,
            first_step: value.first_step.into(),
            additional_steps: value
                .additional_steps
                .iter()
                .copied()
                .map(|step| step.into())
                .collect(),
        }
    }

    fn into_map_t<U>(self, f: fn(T) -> U) -> crate::animations::AnimationTransform<U> {
        crate::animations::AnimationTransform::<U> {
            end: f(self.end),
            duration: self.duration,
            first_step: self.first_step.into(),
            additional_steps: self
                .additional_steps
                .into_iter()
                .map(|step| step.into())
                .collect(),
        }
    }
}

#[derive(Decode, Encode)]
struct AnimationChannel<T> {
    start: T,
    transforms: Vec<AnimationTransform<T>>,
}

impl<T> AnimationChannel<T> {
    fn from_map_t<U>(value: &crate::animations::AnimationChannel<U>, f: fn(&U) -> T) -> Self {
        Self {
            start: f(&value.start),
            transforms: value
                .transforms
                .iter()
                .map(|transform| AnimationTransform::from_map_t(transform, f))
                .collect(),
        }
    }

    fn into_map_t<U>(self, f: fn(T) -> U) -> crate::animations::AnimationChannel<U> {
        crate::animations::AnimationChannel::<U> {
            start: f(self.start),
            transforms: self
                .transforms
                .into_iter()
                .map(|transform| transform.into_map_t(f))
                .collect(),
        }
    }
}

#[derive(Decode, Encode)]
struct NodeAnimation {
    node: String,
    angle: Option<AnimationChannel<f32>>,
}

impl From<&crate::animations::NodeAnimation> for NodeAnimation {
    fn from(value: &crate::animations::NodeAnimation) -> Self {
        Self {
            node: value.node_path.to_string(),
            angle: value
                .angle_channel
                .as_ref()
                .map(|channel| AnimationChannel::from_map_t(channel, |float| *float)),
        }
    }
}

impl From<NodeAnimation> for crate::animations::NodeAnimation {
    fn from(value: NodeAnimation) -> Self {
        Self {
            node_path: Utf8PathBuf::from(value.node),
            angle_channel: value.angle.map(|channel| channel.into_map_t(|float| float)),
        }
    }
}

#[derive(Decode, Encode)]
struct Animation {
    node_animations: Vec<NodeAnimation>,
}

impl From<&crate::animations::Animation> for Animation {
    fn from(value: &crate::animations::Animation) -> Self {
        Self {
            node_animations: value
                .node_animations
                .iter()
                .map(NodeAnimation::from)
                .collect(),
        }
    }
}

impl From<Animation> for crate::animations::Animation {
    fn from(value: Animation) -> Self {
        Self {
            node_animations: value
                .node_animations
                .into_iter()
                .map(crate::animations::NodeAnimation::from)
                .collect(),
        }
    }
}

#[derive(Decode, Encode)]
struct Asset {
    images: Vec<(String, Vec<u8>)>,
    fonts: Vec<(String, Vec<u8>)>,
    root_nodes: Vec<Node>,
    animations: HashMap<String, Animation>,
}

pub trait EnvyAssetProvider {
    fn load_image_bytes_with_name(&mut self, name: String, bytes: Vec<u8>);
    fn load_font_bytes_with_name(&mut self, name: String, bytes: Vec<u8>);

    fn fetch_image_bytes_by_name<'a>(&'a self, name: &str) -> Cow<'a, [u8]>;
    fn fetch_font_bytes_by_name<'a>(&'a self, name: &str) -> Cow<'a, [u8]>;
}

pub fn serialize<B: EnvyBackend + EnvyAssetProvider>(
    tree: &crate::LayoutTree<B>,
    backend: &B,
) -> Vec<u8> {
    let mut serialized_images = HashSet::new();
    let mut serialized_fonts = HashSet::new();

    let mut asset = Asset {
        images: vec![],
        fonts: vec![],
        root_nodes: vec![],
        animations: HashMap::new(),
    };

    fn visit_children_and_serialize<'a, B: EnvyAssetProvider + EnvyBackend>(
        node: &'a NodeItem<B>,
        backend: &B,
        asset: &mut Asset,
        images: &mut HashSet<&'a str>,
        fonts: &mut HashSet<&'a str>,
    ) -> Node {
        let node_impl = if let Some(image) = node.downcast::<crate::node::ImageNode<B>>() {
            let name = image.resource_name();
            if images.insert(name) {
                let bytes = backend.fetch_image_bytes_by_name(name).to_vec();
                asset.images.push((name.to_string(), bytes));
            }

            NodeImplementation::Image(ImageNode {
                resource_name: name.to_string(),
            })
        } else if let Some(text) = node.downcast::<crate::node::TextNode<B>>() {
            let name = text.font_name();
            if fonts.insert(name) {
                let bytes = backend.fetch_font_bytes_by_name(name).to_vec();
                asset.fonts.push((name.to_string(), bytes));
            }

            NodeImplementation::Text(TextNode {
                font_size: text.font_size(),
                line_height: text.line_height(),
                font_name: text.font_name().to_string(),
                text: text.text().to_string(),
            })
        } else if node.is::<crate::node::EmptyNode>() {
            NodeImplementation::Empty
        } else {
            unimplemented!()
        };

        let mut child_node = Node {
            name: node.name().to_string(),
            transform: NodeTransform::from(*node.transform()),
            color: node.color(),
            implementation: node_impl,
            children: vec![],
        };

        node.visit_children(|child| {
            child_node.children.push(visit_children_and_serialize(
                child, backend, asset, images, fonts,
            ));
        });

        child_node
    }

    tree.visit_roots(|root| {
        let node = visit_children_and_serialize(
            root,
            backend,
            &mut asset,
            &mut serialized_images,
            &mut serialized_fonts,
        );
        asset.root_nodes.push(node);
    });

    asset.animations = tree
        .animations
        .iter()
        .map(|(key, value)| (key.clone(), Animation::from(value)))
        .collect();

    let mut output = std::io::Cursor::new(vec![]);
    let _ = bincode::encode_into_std_write(
        Version {
            major: 0,
            minor: 1,
            patch: 0,
        },
        &mut output,
        bincode::config::standard(),
    )
    .unwrap();
    let _ =
        bincode::encode_into_std_write(asset, &mut output, bincode::config::standard()).unwrap();
    output.into_inner()
}

pub fn deserialize<B: EnvyBackend + EnvyAssetProvider>(
    backend: &mut B,
    bytes: &[u8],
) -> crate::LayoutTree<B> {
    let mut reader = std::io::Cursor::new(bytes);
    let version: Version =
        bincode::decode_from_std_read(&mut reader, bincode::config::standard()).unwrap();
    assert_eq!(version, Version::current());

    let mut asset: Asset =
        bincode::decode_from_std_read(&mut reader, bincode::config::standard()).unwrap();

    let mut tree = crate::LayoutTree::new();

    fn produce_children_and_deserialize<B: EnvyBackend + EnvyAssetProvider>(
        node: Node,
        images: &mut Vec<(String, Vec<u8>)>,
        fonts: &mut Vec<(String, Vec<u8>)>,
        backend: &mut B,
    ) -> NodeItem<B> {
        let Node {
            name,
            transform,
            color,
            implementation,
            children,
        } = node;

        let implementation: Box<dyn crate::node::Node<B>> = match implementation {
            NodeImplementation::Empty => Box::new(crate::node::EmptyNode),
            NodeImplementation::Image(image) => {
                if let Some(pos) = images
                    .iter()
                    .position(|(name, _)| name.eq(&image.resource_name))
                {
                    let (name, data) = images.remove(pos);
                    backend.load_image_bytes_with_name(name, data);
                }
                Box::new(crate::node::ImageNode::new(image.resource_name))
            }
            NodeImplementation::Text(text) => {
                if let Some(pos) = fonts.iter().position(|(name, _)| name.eq(&text.font_name)) {
                    let (name, data) = fonts.remove(pos);
                    backend.load_font_bytes_with_name(name, data);
                }
                Box::new(crate::node::TextNode::new(
                    text.font_name,
                    text.font_size,
                    text.line_height,
                    text.text,
                ))
            }
        };

        let mut node = NodeItem::new_boxed(name, transform.into(), color, implementation);
        for child in children {
            assert!(node.add_child(produce_children_and_deserialize(
                child, images, fonts, backend,
            )));
        }

        node
    }

    for root in asset.root_nodes {
        tree.add_child(produce_children_and_deserialize(
            root,
            &mut asset.images,
            &mut asset.fonts,
            backend,
        ));
    }

    tree.animations = asset
        .animations
        .into_iter()
        .map(|(name, animation)| (name, animation.into()))
        .collect();

    tree
}
