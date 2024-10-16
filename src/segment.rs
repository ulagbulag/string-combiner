use std::{fmt, ops, time::Duration};

use serde::{Deserialize, Serialize};

use crate::{
    msa::{AlignedSequence, AlignedToken, GreedyMultipleSequenceAlignerItem},
    token::TokenData,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Segment<K = SegmentKey, V = SegmentValue> {
    pub key: K,
    pub value: V,
}

impl<K, V> ops::Deref for Segment<K, V> {
    type Target = V;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<K, V> AsRef<AlignedSequence<V>> for Segment<K, AlignedSequence<V>> {
    #[inline]
    fn as_ref(&self) -> &AlignedSequence<V> {
        &self.value
    }
}

impl<K, V> AsRef<[AlignedToken<V>]> for Segment<K, AlignedSequence<V>> {
    #[inline]
    fn as_ref(&self) -> &[AlignedToken<V>] {
        &self.value.value
    }
}

impl<K, V> AsRef<Self> for Segment<K, AlignedSequence<V>> {
    #[inline]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<K, V> fmt::Display for Segment<K, V>
where
    V: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}

impl<V> GreedyMultipleSequenceAlignerItem<V> for Segment<SegmentKey, AlignedSequence<V>> {
    #[inline]
    fn build(x: &Self, y: &Self, value: AlignedSequence<V>) -> Self
    where
        Self: Sized,
    {
        Self {
            key: SegmentKey {
                t0: x.key.t0.min(y.key.t0),
                t1: x.key.t1.max(y.key.t1),
            },
            value,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SegmentKey {
    // NOTE: fields are ordered
    pub t1: Duration,
    pub t0: Duration,
}

impl SegmentKey {
    pub fn duration(&self) -> Duration {
        self.t1 - self.t0
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum SegmentKind {
    Normal {
        last_index: usize,
        last_offset: Option<Duration>,
        total_period: Duration,
    },
    Selected,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SegmentValue {
    pub kind: SegmentKind,
    pub text: String,
    pub tokens: Vec<TokenData>,
}
