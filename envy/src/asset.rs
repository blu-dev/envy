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
        Self::new(0, 3, 4)
    }
}

mod v010 {
    use crate::{ImageNodeTemplate, TextNodeTemplate, template::NodeVisibility};

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
                duration: value.duration as f32,
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
                duration: self.duration as usize,
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
                position_channel: None,
                size_channel: None,
                scale_channel: None,
                color_channel: None,
                uv_offset_channel: None,
                uv_scale_channel: None,
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
                total_duration: todo!(),
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

    pub(super) fn deserialize<B: EnvyBackend, A: EnvyAssetProvider>(
        asset_provider: &mut A,
        reader: &mut std::io::Cursor<&[u8]>,
    ) -> crate::LayoutRoot<B> {
        let mut asset: Asset =
            bincode::decode_from_std_read(reader, bincode::config::standard()).unwrap();

        let mut root_template = LayoutTemplate::default();

        fn produce_children_and_deserialize<A: EnvyAssetProvider>(
            node: Node,
            images: &mut Vec<(String, Vec<u8>)>,
            fonts: &mut Vec<(String, Vec<u8>)>,
            asset_provider: &mut A,
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
                        asset_provider.load_image_bytes_with_name(name, data);
                    }
                    NodeImplTemplate::Image(ImageNodeTemplate {
                        texture_name: image.resource_name,
                        mask_texture_name: None,
                        image_scaling_mode_x: Default::default(),
                        image_scaling_mode_y: Default::default(),
                        uv_offset: glam::Vec2::ZERO,
                        uv_scale: glam::Vec2::ONE,
                    })
                }
                NodeImplementationV010::Text(text) => {
                    if let Some(pos) = fonts.iter().position(|(name, _)| name.eq(&text.font_name)) {
                        let (name, data) = fonts.remove(pos);
                        asset_provider.load_font_bytes_with_name(name, data);
                    }
                    NodeImplTemplate::Text(TextNodeTemplate {
                        font_name: text.font_name,
                        text: text.text,
                        font_size: text.font_size,
                        line_height: text.line_height,
                        outline_thickness: 0.0,
                        outline_color: [255; 4]
                    })
                }
            };

            NodeTemplate {
                name,
                transform: transform.into(),
                color,
                implementation,
                visibility: NodeVisibility::Inherited,
                children: children
                    .into_iter()
                    .map(|child| produce_children_and_deserialize(child, images, fonts, asset_provider))
                    .collect(),
            }
        }

        for root in asset.root_nodes {
            root_template.add_child(produce_children_and_deserialize(
                root,
                &mut asset.images,
                &mut asset.fonts,
                asset_provider,
            ));
        }

        root_template.animations = asset
            .animations
            .into_iter()
            .filter(|(name, _)| !name.is_empty())
            .map(|(name, animation)| (name, animation.into()))
            .collect();

        crate::LayoutRoot::from_root_template(root_template, [])
    }
}

mod v020 {
    use super::*;
    use std::io::Cursor;

    use crate::{animations::Animation, template::NodeTemplate};
    #[derive(bincode::Encode, bincode::Decode)]
    struct LayoutTemplate {
        root_nodes: Vec<NodeTemplate>,
        animations: Vec<(String, Animation)>,
    }

    impl From<LayoutTemplate> for crate::LayoutTemplate {
        fn from(value: LayoutTemplate) -> Self {
            Self {
                canvas_size: [1920, 1080],
                root_nodes: value.root_nodes,
                animations: value.animations,
            }
        }
    }

    #[derive(Decode, Encode)]
    struct Asset {
        images: Vec<(String, Vec<u8>)>,
        fonts: Vec<(String, Vec<u8>)>,
        templates: Vec<(String, LayoutTemplate)>,
        root_template: LayoutTemplate,
    }

