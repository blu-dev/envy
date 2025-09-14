use camino::Utf8PathBuf;

use crate::NodeTransform;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum TransformStep {
    Linear,
}

impl TransformStep {
    pub fn transform(&self, input: f32) -> f32 {
        match self {
            Self::Linear => input,
        }
    }
}

#[derive(Clone)]
pub struct AnimationChannel<T> {
    pub start: T,
    pub transforms: Vec<AnimationTransform<T>>,
}

impl<T> AnimationChannel<T> {
    pub fn last_value(&self) -> &T {
        self.transforms
            .last()
            .map(|value| &value.end)
            .unwrap_or(&self.start)
    }
}

#[derive(Clone)]
pub struct AnimationTransform<T> {
    pub end: T,
    pub duration: f32,
    pub first_step: TransformStep,
    pub additional_steps: Vec<TransformStep>,
}

#[derive(Clone)]
pub struct NodeAnimation {
    pub node_path: Utf8PathBuf,
    pub angle_channel: Option<AnimationChannel<f32>>,
}

impl NodeAnimation {
    pub fn animate(&self, timer: f32, node: &mut NodeTransform) -> bool {
        let Some(angle) = self.angle_channel.as_ref() else {
            return true;
        };

        let mut transform_start = angle.start;
        let mut frame_start = 0.0;
        let transform = angle.transforms.iter().find(|transform| {
            if transform.duration + frame_start > timer {
                true
            } else {
                frame_start += transform.duration;
                transform_start = transform.end;
                false
            }
        });

        if let Some(transform) = transform {
            let mut progress = transform
                .first_step
                .transform((timer - frame_start) / transform.duration);
            transform.additional_steps.iter().for_each(|transform| {
                progress = transform.transform(progress);
            });

            node.angle = transform_start + progress * (transform.end - transform_start);
            false
        } else {
            node.angle = *angle.last_value();
            true
        }
    }
}

#[derive(Clone)]
pub struct Animation {
    pub node_animations: Vec<NodeAnimation>,
}
