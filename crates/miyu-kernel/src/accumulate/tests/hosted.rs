//! 供应商侧工具（施工 HT-1）：拼成块、结果的规矩、截长的、打断时一律留下、编号接在工具调用后面。

use super::*;
use crate::block::{HostedTool, Outcome};

fn search() -> Kind {
    Kind::HostedTool {
        name: "web_search".to_string(),
    }
}

fn outcome(index: usize, text: &str, failed: bool) -> Delta {
    Delta::Outcome {
        index,
        text: text.to_string(),
        failed,
    }
}

fn hosted(k: u32, args: &str, outcome: Option<Outcome>) -> Block {
    Block::HostedTool(HostedTool {
        call_id: call(k),
        name: "web_search".to_string(),
        args: args.to_string(),
        outcome,
        private: None,
    })
}

fn found(text: &str) -> Option<Outcome> {
    Some(Outcome {
        text: text.to_string(),
        failed: false,
        cut: false,
    })
}

/// 参数原文、结果、私有数据拼进一块；失败的带 `failed`。
#[test]
fn a_hosted_tool_is_put_together() {
    let blocks = fed(vec![
        start(0, search()),
        text(0, r#"{"query":"#),
        text(0, r#""rust"}"#),
        Delta::Private {
            index: 0,
            private: provider_id("srvtoolu_1"),
        },
        outcome(0, "1. Rust", false),
        end(0),
        start(1, search()),
        text(1, "{}"),
        outcome(1, "超时", true),
        end(1),
    ])
    .finish(reply());
    assert_eq!(
        blocks,
        [
            Block::HostedTool(HostedTool {
                call_id: call(1),
                name: "web_search".to_string(),
                args: r#"{"query":"rust"}"#.to_string(),
                outcome: found("1. Rust"),
                private: Some(provider_id("srvtoolu_1")),
            }),
            hosted(
                2,
                "{}",
                Some(Outcome {
                    text: "超时".to_string(),
                    failed: true,
                    cut: false,
                })
            ),
        ]
    );
}

/// 编号不和工具调用混着数：工具调用照旧 `_1.._k`，供应商侧工具照先后接着排；块的先后还是流里的先后。
#[test]
fn hosted_tools_are_numbered_after_the_tool_calls() {
    let deltas = || {
        vec![
            start(0, search()),
            text(0, "a"),
            end(0),
            start(1, read()),
            text(1, "{}"),
            end(1),
            start(2, search()),
            text(2, "b"),
            end(2),
        ]
    };
    let ids = |blocks: Vec<Block>| -> Vec<String> {
        blocks
            .iter()
            .map(|block| match block {
                Block::ToolCall(call) => call.call_id.to_string(),
                Block::HostedTool(hosted) => hosted.call_id.to_string(),
                other => panic!("{other:?}"),
            })
            .collect()
    };
    assert_eq!(
        ids(fed(deltas()).finish(reply())),
        ["call_45_2", "call_45_1", "call_45_3"]
    );
    assert_eq!(
        ids(fed(deltas()).cut_off(reply())),
        ["call_45_2", "call_45_1", "call_45_3"]
    );
}

/// 打断时丢掉的半截工具调用不占号：留下的工具调用数是 k，供应商侧工具从 `_k+1` 排。
#[test]
fn a_dropped_tool_call_takes_no_number_from_a_hosted_tool() {
    let blocks = fed(vec![
        start(0, read()),
        text(0, r#"{"path":"#),
        start(1, search()),
        text(1, "a"),
        end(1),
        start(2, read()),
        text(2, "{}"),
        end(2),
    ])
    .cut_off(reply());
    assert_eq!(
        blocks,
        [
            hosted(2, "a", None),
            Block::ToolCall(ToolCall {
                call_id: call(1),
                name: "read".to_string(),
                args: "{}".to_string(),
                private: None,
            }),
        ]
    );
}

/// 供应商那边已经跑了：没收全的、没参数的、被打断的一律留下；收到了结果的带着，没收到的没有 `outcome`。
#[test]
fn a_hosted_tool_stays_whether_or_not_it_ended() {
    let deltas = || {
        vec![
            start(0, search()),
            text(0, "a"),
            outcome(0, "找到了", false),
            start(1, search()),
            text(1, "b"),
            start(2, search()),
        ]
    };
    let expected = [
        hosted(1, "a", found("找到了")),
        hosted(2, "b", None),
        hosted(3, "", None),
    ];
    assert_eq!(fed(deltas()).cut_off(reply()), expected);
    assert_eq!(fed(deltas()).finish(reply()), expected);
}

/// 超过上限的截在不超过它的最后一个字的边界上、带 `cut`；正好上限的不截。
#[test]
fn a_long_outcome_is_cut_on_a_char_boundary() {
    let exact = "a".repeat(OUTCOME_BYTES);
    assert_eq!(
        cut_outcome(exact.clone(), false),
        Outcome {
            text: exact,
            failed: false,
            cut: false
        }
    );
    // 「好」三个字节，跨过上限：整个字不要。
    let long = format!("{}好好", "a".repeat(OUTCOME_BYTES - 1));
    let cut = cut_outcome(long, true);
    assert_eq!(cut.text, "a".repeat(OUTCOME_BYTES - 1));
    assert!(cut.cut && cut.failed);
    // 正好落在边界上的不多丢。
    let long = format!("{}好a", "a".repeat(OUTCOME_BYTES - 3));
    assert_eq!(
        cut_outcome(long, false).text,
        format!("{}好", "a".repeat(OUTCOME_BYTES - 3))
    );
    // 累积器收下时就截。
    let blocks = fed(vec![
        start(0, search()),
        outcome(0, &format!("{}好好", "a".repeat(OUTCOME_BYTES - 1)), false),
    ])
    .cut_off(reply());
    let [
        Block::HostedTool(HostedTool {
            outcome: Some(got), ..
        }),
    ] = blocks.as_slice()
    else {
        panic!("{blocks:?}");
    };
    assert_eq!(got.text.len(), OUTCOME_BYTES - 1);
    assert!(got.cut);
}

/// 中途订阅的补推拿得到到目前为止的结果；别的块没有。
#[test]
fn so_far_gives_the_outcome() {
    let accumulator = fed(vec![
        start(0, Kind::Text),
        text(0, "好"),
        start(1, search()),
        text(1, "a"),
        outcome(1, "找到了", false),
        start(2, search()),
    ]);
    let outcomes: Vec<Option<&Outcome>> = accumulator.so_far().map(|(_, _, _, got)| got).collect();
    assert_eq!(outcomes, [None, found("找到了").as_ref(), None]);
}

/// 结果只给供应商侧工具、一块至多一次、要在收全以前；私有数据至多一份。
#[test]
fn mismatched_outcomes_are_driver_errors() {
    let cases: Vec<(Vec<Delta>, Delta, &str)> = vec![
        (vec![], outcome(0, "x", false), "还没开始"),
        (
            vec![start(0, Kind::Text)],
            outcome(0, "x", false),
            "只有供应商侧工具",
        ),
        (
            vec![start(0, Kind::Reasoning)],
            outcome(0, "x", false),
            "只有供应商侧工具",
        ),
        (
            vec![start(0, read())],
            outcome(0, "x", false),
            "只有供应商侧工具",
        ),
        (
            vec![start(0, search()), outcome(0, "x", false)],
            outcome(0, "y", true),
            "结果来了两次",
        ),
        (
            vec![start(0, search()), end(0)],
            outcome(0, "x", false),
            "已经收全了",
        ),
        (
            vec![
                start(0, search()),
                Delta::Private {
                    index: 0,
                    private: provider_id("a"),
                },
            ],
            Delta::Private {
                index: 0,
                private: provider_id("b"),
            },
            "来了两次",
        ),
    ];
    for (before, delta, why) in cases {
        let mut accumulator = fed(before);
        let error = accumulator.apply(delta.clone()).unwrap_err();
        assert!(error.why.contains(why), "{delta:?}：{error}");
    }
}
