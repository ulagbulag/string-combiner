use std::{fs::File, time::Instant};

use string_combiner::{segment::Segment, StringCombiner};

fn main() {
    let segments: Vec<Segment> = ::serde_json::from_reader(
        File::open("./examples/data/live-game-streaming.json").expect("Failed to get data file"),
    )
    .expect("Failed to parse data file");

    let inputs = segments.into_iter().map(|Segment { key, value }| Segment {
        key,
        value: value.tokens,
    });

    let combiner = StringCombiner::default();

    let instant = Instant::now();
    let combined = combiner
        .concat_segments(inputs)
        .expect("Failed to concat segments");

    // println!("Output: {combined}");
    println!("Elapsed: {:?}", instant.elapsed());
}
