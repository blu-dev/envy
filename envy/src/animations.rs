use std::cmp::Ordering;

use crate::{EnvyBackend, ImageNode, Node, NodeTransform};

#[cfg_attr(feature = "asset", derive(bincode::Encode, bincode::Decode))]
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

pub trait Interpolatable: Copy {
    fn interpolate(start: Self, end: Self, progress: f32) -> Self;
}

impl Interpolatable for f32 {
    fn interpolate(start: Self, end: Self, progress: f32) -> Self {
        start + (end - start) * progress
    }
}

impl Interpolatable for glam::Vec2 {
    fn interpolate(start: Self, end: Self, progress: f32) -> Self {
        start + (end - start) * progress
    }
}

impl Interpolatable for [u8; 4] {
    fn interpolate(start: Self, end: Self, progress: f32) -> Self {
        fn gamma(f: f32) -> f32 {
            if f <= 0.0 {
                return f;
            }

            if f <= 0.04045 {
                f / 12.92
            } else {
                f32::powf((f + 0.055) / 1.055, 2.4)
            }
        }

        fn gamma_i(f: f32) -> f32 {
            if f <= 0.0 {
                return f;
            }

            if f <= 0.0031308 {
                f * 12.92
            } else {
                (1.055 * f32::powf(f, 1.0 / 2.4)) - 0.055
            }
        }

        let [r, g, b, a] = start.map(|c| c as f32 / 255.0);
        let sr = gamma(r);
        let sg = gamma(g);
        let sb = gamma(b);
        let sa = a;
        let s_bright = (sr + sg + sb + sa).powf(0.43);

        let [r, g, b, a] = end.map(|c| c as f32 / 255.0);
        let er = gamma(r);
        let eg = gamma(g);
        let eb = gamma(b);
        let ea = a;
        let e_bright = (er + eg + eb + ea).powf(0.43);

        let intensity = (s_bright * (1.0 - progress) + e_bright * progress).powf(0.43f32.recip());
        let mut r = sr * (1.0 - progress) + er * progress;
        let mut g = sg * (1.0 - progress) + eg * progress;
        let mut b = sb * (1.0 - progress) + eb * progress;
        let mut a = sa * (1.0 - progress) + ea * progress;
        let sum = r + g + b + a;
        if sum != 0.0 {
            r *= intensity / sum;
            g *= intensity / sum;
            b *= intensity / sum;
            a *= intensity / sum;
        }

        [gamma_i(r), gamma_i(g), gamma_i(b), a].map(|c| (c * 255.0) as u8)
    }
}

impl<T> AnimationChannel<T> {
    pub fn remove_keyframe(&mut self, keyframe: usize) {
        if keyframe == 0 {
            // Don't
            return;
        }

        let mut total = 0usize;
        for idx in 0..self.transforms.len() {
            let new_total = total + self.transforms[idx].duration;

            match new_total.cmp(&keyframe) {
                Ordering::Less => {}
                Ordering::Equal => {
                    let old_keyframe = self.transforms.remove(idx);
                    if let Some(new_keyframe) = self.transforms.get_mut(idx) {
                        new_keyframe.duration += old_keyframe.duration;
                    }
                    return;
                }
                Ordering::Greater => return,
            }

            total = new_total;
        }
    }

    pub fn insert_keyframe(&mut self, keyframe: usize)
    where
        T: Interpolatable,
    {
        if keyframe == 0 {
            // idk chief don't fuckin do this
            return;
        }

        let mut total = 0usize;
        for idx in 0..self.transforms.len() {
            let new_total = total + self.transforms[idx].duration;

            match new_total.cmp(&keyframe) {
                Ordering::Less => {}
                Ordering::Equal => return, // again don't do this
                Ordering::Greater => {
                    let prev = if idx == 0 {
                        self.start
                    } else {
                        self.transforms[idx - 1].end
                    };

                    let progress = (keyframe - total) as f32 / (new_total - total) as f32;
                    let new_value = T::interpolate(prev, self.transforms[idx].end, progress);

                    self.transforms.insert(
                        idx,
                        AnimationTransform {
                            end: new_value,
                            duration: keyframe - total,
                            first_step: TransformStep::Linear,
                            additional_steps: vec![],
                        },
                    );
                    self.transforms[idx + 1].duration = new_total - keyframe;
                    return;
                }
            }

            total = new_total;
        }

        let duration = keyframe - total;
        let value = *self.last_value();
        self.transforms.push(AnimationTransform {
            end: value,
            duration,
            first_step: TransformStep::Linear,
            additional_steps: vec![],
        });
    }

