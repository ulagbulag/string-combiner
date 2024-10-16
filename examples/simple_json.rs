use std::{fs::File, time::Instant};

use string_combiner::{segment::Segment, StringCombiner};

fn main() {
    let segments: Vec<Segment> = ::serde_json::from_reader(
        File::open("./examples/data/live-game-streaming.json").expect("Failed to get data file"),
    )
    .expect("Failed to parse data file");

    let inputs = segments.into_iter().map(|Segment { key, value }| Segment {
        key,
        value: value.text.into_bytes(),
    });

    let combiner = StringCombiner::default();

    let instant = Instant::now();
    let combined = combiner
        .concat_segments(inputs)
        .expect("Failed to concat segments");
    let combined = String::from_utf8_lossy(&combined);

    println!("Output: {combined}");
    println!("Elapsed: {:?}", instant.elapsed());

    let expected = r#" 이렇게 지구를 구한 거 맞지
 아니요 이번엔 아무 일도 일어나지 않았습니다,
 실패했다 실패했다 실패했다 실패했다
 아니 뭘 실패했는데
 왜 그래?
 이걸로 지구는 멸망했습니다
 지금은 그렇게 보이겠지만
 나비효과라는 말을 아십니까
 어 과거에 사소한 일이 미래에 거대한 영향을 끼친다는 거지
 화재경보기를 울리는 건 미래에 확정된 멸망을 피할 수 있었던 신락같은 가능성
 그 기회를 놓쳤으니 지구의 멸망은 어떻게도 피할 수 없습니다
 아이 그래 알았어 그래서 내가 어떻게 뭐 하면 되는데
 냉장고 코드라도 뽑을까?
 은박지 넣어서 전자레인지 돌리면 돼
 이젠 다 소용없습니다
 실패했다.
 문제는 무의미했다
 자폭해야지.
 이게 용사님이 원하던 결말인가요?
 왜 화재 경보기를 울리지 않으셨죠?
 그 이후 메르헬은 두번 다시 내 앞에 나타나지 않았다."#;
    assert_eq!(expected.replace('\n', ""), combined);
}
