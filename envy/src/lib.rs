mod animations;
#[cfg(feature = "asset")]
pub mod asset;
mod backend;
mod node;
mod tree;

pub use animations::{
    Animation, AnimationChannel, AnimationTransform, NodeAnimation, TransformStep,
};
pub use backend::{EnvyBackend, PreparedGlyph, TextLayoutArgs};
pub use node::{
    EmptyNode, ImageNode, Node, NodeDisjointAccessor, NodeItem, NodeTransform, TextNode,
};
pub use tree::LayoutTree;

use bytemuck::{Pod, Zeroable};

#[cfg(feature = "unsend")]
pub trait EnvyMaybeSendSync {}

#[cfg(not(feature = "unsend"))]
pub trait EnvyMaybeSendSync: Send + Sync {}

#[cfg(feature = "unsend")]
impl<T> EnvyMaybeSendSync for T {}

#[cfg(not(feature = "unsend"))]
impl<T: Send + Sync> EnvyMaybeSendSync for T {}

#[repr(align(256), C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct DrawUniform {
    pub model_matrix: glam::Mat4,
    pub color: glam::Vec4,
    pub model_i_matrix: glam::Mat4,
    _alignment: [u8; 0x70],
}

impl DrawUniform {
    pub fn new(model: glam::Mat4, color: glam::Vec4) -> Self {
        debug_assert_eq!(std::mem::size_of::<Self>(), std::mem::align_of::<Self>());

        Self {
            model_matrix: model,
            color,
            model_i_matrix: model.inverse(),
            _alignment: [0u8; 0x70],
        }
    }
}

unsafe impl Pod for DrawUniform {}
unsafe impl Zeroable for DrawUniform {
    fn zeroed() -> Self {
        Self {
            model_matrix: glam::Mat4::zeroed(),
            color: glam::Vec4::zeroed(),
            model_i_matrix: glam::Mat4::zeroed(),
            _alignment: [0u8; 0x70]
        }
    }
}

#[repr(align(256), C)]
#[derive(Debug, Copy, Clone, PartialEq, Pod, Zeroable)]
pub struct ViewUniform {
    pub view_matrix: glam::Mat4,
    pub projection_matrix: glam::Mat4,
    _alignment: [u8; 0x80],
}

impl ViewUniform {
    pub fn new(view: glam::Mat4, proj: glam::Mat4) -> Self {
        debug_assert_eq!(std::mem::size_of::<Self>(), std::mem::align_of::<Self>());

        Self {
            view_matrix: view,
            projection_matrix: proj,
            _alignment: [0u8; 0x80],
        }
    }
}