    pub fn get_prev_keyframe_idx(&self, keyframe: usize) -> usize {
        let mut total = 0usize;
        for idx in 0..self.transforms.len() {
            let new_total = total + self.transforms[idx].duration;

            match new_total.cmp(&keyframe) {
                Ordering::Less => {}
                Ordering::Equal | Ordering::Greater => return total,
            }
            total = new_total;
        }

        total
    }

    pub fn get_next_keyframe_idx(&self, keyframe: usize) -> usize {
        let mut total = 0usize;
        for idx in 0..self.transforms.len() {
            let new_total = total + self.transforms[idx].duration;

            match total.cmp(&keyframe) {
                Ordering::Less => {}
                Ordering::Equal | Ordering::Greater => return new_total,
            }
            total = new_total;
        }

        total
    }

    pub fn keyframe_mut(&mut self, keyframe: usize) -> Option<&mut T> {
        if keyframe == 0 {
            Some(&mut self.start)
        } else {
            let mut total = 0usize;
            for transform in self.transforms.iter_mut() {
                total += transform.duration;

                match total.cmp(&keyframe) {
                    Ordering::Less => continue,
                    Ordering::Equal => return Some(&mut transform.end),
                    Ordering::Greater => return None,
                }
            }

            None
        }
    }

    pub fn value_for_frame(&mut self, keyframe: usize) -> T
    where
        T: Interpolatable,
    {
        if keyframe == 0 {
            self.start
        } else {
            let mut total = 0usize;

            for idx in 0..self.transforms.len() {
                let new_total = total + self.transforms[idx].duration;

                match new_total.cmp(&keyframe) {
                    Ordering::Less => {}
                    Ordering::Equal => return self.transforms[idx].end,
                    Ordering::Greater => {
                        let prev = if idx == 0 {
                            self.start
                        } else {
                            self.transforms[idx - 1].end
                        };

                        let progress = (keyframe - total) as f32 / (new_total - total) as f32;
                        return T::interpolate(prev, self.transforms[idx].end, progress);
                    }
                }

                total = new_total;
            }

            *self.last_value()
        }
    }

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
    pub duration: usize,
    pub first_step: TransformStep,
    pub additional_steps: Vec<TransformStep>,
}

