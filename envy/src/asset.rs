use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

use bincode::{Decode, Encode};

use crate::{EnvyBackend, LayoutTemplate, NodeImplTemplate, NodeTemplate};

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
        Self::new(0, 2, 0)
    }
}

mod v010 {
    use crate::{ImageNodeTemplate, TextNodeTemplate};

    use super::*;
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
    enum NodeImplementationV010 {
        Empty,
        Image(ImageNode),
        Text(TextNode),
    }

    #[derive(Decode, Encode)]
    struct Node {
        name: String,
        transform: NodeTransform,
        color: [u8; 4],
        implementation: NodeImplementationV010,
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
                node_path: value.node,
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

    pub(super) fn deserialize<B: EnvyBackend + EnvyAssetProvider>(
        backend: &mut B,
        reader: &mut std::io::Cursor<&[u8]>
    ) -> crate::LayoutRoot<B> {
        let mut asset: Asset =
            bincode::decode_from_std_read(reader, bincode::config::standard()).unwrap();


        let mut root_template = LayoutTemplate::default();


        fn produce_children_and_deserialize<B: EnvyBackend + EnvyAssetProvider>(
            node: Node,
            images: &mut Vec<(String, Vec<u8>)>,
            fonts: &mut Vec<(String, Vec<u8>)>,
            backend: &mut B,
        ) -> NodeTemplate {
            let Node {
                name,
                transform,
                color,
                implementation,
                children,
            } = node;

            let implementation = match implementation {
                NodeImplementationV010::Empty => NodeImplTemplate::Empty,
                NodeImplementationV010::Image(image) => {
                    if let Some(pos) = images
                        .iter()
                        .position(|(name, _)| name.eq(&image.resource_name))
                    {
                        let (name, data) = images.remove(pos);
                        backend.load_image_bytes_with_name(name, data);
                    }
                    NodeImplTemplate::Image(ImageNodeTemplate { texture_name: image.resource_name })
                }
                NodeImplementationV010::Text(text) => {
                    if let Some(pos) = fonts.iter().position(|(name, _)| name.eq(&text.font_name)) {
                        let (name, data) = fonts.remove(pos);
                        backend.load_font_bytes_with_name(name, data);
                    }
                    NodeImplTemplate::Text(TextNodeTemplate { font_name: text.font_name, text: text.text, font_size: text.font_size, line_height: text.line_height })
                }
            };

            NodeTemplate {
                name,
                transform: transform.into(),
                color,
                implementation,
                children: children.into_iter().map(|child| produce_children_and_deserialize(child, images, fonts, backend)).collect()
            }
        }

        for root in asset.root_nodes {
            root_template.add_child(produce_children_and_deserialize(
                root,
                &mut asset.images,
                &mut asset.fonts,
                backend,
            ));
        }

        root_template.animations = asset.animations.into_iter().filter(|(name, _)| !name.is_empty()).map(|(name, animation)| (name, animation.into())).collect();

        crate::LayoutRoot::from_root_template(root_template, [])
    }
}

#[derive(Decode, Encode)]
struct Asset {
    images: Vec<(String, Vec<u8>)>,
    fonts: Vec<(String, Vec<u8>)>,
    templates: Vec<(String, LayoutTemplate)>,
    root_template: LayoutTemplate,
}

pub trait EnvyAssetProvider {
    fn load_image_bytes_with_name(&mut self, name: String, bytes: Vec<u8>);
    fn load_font_bytes_with_name(&mut self, name: String, bytes: Vec<u8>);

    fn fetch_image_bytes_by_name<'a>(&'a self, name: &str) -> Cow<'a, [u8]>;
    fn fetch_font_bytes_by_name<'a>(&'a self, name: &str) -> Cow<'a, [u8]>;
}

pub fn serialize<B: EnvyBackend + EnvyAssetProvider>(
    root: &crate::LayoutRoot<B>,
    backend: &B,
) -> Vec<u8> {
    let mut serialized_images = HashSet::new();
    let mut serialized_fonts = HashSet::new();

    let mut asset = Asset {
        images: vec![],
        fonts: vec![],
        templates: vec![],
        root_template: LayoutTemplate::default()
    };

    asset.templates = root.templates().into_iter().map(|(name, template)| (name.to_string(), template.clone())).collect::<Vec<_>>();
    asset.root_template = root.root_template().clone();

    for template in [&asset.root_template].into_iter().chain(asset.templates.iter().map(|(_, template)| template)) {
        template.walk_tree(|node| {
            match &node.implementation {
                NodeImplTemplate::Image(image) if !serialized_images.contains(&image.texture_name) => {
                    serialized_images.insert(image.texture_name.clone());
                    asset.images.push((image.texture_name.clone(), backend.fetch_image_bytes_by_name(&image.texture_name).to_vec()))
                },
                NodeImplTemplate::Text(text) if !serialized_fonts.contains(&text.font_name) => {
                    serialized_fonts.insert(text.font_name.clone());
                    asset.fonts.push((text.font_name.clone(), backend.fetch_font_bytes_by_name(&text.font_name).to_vec()))
                },
                _ => {}
            }
        })
    }

    let mut output = std::io::Cursor::new(vec![]);
    let _ = bincode::encode_into_std_write(
        Version::current(),
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
) -> crate::LayoutRoot<B> {
    let mut reader = std::io::Cursor::new(bytes);
    let version: Version =
        bincode::decode_from_std_read(&mut reader, bincode::config::standard()).unwrap();

    if version == Version::new(0, 1, 0) {
        return v010::deserialize(backend, &mut reader);
    }

    assert_eq!(version, Version::current());

    let asset: Asset = bincode::decode_from_std_read(&mut reader, bincode::config::standard()).unwrap();

    let root = crate::LayoutRoot::from_root_template(asset.root_template, asset.templates);

    for (image, bytes) in asset.images {
        backend.load_image_bytes_with_name(image, bytes);
    }

    for (font, bytes) in asset.fonts {
        backend.load_font_bytes_with_name(font, bytes);
    }

    root
}