    pub(super) fn deserialize<B: EnvyBackend, A: EnvyAssetProvider>(
        asset_provider: &mut A,
        reader: &mut Cursor<&[u8]>,
    ) -> crate::LayoutRoot<B> {
        let asset: Asset =
            bincode::decode_from_std_read(reader, bincode::config::standard()).unwrap();

        let root_template = crate::LayoutTemplate::from(asset.root_template);

        let templates = asset
            .templates
            .into_iter()
            .map(|(name, template)| (name, crate::LayoutTemplate::from(template)));

        let root = crate::LayoutRoot::from_root_template(root_template, templates);

        for (image, bytes) in asset.images {
            asset_provider.load_image_bytes_with_name(image, bytes);
        }

        for (font, bytes) in asset.fonts {
            asset_provider.load_font_bytes_with_name(font, bytes);
        }

        root
    }
}

mod v021 {
    use super::*;
    use std::io::Cursor;

    #[derive(bincode::Encode, bincode::Decode)]
    struct NodeAnimation {
        node_path: String,
        angle_channel: Option<AnimationChannel<f32>>,
        position_channel: Option<AnimationChannel<glam::Vec2>>,
        size_channel: Option<AnimationChannel<glam::Vec2>>,
        scale_channel: Option<AnimationChannel<glam::Vec2>>,
    }

    impl From<NodeAnimation> for crate::animations::NodeAnimation {
        fn from(value: NodeAnimation) -> Self {
            Self {
                node_path: value.node_path,
                angle_channel: value.angle_channel,
                position_channel: value.position_channel,
                size_channel: value.size_channel,
                scale_channel: value.scale_channel,
                color_channel: None,
                uv_offset_channel: None,
                uv_scale_channel: None,
            }
        }
    }

    #[derive(bincode::Encode, bincode::Decode)]
    struct Animation {
        node_animations: Vec<NodeAnimation>,
        total_duration: usize,
    }

    impl From<Animation> for crate::animations::Animation {
        fn from(value: Animation) -> Self {
            Self {
                node_animations: value.node_animations.into_iter().map(Into::into).collect(),
                total_duration: value.total_duration,
            }
        }
    }

    use crate::{template::NodeTemplate, AnimationChannel};
    #[derive(bincode::Encode, bincode::Decode)]
    struct LayoutTemplate {
        canvas_size: [u32; 2],
        root_nodes: Vec<NodeTemplate>,
        animations: Vec<(String, Animation)>,
    }

    impl From<LayoutTemplate> for crate::LayoutTemplate {
        fn from(value: LayoutTemplate) -> Self {
            Self {
                canvas_size: value.canvas_size,
                root_nodes: value.root_nodes,
                animations: value
                    .animations
                    .into_iter()
                    .map(|(name, anim)| (name, anim.into()))
                    .collect(),
            }
        }
    }

    #[derive(Decode, Encode)]
    struct Asset {
        images: Vec<(String, Vec<u8>)>,
        fonts: Vec<(String, Vec<u8>)>,
        templates: Vec<(String, LayoutTemplate)>,
        root_template: LayoutTemplate,
    }

    pub(super) fn deserialize<B: EnvyBackend, A: EnvyAssetProvider>(
        backend: &mut A,
        reader: &mut Cursor<&[u8]>,
    ) -> crate::LayoutRoot<B> {
        let asset: Asset =
            bincode::decode_from_std_read(reader, bincode::config::standard()).unwrap();

        let root_template = crate::LayoutTemplate::from(asset.root_template);

        let templates = asset
            .templates
            .into_iter()
            .map(|(name, template)| (name, crate::LayoutTemplate::from(template)));

        let root = crate::LayoutRoot::from_root_template(root_template, templates);

        for (image, bytes) in asset.images {
            backend.load_image_bytes_with_name(image, bytes);
        }

        for (font, bytes) in asset.fonts {
            backend.load_font_bytes_with_name(font, bytes);
        }

        root
    }
}

mod v030 {
    use std::io::Cursor;

    use crate::{AnimationChannel, NodeTransform, SublayoutNodeTemplate, TextNodeTemplate, template::NodeVisibility};

    #[derive(bincode::Encode, bincode::Decode, Clone)]
    struct ImageNodeTemplate {
        pub texture_name: String,
    }

    impl From<ImageNodeTemplate> for crate::ImageNodeTemplate {
        fn from(value: ImageNodeTemplate) -> Self {
            Self {
                texture_name: value.texture_name,
                mask_texture_name: None,
                image_scaling_mode_x: Default::default(),
                image_scaling_mode_y: Default::default(),
                uv_offset: glam::Vec2::ZERO,
                uv_scale: glam::Vec2::ONE
            }
        }
    }