#[cfg(feature = "asset")]
const _: () = {
    use glam::Vec2;

    impl bincode::Encode for AnimationChannel<f32> {
        fn encode<E: bincode::enc::Encoder>(
            &self,
            encoder: &mut E,
        ) -> Result<(), bincode::error::EncodeError> {
            self.start.encode(encoder)?;
            self.transforms.encode(encoder)
        }
    }

    impl<C> bincode::Decode<C> for AnimationChannel<f32> {
        fn decode<D: bincode::de::Decoder<Context = C>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            Ok(Self {
                start: f32::decode(decoder)?,
                transforms: <Vec<AnimationTransform<f32>>>::decode(decoder)?,
            })
        }
    }

    impl<'de, C> bincode::BorrowDecode<'de, C> for AnimationChannel<f32> {
        fn borrow_decode<D: bincode::de::BorrowDecoder<'de, Context = C>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            bincode::Decode::decode(decoder)
        }
    }

    impl bincode::Encode for AnimationTransform<f32> {
        fn encode<E: bincode::enc::Encoder>(
            &self,
            encoder: &mut E,
        ) -> Result<(), bincode::error::EncodeError> {
            self.end.encode(encoder)?;
            self.duration.encode(encoder)?;
            self.first_step.encode(encoder)?;
            self.additional_steps.encode(encoder)
        }
    }

    impl<C> bincode::Decode<C> for AnimationTransform<f32> {
        fn decode<D: bincode::de::Decoder<Context = C>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            Ok(Self {
                end: f32::decode(decoder)?,
                duration: usize::decode(decoder)?,
                first_step: TransformStep::decode(decoder)?,
                additional_steps: <Vec<TransformStep>>::decode(decoder)?,
            })
        }
    }

    impl<'de, C> bincode::BorrowDecode<'de, C> for AnimationTransform<f32> {
        fn borrow_decode<D: bincode::de::BorrowDecoder<'de, Context = C>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            bincode::Decode::decode(decoder)
        }
    }

    impl bincode::Encode for AnimationChannel<[u8; 4]> {
        fn encode<E: bincode::enc::Encoder>(
            &self,
            encoder: &mut E,
        ) -> Result<(), bincode::error::EncodeError> {
            self.start.encode(encoder)?;
            self.transforms.encode(encoder)
        }
    }

    impl<C> bincode::Decode<C> for AnimationChannel<[u8; 4]> {
        fn decode<D: bincode::de::Decoder<Context = C>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            Ok(Self {
                start: <[u8; 4]>::decode(decoder)?,
                transforms: <Vec<AnimationTransform<[u8; 4]>>>::decode(decoder)?,
            })
        }
    }

    impl<'de, C> bincode::BorrowDecode<'de, C> for AnimationChannel<[u8; 4]> {
        fn borrow_decode<D: bincode::de::BorrowDecoder<'de, Context = C>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            bincode::Decode::decode(decoder)
        }
    }

    impl bincode::Encode for AnimationTransform<[u8; 4]> {
        fn encode<E: bincode::enc::Encoder>(
            &self,
            encoder: &mut E,
        ) -> Result<(), bincode::error::EncodeError> {
            self.end.encode(encoder)?;
            self.duration.encode(encoder)?;
            self.first_step.encode(encoder)?;
            self.additional_steps.encode(encoder)
        }
    }

    impl<C> bincode::Decode<C> for AnimationTransform<[u8; 4]> {
        fn decode<D: bincode::de::Decoder<Context = C>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            Ok(Self {
                end: <[u8; 4]>::decode(decoder)?,
                duration: usize::decode(decoder)?,
                first_step: TransformStep::decode(decoder)?,
                additional_steps: <Vec<TransformStep>>::decode(decoder)?,
            })
        }
    }

    impl<'de, C> bincode::BorrowDecode<'de, C> for AnimationTransform<[u8; 4]> {
        fn borrow_decode<D: bincode::de::BorrowDecoder<'de, Context = C>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            bincode::Decode::decode(decoder)
        }
    }

    impl bincode::Encode for AnimationChannel<Vec2> {
        fn encode<E: bincode::enc::Encoder>(
            &self,
            encoder: &mut E,
        ) -> Result<(), bincode::error::EncodeError> {
            <[f32; 2]>::from(self.start).encode(encoder)?;
            self.transforms.encode(encoder)
        }
    }

    impl<C> bincode::Decode<C> for AnimationChannel<Vec2> {
        fn decode<D: bincode::de::Decoder<Context = C>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            Ok(Self {
                start: <[f32; 2]>::decode(decoder)?.into(),
                transforms: <Vec<AnimationTransform<Vec2>>>::decode(decoder)?,
            })
        }
    }

    impl<'de, C> bincode::BorrowDecode<'de, C> for AnimationChannel<Vec2> {
        fn borrow_decode<D: bincode::de::BorrowDecoder<'de, Context = C>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            bincode::Decode::decode(decoder)
        }
    }

    impl bincode::Encode for AnimationTransform<Vec2> {
        fn encode<E: bincode::enc::Encoder>(
            &self,
            encoder: &mut E,
        ) -> Result<(), bincode::error::EncodeError> {
            <[f32; 2]>::from(self.end).encode(encoder)?;
            self.duration.encode(encoder)?;
            self.first_step.encode(encoder)?;
            self.additional_steps.encode(encoder)
        }
    }

    impl<'de, C> bincode::BorrowDecode<'de, C> for AnimationTransform<Vec2> {
        fn borrow_decode<D: bincode::de::BorrowDecoder<'de, Context = C>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            bincode::Decode::decode(decoder)
        }
    }

    impl<C> bincode::Decode<C> for AnimationTransform<Vec2> {
        fn decode<D: bincode::de::Decoder<Context = C>>(
            decoder: &mut D,
        ) -> Result<Self, bincode::error::DecodeError> {
            Ok(Self {
                end: <[f32; 2]>::decode(decoder)?.into(),
                duration: usize::decode(decoder)?,
                first_step: TransformStep::decode(decoder)?,
                additional_steps: <Vec<TransformStep>>::decode(decoder)?,
            })
        }
    }
};

