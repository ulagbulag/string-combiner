use std::time::Instant;

use string_combiner::{
    msa::{
        AlignedSequence, AlignedToken, GreedyMultipleSequenceAligner, MultipleSequenceAlignment,
        Scoring, SequenceMatch,
    },
    token::AlignmentTokenMergeVisitor,
};

fn main() {
    let inputs = vec![
        "Hello World".chars(),
        "Hello worl d!".chars(),
        "내 어린시절 우연히?".chars(),
        "시찰 우연히 들었던 ".chars(),
        "우연히 들었던 믿지 못할 한 마디".chars(),
        "Hello bold".chars(),
    ]
    .into_iter()
    .map(AlignedSequence::from_iter);

    let score = |a: &AlignedToken<_>, b: &AlignedToken<_>| if a == b { 2i32 } else { -3i32 };
    let scoring = Scoring::new(-5, -1, score);

    #[allow(unused_variables)]
    fn match_fn(
        a: &AlignedSequence<char>,
        b: &AlignedSequence<char>,
    ) -> SequenceMatch<AlignedSequence<char>> {
        SequenceMatch::Matched
    }

    fn score_fn(s: &AlignedSequence<char>) -> Option<usize> {
        if s.num_deleted_x < 3 && s.num_deleted_y < 3 {
            Some(s.total_matched())
        } else {
            None
        }
    }

    let aligner = GreedyMultipleSequenceAligner::new(match_fn, score_fn);
    let visitor = AlignmentTokenMergeVisitor::default();

    let instant = Instant::now();
    let combined = aligner
        .reduce_all(scoring, visitor, inputs)
        .expect("Failed to concat texts");

    println!("{combined:?}");
    println!("Output: {combined}");
    println!("Elapsed: {:?}", instant.elapsed());

    let expected = "내 어린시절 우연히 들었던 믿지 못할 한 마디";
    assert_eq!(expected, combined.to_string());
}