    #[derive(bincode::Encode, bincode::Decode, Clone)]
    enum NodeImplTemplate {
        Empty,
        Image(ImageNodeTemplate),
        Text(TextNodeTemplate),
        Sublayout(SublayoutNodeTemplate),
    }

    impl From<NodeImplTemplate> for crate::NodeImplTemplate {
        fn from(value: NodeImplTemplate) -> Self {
            use NodeImplTemplate as N;
            match value {
                N::Empty => Self::Empty,
                N::Image(image) => Self::Image(image.into()),
                N::Text(text) => Self::Text(text),
                N::Sublayout(sublayout) => Self::Sublayout(sublayout),
            }
        }
    }

    #[derive(Clone, bincode::Encode, bincode::Decode)]
    struct NodeTemplate {
        name: String,
        transform: NodeTransform,
        color: [u8; 4],
        children: Vec<NodeTemplate>,
        implementation: NodeImplTemplate,
    }

    impl From<NodeTemplate> for crate::NodeTemplate {
        fn from(value: NodeTemplate) -> Self {
            Self {
                name: value.name,
                transform: value.transform,
                color: value.color,
                visibility: NodeVisibility::Inherited,
                children: value.children.into_iter().map(Into::into).collect(),
                implementation: value.implementation.into(),
            }
        }
    }

    #[derive(bincode::Encode, bincode::Decode)]
    struct NodeAnimation {
        node_path: String,
        angle_channel: Option<AnimationChannel<f32>>,
        position_channel: Option<AnimationChannel<glam::Vec2>>,
        size_channel: Option<AnimationChannel<glam::Vec2>>,
        scale_channel: Option<AnimationChannel<glam::Vec2>>,
        color_channel: Option<AnimationChannel<[u8; 4]>>,
    }

    impl From<NodeAnimation> for crate::animations::NodeAnimation {
        fn from(value: NodeAnimation) -> Self {
            Self {
                node_path: value.node_path,
                angle_channel: value.angle_channel,
                position_channel: value.position_channel,
                size_channel: value.size_channel,
                scale_channel: value.scale_channel,
                color_channel: value.color_channel,
                uv_offset_channel: None,
                uv_scale_channel: None,
            }
        }
    }

    #[derive(bincode::Encode, bincode::Decode)]
    struct Animation {
        node_animations: Vec<NodeAnimation>,
        total_duration: usize,
    }

    impl From<Animation> for crate::animations::Animation {
        fn from(value: Animation) -> Self {
            Self {
                node_animations: value.node_animations.into_iter().map(Into::into).collect(),
                total_duration: value.total_duration,
            }
        }
    }

    #[derive(bincode::Encode, bincode::Decode)]
    struct LayoutTemplate {
        canvas_size: [u32; 2],
        root_nodes: Vec<NodeTemplate>,
        animations: Vec<(String, Animation)>,
    }

    impl From<LayoutTemplate> for crate::LayoutTemplate {
        fn from(value: LayoutTemplate) -> Self {
            Self {
                canvas_size: value.canvas_size,
                root_nodes: value.root_nodes.into_iter().map(Into::into).collect(),
                animations: value
                    .animations
                    .into_iter()
                    .map(|(name, anim)| (name, anim.into()))
                    .collect(),
            }
        }
    }

    #[derive(bincode::Decode, bincode::Encode)]
    struct Asset {
        images: Vec<(String, Vec<u8>)>,
        fonts: Vec<(String, Vec<u8>)>,
        templates: Vec<(String, LayoutTemplate)>,
        root_template: LayoutTemplate,
    }

    pub(super) fn deserialize<B: crate::EnvyBackend, A: super::EnvyAssetProvider>(
        backend: &mut A,
        reader: &mut Cursor<&[u8]>,
    ) -> crate::LayoutRoot<B> {
        let asset: Asset =
            bincode::decode_from_std_read(reader, bincode::config::standard()).unwrap();

        let root_template = crate::LayoutTemplate::from(asset.root_template);

        let templates = asset
            .templates
            .into_iter()
            .map(|(name, template)| (name, crate::LayoutTemplate::from(template)));

        let root = crate::LayoutRoot::from_root_template(root_template, templates);

        for (image, bytes) in asset.images {
            backend.load_image_bytes_with_name(image, bytes);
        }

        for (font, bytes) in asset.fonts {
            backend.load_font_bytes_with_name(font, bytes);
        }

        root
    }
}