#[cfg_attr(feature = "asset", derive(bincode::Encode, bincode::Decode))]
#[derive(Clone)]
pub struct NodeAnimation {
    pub node_path: String,
    pub angle_channel: Option<AnimationChannel<f32>>,
    pub position_channel: Option<AnimationChannel<glam::Vec2>>,
    pub size_channel: Option<AnimationChannel<glam::Vec2>>,
    pub scale_channel: Option<AnimationChannel<glam::Vec2>>,
    pub color_channel: Option<AnimationChannel<[u8; 4]>>,

    // Texture node specific animations
    pub uv_offset_channel: Option<AnimationChannel<glam::Vec2>>,
    pub uv_scale_channel: Option<AnimationChannel<glam::Vec2>>,
}

impl NodeAnimation {
    pub fn animate<B: EnvyBackend>(&self, timer: f32, node: &mut NodeTransform, color: &mut [u8; 4], node_impl: &mut dyn Node<B>) -> bool {
        let mut is_done = true;

        if let Some(angle) = self.angle_channel.as_ref() {
            let mut transform_start = angle.start;
            let mut frame_start = 0.0;
            let transform = angle.transforms.iter().find(|transform| {
                if transform.duration as f32 + frame_start <= timer {
                    frame_start += transform.duration as f32;
                    transform_start = transform.end;
                    false
                } else {
                    true
                }
            });

            if let Some(transform) = transform {
                let mut progress = transform
                    .first_step
                    .transform((timer - frame_start) / transform.duration as f32);
                transform.additional_steps.iter().for_each(|transform| {
                    progress = transform.transform(progress);
                });

                node.angle = transform_start + progress * (transform.end - transform_start);
                is_done = false;
            } else {
                node.angle = *angle.last_value();
            }
        }

        if let Some(position) = self.position_channel.as_ref() {
            let mut transform_start = position.start;
            let mut frame_start = 0.0;
            let transform = position.transforms.iter().find(|transform| {
                if transform.duration as f32 + frame_start <= timer {
                    frame_start += transform.duration as f32;
                    transform_start = transform.end;
                    false
                } else {
                    true
                }
            });

            if let Some(transform) = transform {
                let mut progress = transform
                    .first_step
                    .transform((timer - frame_start) / transform.duration as f32);
                transform.additional_steps.iter().for_each(|transform| {
                    progress = transform.transform(progress);
                });

                node.position = transform_start + progress * (transform.end - transform_start);
                is_done = false;
            } else {
                node.position = *position.last_value();
            }
        }

        if let Some(size) = self.size_channel.as_ref() {
            let mut transform_start = size.start;
            let mut frame_start = 0.0;
            let transform = size.transforms.iter().find(|transform| {
                if transform.duration as f32 + frame_start <= timer {
                    frame_start += transform.duration as f32;
                    transform_start = transform.end;
                    false
                } else {
                    true
                }
            });

            if let Some(transform) = transform {
                let mut progress = transform
                    .first_step
                    .transform((timer - frame_start) / transform.duration as f32);
                transform.additional_steps.iter().for_each(|transform| {
                    progress = transform.transform(progress);
                });

                node.size = transform_start + progress * (transform.end - transform_start);
                is_done = false;
            } else {
                node.size = *size.last_value();
            }
        }

        if let Some(scale) = self.scale_channel.as_ref() {
            let mut transform_start = scale.start;
            let mut frame_start = 0.0;
            let transform = scale.transforms.iter().find(|transform| {
                if transform.duration as f32 + frame_start <= timer {
                    frame_start += transform.duration as f32;
                    transform_start = transform.end;
                    false
                } else {
                    true
                }
            });

            if let Some(transform) = transform {
                let mut progress = transform
                    .first_step
                    .transform((timer - frame_start) / transform.duration as f32);
                transform.additional_steps.iter().for_each(|transform| {
                    progress = transform.transform(progress);
                });

                node.scale = transform_start + progress * (transform.end - transform_start);
                is_done = false;
            } else {
                node.scale = *scale.last_value();
            }
        }

        if let Some(channel) = self.color_channel.as_ref() {
            let mut transform_start = channel.start;
            let mut frame_start = 0.0;
            let transform = channel.transforms.iter().find(|transform| {
                if transform.duration as f32 + frame_start <= timer {
                    frame_start += transform.duration as f32;
                    transform_start = transform.end;
                    false
                } else {
                    true
                }
            });

            if let Some(transform) = transform {
                let mut progress = transform
                    .first_step
                    .transform((timer - frame_start) / transform.duration as f32);
                transform.additional_steps.iter().for_each(|transform| {
                    progress = transform.transform(progress);
                });

                *color = <[u8; 4]>::interpolate(transform_start, transform.end, progress);
                is_done = false;
            } else {
                *color = *channel.last_value();
            }
        }

        if let Some(image) = node_impl.as_any_mut().downcast_mut::<ImageNode<B>>() {
            if let Some(channel) = self.uv_offset_channel.as_ref() {
                let mut transform_start = channel.start;
                let mut frame_start = 0.0;
                let transform = channel.transforms.iter().find(|transform| {
                    if transform.duration as f32 + frame_start <= timer {
                        frame_start += transform.duration as f32;
                        transform_start = transform.end;
                        false
                    } else {
                        true
                    }
                });

                if let Some(transform) = transform {
                    let mut progress = transform
                        .first_step
                        .transform((timer - frame_start) / transform.duration as f32);
                    transform.additional_steps.iter().for_each(|transform| {
                        progress = transform.transform(progress);
                    });

                    image.set_uv_offset(glam::Vec2::interpolate(transform_start, transform.end, progress));
                    is_done = false;
                } else {
                    image.set_uv_offset(*channel.last_value());
                }
            }

            if let Some(channel) = self.uv_scale_channel.as_ref() {
                let mut transform_start = channel.start;
                let mut frame_start = 0.0;
                let transform = channel.transforms.iter().find(|transform| {
                    if transform.duration as f32 + frame_start <= timer {
                        frame_start += transform.duration as f32;
                        transform_start = transform.end;
                        false
                    } else {
                        true
                    }
                });

                if let Some(transform) = transform {
                    let mut progress = transform
                        .first_step
                        .transform((timer - frame_start) / transform.duration as f32);
                    transform.additional_steps.iter().for_each(|transform| {
                        progress = transform.transform(progress);
                    });

                    image.set_uv_scale(glam::Vec2::interpolate(transform_start, transform.end, progress));
                    is_done = false;
                } else {
                    image.set_uv_scale(*channel.last_value());
                }
            }
        }

        is_done
    }
}

#[cfg_attr(feature = "asset", derive(bincode::Encode, bincode::Decode))]
#[derive(Clone)]
pub struct Animation {
    pub node_animations: Vec<NodeAnimation>,
    pub total_duration: usize,
}
