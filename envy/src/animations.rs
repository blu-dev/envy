use crate::NodeTransform;

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

#[cfg(feature = "asset")]
const _: () = {
    use glam::Vec2;

    impl bincode::Encode for AnimationChannel<f32> {
        fn encode<E: bincode::enc::Encoder>(&self, encoder: &mut E) -> Result<(), bincode::error::EncodeError> {
            self.start.encode(encoder)?;
            self.transforms.encode(encoder)
        }
    }

    impl<C> bincode::Decode<C> for AnimationChannel<f32> {
        fn decode<D: bincode::de::Decoder<Context = C>>(decoder: &mut D) -> Result<Self, bincode::error::DecodeError> {
            Ok(Self {
                start: f32::decode(decoder)?,
                transforms: <Vec<AnimationTransform<f32>>>::decode(decoder)?
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
        fn encode<E: bincode::enc::Encoder>(&self, encoder: &mut E) -> Result<(), bincode::error::EncodeError> {
            self.end.encode(encoder)?;
            self.duration.encode(encoder)?;
            self.first_step.encode(encoder)?;
            self.additional_steps.encode(encoder)
        }
    }

    impl<C> bincode::Decode<C> for AnimationTransform<f32> {
        fn decode<D: bincode::de::Decoder<Context = C>>(decoder: &mut D) -> Result<Self, bincode::error::DecodeError> {
            Ok(Self {
                end: f32::decode(decoder)?,
                duration: f32::decode(decoder)?,
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

    impl bincode::Encode for AnimationChannel<Vec2> {
        fn encode<E: bincode::enc::Encoder>(&self, encoder: &mut E) -> Result<(), bincode::error::EncodeError> {
            <[f32; 2]>::from(self.start).encode(encoder)?;
            self.transforms.encode(encoder)
        }
    }

    impl<C> bincode::Decode<C> for AnimationChannel<Vec2> {
        fn decode<D: bincode::de::Decoder<Context = C>>(decoder: &mut D) -> Result<Self, bincode::error::DecodeError> {
            Ok(Self {
                start: <[f32; 2]>::decode(decoder)?.into(),
                transforms: <Vec<AnimationTransform<Vec2>>>::decode(decoder)?
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
        fn encode<E: bincode::enc::Encoder>(&self, encoder: &mut E) -> Result<(), bincode::error::EncodeError> {
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
        fn decode<D: bincode::de::Decoder<Context = C>>(decoder: &mut D) -> Result<Self, bincode::error::DecodeError> {
            Ok(Self {
                end: <[f32; 2]>::decode(decoder)?.into(),
                duration: f32::decode(decoder)?,
                first_step: TransformStep::decode(decoder)?,
                additional_steps: <Vec<TransformStep>>::decode(decoder)?
            })
        }
    }
};

#[cfg_attr(feature = "asset", derive(bincode::Encode, bincode::Decode))]
#[derive(Clone)]
pub struct NodeAnimation {
    pub node_path: String,
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

#[cfg_attr(feature = "asset", derive(bincode::Encode, bincode::Decode))]
#[derive(Clone)]
pub struct Animation {
    pub node_animations: Vec<NodeAnimation>,
}