mod v031 {
    use std::io::Cursor;

    use crate::{Animation, NodeTransform, SublayoutNodeTemplate, TextNodeTemplate, template::NodeVisibility};

    #[derive(bincode::Encode, bincode::Decode, Clone)]
    struct ImageNodeTemplate {
        pub texture_name: String,
        pub mask_texture_name: Option<String>,
    }

    impl From<ImageNodeTemplate> for crate::ImageNodeTemplate {
        fn from(value: ImageNodeTemplate) -> Self {
            Self {
                texture_name: value.texture_name,
                mask_texture_name: value.mask_texture_name,
                image_scaling_mode_x: Default::default(),
                image_scaling_mode_y: Default::default(),
                uv_offset: glam::Vec2::ZERO,
                uv_scale: glam::Vec2::ONE,
            }
        }
    }

    #[derive(bincode::Encode, bincode::Decode, Clone)]
    enum NodeImplTemplate {
        Empty,
        Image(ImageNodeTemplate),
        Text(TextNodeTemplate),
        Sublayout(SublayoutNodeTemplate),
    }

    impl From<NodeImplTemplate> for crate::NodeImplTemplate {
        fn from(value: NodeImplTemplate) -> Self {
            use NodeImplTemplate as N;
            match value {
                N::Empty => Self::Empty,
                N::Image(image) => Self::Image(image.into()),
                N::Text(text) => Self::Text(text),
                N::Sublayout(sublayout) => Self::Sublayout(sublayout),
            }
        }
    }

    #[derive(Clone, bincode::Encode, bincode::Decode)]
    struct NodeTemplate {
        name: String,
        transform: NodeTransform,
        color: [u8; 4],
        children: Vec<NodeTemplate>,
        implementation: NodeImplTemplate,
    }

    impl From<NodeTemplate> for crate::NodeTemplate {
        fn from(value: NodeTemplate) -> Self {
            Self {
                name: value.name,
                transform: value.transform,
                color: value.color,
                visibility: NodeVisibility::Inherited,
                children: value.children.into_iter().map(Into::into).collect(),
                implementation: value.implementation.into(),
            }
        }
    }

    #[derive(bincode::Encode, bincode::Decode)]
    struct LayoutTemplate {
        canvas_size: [u32; 2],
        root_nodes: Vec<NodeTemplate>,
        animations: Vec<(String, Animation)>,
    }

    impl From<LayoutTemplate> for crate::LayoutTemplate {
        fn from(value: LayoutTemplate) -> Self {
            Self {
                canvas_size: value.canvas_size,
                root_nodes: value.root_nodes.into_iter().map(Into::into).collect(),
                animations: value
                    .animations
                    .into_iter()
                    .map(|(name, anim)| (name, anim.into()))
                    .collect(),
            }
        }
    }

    #[derive(bincode::Decode, bincode::Encode)]
    struct Asset {
        images: Vec<(String, Vec<u8>)>,
        fonts: Vec<(String, Vec<u8>)>,
        templates: Vec<(String, LayoutTemplate)>,
        root_template: LayoutTemplate,
    }

    pub(super) fn deserialize<B: crate::EnvyBackend, A: super::EnvyAssetProvider>(
        backend: &mut A,
        reader: &mut Cursor<&[u8]>,
    ) -> crate::LayoutRoot<B> {
        let asset: Asset =
            bincode::decode_from_std_read(reader, bincode::config::standard()).unwrap();

        let root_template = crate::LayoutTemplate::from(asset.root_template);

        let templates = asset
            .templates
            .into_iter()
            .map(|(name, template)| (name, crate::LayoutTemplate::from(template)));

        let root = crate::LayoutRoot::from_root_template(root_template, templates);

        for (image, bytes) in asset.images {
            backend.load_image_bytes_with_name(image, bytes);
        }

        for (font, bytes) in asset.fonts {
            backend.load_font_bytes_with_name(font, bytes);
        }

        root
    }
}

