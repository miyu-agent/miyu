//! 只交正文的字；换了一次请求的先交从头再来，没交过字的不交。

use miyu_kernel::id::Seq;

use super::*;

fn delta(index: usize, piece: Piece) -> ModelDelta {
    ModelDelta {
        seen: Seq::new(1).expect("合写法"),
        index,
        piece,
    }
}

/// 照先后看完，交回交出来的：正文的字照原样，从头再来写成 `↺`。
fn told(deltas: &[ModelDelta]) -> Vec<String> {
    let mut stream = Stream::default();
    let mut heard = Vec::new();
    for delta in deltas {
        stream.see(delta, &mut |watched| {
            heard.push(match watched {
                Watched::Text(text) => text.to_string(),
                Watched::Restart => "↺".to_string(),
            });
        });
    }
    heard
}

#[test]
fn only_the_answer_is_told() {
    let heard = told(&[
        delta(0, Piece::Start(Kind::Reasoning)),
        delta(0, Piece::Text("想一想".to_string())),
        delta(0, Piece::End),
        delta(1, Piece::Start(Kind::Text)),
        delta(1, Piece::Text("{\"a\"".to_string())),
        delta(1, Piece::Text(":1}".to_string())),
        delta(1, Piece::End),
    ]);
    assert_eq!(heard, ["{\"a\"", ":1}"]);
}

#[test]
fn a_new_request_starts_over() {
    let heard = told(&[
        delta(0, Piece::Start(Kind::Text)),
        delta(0, Piece::Text("半截".to_string())),
        delta(0, Piece::Start(Kind::Text)),
        delta(0, Piece::Text("整段".to_string())),
    ]);
    assert_eq!(heard, ["半截", "↺", "整段"]);
}

#[test]
fn a_restart_before_any_answer_is_not_told() {
    let heard = told(&[
        delta(0, Piece::Start(Kind::Reasoning)),
        delta(0, Piece::Text("想".to_string())),
        delta(0, Piece::Start(Kind::Text)),
        delta(0, Piece::Text("答".to_string())),
    ]);
    assert_eq!(heard, ["答"], "只想了没答的不用作废");
}
