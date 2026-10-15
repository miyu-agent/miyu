use serde_json::json;

use super::*;

fn texts(blocks: Option<Vec<Inject>>) -> Option<Vec<String>> {
    blocks.map(|blocks| blocks.into_iter().map(|block| block.text).collect())
}

#[test]
fn a_well_formed_answer_is_taken_as_is() {
    let answer =
        json!({"inject": [{"kind": "meme", "text": "a"}, {"kind": "meme_2", "text": "b"}]});
    let taken = blocks("memes", answer).expect("写法对");
    assert_eq!(taken.len(), 2);
    assert_eq!(
        (
            taken[0].package.as_str(),
            taken[0].kind.as_str(),
            taken[0].text.as_str()
        ),
        ("memes", "meme", "a")
    );
    assert_eq!(
        texts(blocks("memes", json!({"inject": []}))),
        Some(Vec::new()),
        "空的是没交"
    );
}

#[test]
fn at_most_three_blocks() {
    let block = json!({"kind": "meme", "text": "a"});
    assert!(blocks("memes", json!({"inject": [block, block, block]})).is_some());
    assert!(blocks("memes", json!({"inject": [block, block, block, block]})).is_none());
}

#[test]
fn text_is_one_to_four_thousand_characters() {
    let longest = "字".repeat(MOST_CHARS);
    assert!(
        blocks(
            "memes",
            json!({"inject": [{"kind": "meme", "text": longest}]})
        )
        .is_some()
    );
    let over = "字".repeat(MOST_CHARS + 1);
    assert!(blocks("memes", json!({"inject": [{"kind": "meme", "text": over}]})).is_none());
    assert!(blocks("memes", json!({"inject": [{"kind": "meme", "text": ""}]})).is_none());
}

#[test]
fn one_bad_block_drops_the_whole_answer() {
    let good = json!({"kind": "meme", "text": "a"});
    for bad in [
        json!({"kind": "Bad Kind", "text": "a"}),
        json!({"kind": "meme", "text": "a", "extra": 1}),
        json!({"kind": "meme"}),
    ] {
        assert!(
            blocks("memes", json!({"inject": [good, bad]})).is_none(),
            "{bad}"
        );
    }
    assert!(
        blocks("memes", json!({"inject": [good], "more": 1})).is_none(),
        "多的格"
    );
    assert!(blocks("memes", json!({})).is_none(), "没有 inject");
    assert!(
        blocks("Bad Package", json!({"inject": [good]})).is_none(),
        "包的编号不合写法"
    );
}
