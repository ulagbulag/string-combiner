pub mod msa;
pub mod segment;
pub mod token;

use std::fmt;

use crate::{
    msa::{
        AlignedSequence, AlignedToken, GreedyMultipleSequenceAligner,
        GreedyMultipleSequenceAlignerItem, MultipleSequenceAlignment, Scoring, SequenceMatch,
    },
    segment::{Segment, SegmentKey},
    token::AlignmentTokenMergeVisitor,
};

#[derive(Clone, Debug)]
pub struct StringCombiner {
    pub gap_extend: i32,
    pub gap_open: i32,
    pub threshold_deletion: usize,
}

impl Default for StringCombiner {
    fn default() -> Self {
        Self {
            gap_extend: -1,
            gap_open: -5,
            threshold_deletion: 3,
        }
    }
}

impl StringCombiner {
    pub fn concat_segments<I, T, IT>(&self, inputs: I) -> Option<Segment<SegmentKey, Vec<T>>>
    where
        I: IntoIterator<Item = Segment<SegmentKey, IT>>,
        IT: IntoIterator<Item = T>,
        T: Send + Sync + Clone + Eq,
    {
        fn match_fn<T>(
            a: &Segment<SegmentKey, AlignedSequence<T>>,
            b: &Segment<SegmentKey, AlignedSequence<T>>,
        ) -> SequenceMatch<Segment<SegmentKey, AlignedSequence<T>>>
        where
            T: Clone,
        {
            if a.key.t1 > b.key.t0 {
                SequenceMatch::Matched
            } else {
                SequenceMatch::Custom(Segment {
                    key: SegmentKey {
                        t0: a.key.t0.min(b.key.t0),
                        t1: a.key.t1.max(b.key.t1),
                    },
                    value: a.value.join(&b.value, None),
                })
            }
        }

        let inputs = inputs.into_iter().map(|Segment { key, value }| Segment {
            key,
            value: AlignedSequence::from_iter(value),
        });
        self.concat_with::<_, _, _>(inputs, match_fn)
            .map(|Segment { key, value }| Segment {
                key,
                value: value.value.into_iter().map(|token| token.data).collect(),
            })
    }

    pub fn concat_strings<I, T>(&self, inputs: I) -> Option<String>
    where
        AlignedSequence<T>: fmt::Display,
        I: IntoIterator,
        <I as IntoIterator>::Item: IntoIterator<Item = T>,
        T: Send + Sync + Clone + Eq,
    {
        fn match_fn<T>(
            _a: &AlignedSequence<T>,
            _b: &AlignedSequence<T>,
        ) -> SequenceMatch<AlignedSequence<T>> {
            SequenceMatch::Matched
        }

        let inputs = inputs.into_iter().map(AlignedSequence::from_iter);
        self.concat_with(inputs, match_fn)
            .map(|seq| seq.to_string())
    }

    pub fn concat_with<I, T, F>(&self, inputs: I, match_fn: F) -> Option<I::Item>
    where
        F: Sync + Fn(&I::Item, &I::Item) -> SequenceMatch<I::Item>,
        I: IntoIterator,
        I::Item: Send
            + Sync
            + Clone
            + AsRef<AlignedSequence<T>>
            + AsRef<[AlignedToken<T>]>
            + AsRef<I::Item>
            + GreedyMultipleSequenceAlignerItem<T>,
        T: Send + Sync + Clone + Eq,
    {
        let score = |a: &AlignedToken<_>, b: &AlignedToken<_>| if a == b { 2i32 } else { -3i32 };
        let scoring = Scoring::new(self.gap_open, self.gap_extend, score);

        let score_fn = |s: &I::Item| -> Option<usize> {
            let s: &AlignedSequence<T> = s.as_ref();
            if s.num_deleted_x <= self.threshold_deletion
                && s.num_deleted_y <= self.threshold_deletion
            {
                Some(s.total_matched())
            } else {
                None
            }
        };

        let aligner = GreedyMultipleSequenceAligner::new(match_fn, score_fn);
        let visitor = AlignmentTokenMergeVisitor::default();
        aligner.reduce_all(scoring, visitor, inputs)
    }
}

#[cfg(test)]
mod tests {
    use crate::StringCombiner;

    #[test]
    fn test_iter_empty() {
        let inputs: Vec<Vec<char>> = vec![];
        let combiner = StringCombiner::default();
        let expected = None;
        let combined = combiner.concat_strings(inputs);
        assert_eq!(expected, combined.as_deref())
    }

    #[test]
    fn test_iter_single() {
        let inputs = vec!["Hello World".chars()];
        let combiner = StringCombiner::default();
        let expected = Some("Hello World");
        let combined = combiner.concat_strings(inputs);
        assert_eq!(expected, combined.as_deref())
    }

    #[test]
    fn test_iter_multiple() {
        let inputs = vec!["Hello World".chars(), "World!".chars()];
        let combiner = StringCombiner::default();
        let expected = Some("Hello World!");
        let combined = combiner.concat_strings(inputs);
        assert_eq!(expected, combined.as_deref())
    }

    #[test]
    fn test_errorous_inputs_binary() {
        let inputs = vec!["Hello World".chars(), "world!".chars()];
        let combiner = StringCombiner::default();
        let expected = Some("Hello World!");
        let combined = combiner.concat_strings(inputs);
        assert_eq!(expected, combined.as_deref())
    }

    #[test]
    fn test_errorous_inputs_keep_left() {
        let inputs = vec![
            "Hello World".chars(),
            "Hello world".chars(),
            "world".chars(),
            "world!".chars(),
            "world! My name is".chars(),
            "world! My name is Ho Kim.".chars(),
        ];
        let combiner = StringCombiner::default();
        let expected = Some("Hello World! My name is Ho Kim.");
        let combined = combiner.concat_strings(inputs);
        assert_eq!(expected, combined.as_deref())
    }

    #[test]
    fn test_errorous_inputs_shift() {
        let inputs = vec![
            "Hello World".chars(),
            "Hello world".chars(),
            "world".chars(),
            "world!".chars(),
            "world! My name is".chars(),
            "world! My name is Ho Kim.".chars(),
        ];
        let combiner = StringCombiner {
            threshold_deletion: 0,
            ..Default::default()
        };
        let expected = Some("Hello world! My name is Ho Kim.");
        let combined = combiner.concat_strings(inputs);
        assert_eq!(expected, combined.as_deref())
    }
}
