use std::time::Instant;

use string_combiner::StringCombiner;

fn main() {
    let inputs = vec![
        "Hello World".chars(),
        "Hello worl d!".chars(),
        "내 어린시절 우연히?".chars(),
        "시찰 우연히 들었던 ".chars(),
        "우연히 들었던 믿지 못할 한 마디".chars(),
        "Hello bold".chars(),
    ];

    let combiner = StringCombiner::default();

    let instant = Instant::now();
    let combined = combiner
        .concat_strings(inputs)
        .expect("Failed to concat texts");

    println!("Output: {combined}");
    println!("Elapsed: {:?}", instant.elapsed());

    let expected = "내 어린시절 우연히 들었던 믿지 못할 한 마디";
    assert_eq!(expected, combined);
}