mod v032 {
    use std::io::Cursor;

    use crate::{AnimationChannel, ImageScalingMode, NodeTransform, SublayoutNodeTemplate, TextNodeTemplate, template::NodeVisibility};

    #[derive(bincode::Encode, bincode::Decode, Clone)]
    struct ImageNodeTemplate {
        pub texture_name: String,
        pub mask_texture_name: Option<String>,
        pub image_scaling_mode_x: ImageScalingMode,
        pub image_scaling_mode_y: ImageScalingMode,
    }

    impl From<ImageNodeTemplate> for crate::ImageNodeTemplate {
        fn from(value: ImageNodeTemplate) -> Self {
            Self {
                texture_name: value.texture_name,
                mask_texture_name: value.mask_texture_name,
                image_scaling_mode_x: value.image_scaling_mode_x,
                image_scaling_mode_y: value.image_scaling_mode_y,
                uv_offset: glam::Vec2::ZERO,
                uv_scale: glam::Vec2::ONE,
            }
        }
    }

    #[derive(bincode::Encode, bincode::Decode, Clone)]
    enum NodeImplTemplate {
        Empty,
        Image(ImageNodeTemplate),
        Text(TextNodeTemplate),
        Sublayout(SublayoutNodeTemplate),
    }

    impl From<NodeImplTemplate> for crate::NodeImplTemplate {
        fn from(value: NodeImplTemplate) -> Self {
            use NodeImplTemplate as N;
            match value {
                N::Empty => Self::Empty,
                N::Image(image) => Self::Image(image.into()),
                N::Text(text) => Self::Text(text),
                N::Sublayout(sublayout) => Self::Sublayout(sublayout),
            }
        }
    }

    #[derive(Clone, bincode::Encode, bincode::Decode)]
    struct NodeTemplate {
        name: String,
        transform: NodeTransform,
        color: [u8; 4],
        children: Vec<NodeTemplate>,
        implementation: NodeImplTemplate,
    }

    impl From<NodeTemplate> for crate::NodeTemplate {
        fn from(value: NodeTemplate) -> Self {
            Self {
                name: value.name,
                transform: value.transform,
                color: value.color,
                visibility: NodeVisibility::Inherited,
                children: value.children.into_iter().map(Into::into).collect(),
                implementation: value.implementation.into(),
            }
        }
    }

    #[derive(bincode::Encode, bincode::Decode)]
    struct NodeAnimation {
        node_path: String,
        angle_channel: Option<AnimationChannel<f32>>,
        position_channel: Option<AnimationChannel<glam::Vec2>>,
        size_channel: Option<AnimationChannel<glam::Vec2>>,
        scale_channel: Option<AnimationChannel<glam::Vec2>>,
        color_channel: Option<AnimationChannel<[u8; 4]>>,
    }

    impl From<NodeAnimation> for crate::animations::NodeAnimation {
        fn from(value: NodeAnimation) -> Self {
            Self {
                node_path: value.node_path,
                angle_channel: value.angle_channel,
                position_channel: value.position_channel,
                size_channel: value.size_channel,
                scale_channel: value.scale_channel,
                color_channel: value.color_channel,
                uv_offset_channel: None,
                uv_scale_channel: None,
            }
        }
    }

    #[derive(bincode::Encode, bincode::Decode)]
    struct Animation {
        node_animations: Vec<NodeAnimation>,
        total_duration: usize,
    }

    impl From<Animation> for crate::animations::Animation {
        fn from(value: Animation) -> Self {
            Self {
                node_animations: value.node_animations.into_iter().map(Into::into).collect(),
                total_duration: value.total_duration,
            }
        }
    }

    #[derive(bincode::Encode, bincode::Decode)]
    struct LayoutTemplate {
        canvas_size: [u32; 2],
        root_nodes: Vec<NodeTemplate>,
        animations: Vec<(String, Animation)>,
    }

