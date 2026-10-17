//! 正在流的块（施工 9-8 六补，`protocol.md`「补发」第 4 条）：中途订阅的头要的那一份，照原来的形状拼好。

use super::executor::*;
use super::*;
use crate::accumulate::{Delta, Kind};
use crate::event::{Piece, TransientBody};

/// 正在流的块（施工 9-8 六补）：还没发出去的没有；到目前为止的每块照原来的形状（`start`、拼好的字、收全了的 `end`），
/// 工具调用带名字，和推过的同一个 `turn`、`by`、`cause`；说完了的没有。
#[test]
fn the_blocks_streamed_so_far_are_rebuilt_for_a_late_subscriber() {
    let mut session = asking();
    assert!(session.streaming(at(50)).is_empty(), "还没发出去");
    session.handle(sent(5));
    assert!(session.streaming(at(50)).is_empty(), "一段都还没来");
    for piece in words(0, "你好") {
        session.handle(delta(5, 41, piece));
    }
    session.handle(delta(
        5,
        42,
        Delta::Start {
            index: 1,
            kind: Kind::ToolCall {
                name: "read".to_string(),
            },
        },
    ));
    session.handle(delta(
        5,
        42,
        Delta::Text {
            index: 1,
            text: "{\"pa".to_string(),
        },
    ));
    session.handle(delta(
        5,
        43,
        Delta::Text {
            index: 1,
            text: "th\"".to_string(),
        },
    ));
    let streaming = session.streaming(at(50));
    let pieces: Vec<(usize, Piece)> = streaming
        .iter()
        .map(|transient| {
            assert_eq!(transient.at, at(50));
            assert_eq!(transient.turn, Some(turn3()));
            assert_eq!(transient.by, By::Model(deepseek()));
            assert_eq!(transient.cause, Some(id(1)));
            let TransientBody::ModelDelta(body) = &transient.body else {
                panic!("应该是 model.delta：{transient:?}");
            };
            assert_eq!(body.seen, seq(5));
            (body.index, body.piece.clone())
        })
        .collect();
    assert_eq!(
        pieces,
        [
            (0, Piece::Start(Kind::Text)),
            (0, Piece::Text("你好".to_string())),
            (0, Piece::End),
            (
                1,
                Piece::Start(Kind::ToolCall {
                    name: "read".to_string()
                })
            ),
            (1, Piece::Text("{\"path\"".to_string())),
        ]
    );
    session.handle(delta(5, 44, Delta::End { index: 1 }));
    session.handle(ended(5));
    assert!(session.streaming(at(51)).is_empty(), "说完了没有在流的");
}
