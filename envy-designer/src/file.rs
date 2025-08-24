use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use crate::tree::{
    EmptyNode, Node, NodeImpl, NodeSettings, RenderBackend, TextNode, TextureNode, UiTree,
};

#[derive(Deserialize, Serialize)]
enum NodeImplRepr {
    Empty,
    Texture {
        name: String,
    },
    Text {
        font: String,
        size: f32,
        height: f32,
        text: String,
    },
}

impl NodeImplRepr {
    fn from_implementation<T: RenderBackend>(implementation: &dyn NodeImpl<T>) -> Self {
        if let Some(_) = implementation.as_any().downcast_ref::<EmptyNode>() {
            Self::Empty
        } else if let Some(texture) = implementation.as_any().downcast_ref::<TextureNode<T>>() {
            Self::Texture {
                name: texture.texture_name().to_string(),
            }
        } else if let Some(text) = implementation.as_any().downcast_ref::<TextNode<T>>() {
            Self::Text {
                font: text.font_name.clone(),
                size: text.font_size,
                height: text.line_height,
                text: text.text.clone(),
            }
        } else {
            unimplemented!()
        }
    }

    fn make_implementation<T: RenderBackend>(&self) -> Box<dyn NodeImpl<T>> {
        match self {
            Self::Empty => Box::new(EmptyNode),
            Self::Texture { name } => Box::new(TextureNode::new(name.clone())),
            Self::Text {
                font,
                size,
                height,
                text,
            } => Box::new(TextNode::new(font.clone(), *size, *height, text.clone())),
        }
    }
}

#[derive(Deserialize, Serialize)]
struct NodeRepr {
    name: String,
    settings: NodeSettings,
    impl_repr: NodeImplRepr,
    children: Vec<NodeRepr>,
}

impl NodeRepr {
    fn make_real_node<T: RenderBackend>(&self) -> Node<T> {
        let mut node = Node::new_boxed(
            self.name.clone(),
            self.settings.position,
            self.settings.size,
            self.impl_repr.make_implementation(),
        )
        .with_settings(|settings| *settings = self.settings);

        for child in self.children.iter() {
            node = node.with_child(child.make_real_node());
        }

        node
    }

    fn from_real_node<T: RenderBackend>(node: &Node<T>) -> Self {
        Self {
            name: node.name.clone(),
            settings: node.settings,
            impl_repr: NodeImplRepr::from_implementation(&*node.implementation),
            children: node.children.iter().map(Self::from_real_node).collect(),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct MenuFile {
    root_children: Vec<NodeRepr>,
    pub image_resources: Vec<(String, Vec<u8>)>,
    pub font_resources: Vec<(String, Vec<u8>)>,
}

impl MenuFile {
    pub fn open(file: impl AsRef<Utf8Path>) -> Self {
        let file_data = std::fs::read(file.as_ref()).unwrap();
        bincode::deserialize(&file_data).unwrap()
    }

    pub fn create_tree<T: RenderBackend>(&self) -> UiTree<T> {
        let mut tree = UiTree::new();
        for child in self.root_children.iter() {
            tree = tree.with_child(child.make_real_node());
        }

        tree
    }

    pub fn from_tree<T: RenderBackend>(tree: &UiTree<T>) -> Self {
        Self {
            root_children: tree
                .root_children
                .iter()
                .map(NodeRepr::from_real_node)
                .collect(),
            image_resources: vec![],
            font_resources: vec![],
        }
    }

    pub fn save(&self, path: impl AsRef<Utf8Path>) {
        let data = bincode::serialize(self).unwrap();
        std::fs::write(path.as_ref(), data).unwrap();
    }
}