    impl From<LayoutTemplate> for crate::LayoutTemplate {
        fn from(value: LayoutTemplate) -> Self {
            Self {
                canvas_size: value.canvas_size,
                root_nodes: value.root_nodes.into_iter().map(Into::into).collect(),
                animations: value
                    .animations
                    .into_iter()
                    .map(|(name, anim)| (name, anim.into()))
                    .collect(),
            }
        }
    }

    #[derive(bincode::Decode, bincode::Encode)]
    struct Asset {
        images: Vec<(String, Vec<u8>)>,
        fonts: Vec<(String, Vec<u8>)>,
        templates: Vec<(String, LayoutTemplate)>,
        root_template: LayoutTemplate,
    }

    pub(super) fn deserialize<B: crate::EnvyBackend, A: super::EnvyAssetProvider>(
        backend: &mut A,
        reader: &mut Cursor<&[u8]>,
    ) -> crate::LayoutRoot<B> {
        let asset: Asset =
            bincode::decode_from_std_read(reader, bincode::config::standard()).unwrap();

        let root_template = crate::LayoutTemplate::from(asset.root_template);

        let templates = asset
            .templates
            .into_iter()
            .map(|(name, template)| (name, crate::LayoutTemplate::from(template)));

        let root = crate::LayoutRoot::from_root_template(root_template, templates);

        for (image, bytes) in asset.images {
            backend.load_image_bytes_with_name(image, bytes);
        }

        for (font, bytes) in asset.fonts {
            backend.load_font_bytes_with_name(font, bytes);
        }

        root
    }
}

mod v033 {
    use std::io::Cursor;

    use crate::{Animation, ImageNodeTemplate, NodeTransform, SublayoutNodeTemplate, template::NodeVisibility};

    #[derive(bincode::Encode, bincode::Decode, Clone)]
    struct TextNodeTemplate {
        pub font_name: String,
        pub text: String,
        pub font_size: f32,
        pub line_height: f32,
    }

    impl From<TextNodeTemplate> for crate::TextNodeTemplate {
        fn from(value: TextNodeTemplate) -> Self {
            Self {
                font_name: value.font_name,
                text: value.text,
                font_size: value.font_size,
                line_height: value.line_height,
                outline_color: [255; 4],
                outline_thickness: 0.0
            }
        }
    }

    #[derive(bincode::Encode, bincode::Decode, Clone)]
    enum NodeImplTemplate {
        Empty,
        Image(ImageNodeTemplate),
        Text(TextNodeTemplate),
        Sublayout(SublayoutNodeTemplate),
    }

    impl From<NodeImplTemplate> for crate::NodeImplTemplate {
        fn from(value: NodeImplTemplate) -> Self {
            use NodeImplTemplate as N;
            match value {
                N::Empty => Self::Empty,
                N::Image(image) => Self::Image(image),
                N::Text(text) => Self::Text(text.into()),
                N::Sublayout(sublayout) => Self::Sublayout(sublayout),
            }
        }
    }

    #[derive(Clone, bincode::Encode, bincode::Decode)]
    struct NodeTemplate {
        name: String,
        transform: NodeTransform,
        color: [u8; 4],
        visibility: NodeVisibility,
        children: Vec<NodeTemplate>,
        implementation: NodeImplTemplate,
    }

    impl From<NodeTemplate> for crate::NodeTemplate {
        fn from(value: NodeTemplate) -> Self {
            Self {
                name: value.name,
                transform: value.transform,
                color: value.color,
                visibility: value.visibility,
                children: value.children.into_iter().map(Into::into).collect(),
                implementation: value.implementation.into(),
            }
        }
    }

    #[derive(bincode::Encode, bincode::Decode)]
    struct LayoutTemplate {
        canvas_size: [u32; 2],
        root_nodes: Vec<NodeTemplate>,
        animations: Vec<(String, Animation)>,
    }

    impl From<LayoutTemplate> for crate::LayoutTemplate {
        fn from(value: LayoutTemplate) -> Self {
            Self {
                canvas_size: value.canvas_size,
                root_nodes: value.root_nodes.into_iter().map(Into::into).collect(),
                animations: value
                    .animations
                    .into_iter()
                    .map(|(name, anim)| (name, anim.into()))
                    .collect(),
            }
        }
    }

    #[derive(bincode::Decode, bincode::Encode)]
    struct Asset {
        images: Vec<(String, Vec<u8>)>,
        fonts: Vec<(String, Vec<u8>)>,
        templates: Vec<(String, LayoutTemplate)>,
        root_template: LayoutTemplate,
    }

    pub(super) fn deserialize<B: crate::EnvyBackend, A: super::EnvyAssetProvider>(
        backend: &mut A,
        reader: &mut Cursor<&[u8]>,
    ) -> crate::LayoutRoot<B> {
        let asset: Asset =
            bincode::decode_from_std_read(reader, bincode::config::standard()).unwrap();

        let root_template = crate::LayoutTemplate::from(asset.root_template);

        let templates = asset
            .templates
            .into_iter()
            .map(|(name, template)| (name, crate::LayoutTemplate::from(template)));

        let root = crate::LayoutRoot::from_root_template(root_template, templates);

        for (image, bytes) in asset.images {
            backend.load_image_bytes_with_name(image, bytes);
        }

        for (font, bytes) in asset.fonts {
            backend.load_font_bytes_with_name(font, bytes);
        }

        root
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
        root_template: LayoutTemplate::default(),
    };

    asset.templates = root
        .iter_templates()
        .into_iter()
        .map(|(name, template)| (name.to_string(), template.clone()))
        .collect::<Vec<_>>();
    asset.root_template = root.root_template().clone();

    for template in [&asset.root_template]
        .into_iter()
        .chain(asset.templates.iter().map(|(_, template)| template))
    {
        template.walk_tree(|node| match &node.implementation {
            NodeImplTemplate::Image(image) => {
                if !serialized_images.contains(&image.texture_name) {
                    serialized_images.insert(image.texture_name.clone());
                    asset.images.push((
                        image.texture_name.clone(),
                        backend
                            .fetch_image_bytes_by_name(&image.texture_name)
                            .to_vec(),
                    ))
                }

                if let Some(mask_name) = image.mask_texture_name.as_ref() {
                    if !serialized_images.contains(mask_name) {
                        serialized_images.insert(mask_name.clone());
                        asset.images.push((
                            mask_name.clone(),
                            backend.fetch_image_bytes_by_name(&mask_name).to_vec(),
                        ))
                    }
                }
            }
            NodeImplTemplate::Text(text) if !serialized_fonts.contains(&text.font_name) => {
                serialized_fonts.insert(text.font_name.clone());
                asset.fonts.push((
                    text.font_name.clone(),
                    backend.fetch_font_bytes_by_name(&text.font_name).to_vec(),
                ))
            }
            _ => {}
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

pub fn deserialize<B: EnvyBackend, A: EnvyAssetProvider>(
    asset_provider: &mut A,
    bytes: &[u8],
) -> crate::LayoutRoot<B> {
    let mut reader = std::io::Cursor::new(bytes);
    let version: Version =
        bincode::decode_from_std_read(&mut reader, bincode::config::standard()).unwrap();

    if version == Version::new(0, 1, 0) {
        return v010::deserialize(asset_provider, &mut reader);
    } else if version == Version::new(0, 2, 0) {
        return v020::deserialize(asset_provider, &mut reader);
    } else if version == Version::new(0, 2, 1) {
        return v021::deserialize(asset_provider, &mut reader);
    } else if version == Version::new(0, 3, 0) {
        return v030::deserialize(asset_provider, &mut reader);
    } else if version == Version::new(0, 3, 1) {
        return v031::deserialize(asset_provider, &mut reader);
    } else if version == Version::new(0, 3, 2) {
        return v032::deserialize(asset_provider, &mut reader);
    } else if version == Version::new(0, 3, 3) {
        return v033::deserialize(asset_provider, &mut reader);
    }

    assert_eq!(version, Version::current());

    let asset: Asset =
        bincode::decode_from_std_read(&mut reader, bincode::config::standard()).unwrap();

    let root = crate::LayoutRoot::from_root_template(asset.root_template, asset.templates);

    for (image, bytes) in asset.images {
        asset_provider.load_image_bytes_with_name(image, bytes);
    }

    for (font, bytes) in asset.fonts {
        asset_provider.load_font_bytes_with_name(font, bytes);
    }

    root
}
