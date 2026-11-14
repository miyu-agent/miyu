//! 供应商侧工具（施工 HT-1）：编号接在工具调用后面的收、混着数的拒；它不欠结果，回合不用等它；旧版本只数工具调用
//! 也读得进。

use super::*;

fn hosted(id: &str) -> String {
    format!(
        r#"{{"type":"hosted_tool","call_id":"{id}","name":"web_search","args":"{{}}","outcome":{{"text":"ok"}}}}"#
    )
}

fn call(id: &str) -> String {
    format!(r#"{{"type":"tool_call","call_id":"{id}","name":"read","args":"{{}}"}}"#)
}

fn reply_of(blocks: &[String]) -> String {
    format!(r#"{{"blocks":[{}],"seen":4}}"#, blocks.join(","))
}

/// `[供应商侧, 工具调用, 供应商侧]` 照 `_2`、`_1`、`_3` 收；混着数的、排在工具调用前面的、跳号的拒。
#[test]
fn hosted_tools_are_numbered_after_the_tool_calls() {
    let mut ledger = after(4);
    let good = reply_of(&[hosted("call_5_2"), call("call_5_1"), hosted("call_5_3")]);
    ledger
        .append(&event(5, Some(3), "message.assistant", &good))
        .unwrap();
    assert_eq!(ledger.pending_calls(), [CallId::parse("call_5_1").unwrap()]);

    for (bad, why) in [
        (
            reply_of(&[hosted("call_5_1"), call("call_5_2"), hosted("call_5_3")]),
            "tool call 1 should have id call_5_1, got call_5_2",
        ),
        (
            reply_of(&[hosted("call_5_1"), call("call_5_1")]),
            "hosted tool 2 should have id call_5_2, got call_5_1",
        ),
        (
            reply_of(&[call("call_5_1"), hosted("call_5_3")]),
            "hosted tool 2 should have id call_5_2, got call_5_3",
        ),
        (
            reply_of(&[hosted("call_4_1")]),
            "hosted tool 1 should have id call_5_1, got call_4_1",
        ),
    ] {
        let mut ledger = after(4);
        refused(
            &mut ledger,
            &event(5, Some(3), "message.assistant", &bad),
            why,
        );
    }
}

/// 它不欠结果：只有它的回复后面不用等 `tool.result` 就能结束回合；给它发结果的拒。
#[test]
fn a_hosted_tool_is_not_waiting_for_a_result() {
    let mut ledger = after(4);
    let only = reply_of(&[hosted("call_5_1")]);
    ledger
        .append(&event(5, Some(3), "message.assistant", &only))
        .unwrap();
    assert!(ledger.pending_calls().is_empty());
    refused(
        &mut ledger.clone(),
        &event(6, Some(3), "tool.result", &result("call_5_1", "ok")),
        "call_5_1 is not a call waiting for a result",
    );
    ledger
        .append(&event(
            6,
            Some(3),
            "turn.ended",
            r#"{"reason":"completed"}"#,
        ))
        .unwrap();
}

/// 旧版本读得进：累积器拼出的 `[供应商侧, 工具调用]` 写成日志行，把 `hosted_tool` 那块除了 `type` 当黑盒（旧版本
/// 不认识它），照旧版本 `check_call_ids` 的规矩只数工具调用：第 k 个工具调用的编号是 `call_<序号>_<k>`。
#[test]
fn an_old_version_still_reads_the_tool_call_ids() {
    use crate::accumulate::{Accumulator, Delta, Kind};
    let mut accumulator = Accumulator::default();
    for delta in [
        Delta::Start {
            index: 0,
            kind: Kind::HostedTool {
                name: "web_search".to_string(),
            },
        },
        Delta::End { index: 0 },
        Delta::Start {
            index: 1,
            kind: Kind::ToolCall {
                name: "read".to_string(),
            },
        },
        Delta::End { index: 1 },
    ] {
        accumulator.apply(delta).unwrap();
    }
    let seq = Seq::new(5).unwrap();
    let line = serde_json::to_string(&accumulator.finish(seq)).unwrap();
    let blocks: Vec<serde_json::Value> = serde_json::from_str(&line).unwrap();
    let old: Vec<&str> = blocks
        .iter()
        .filter(|block| block["type"] == "tool_call")
        .map(|block| block["call_id"].as_str().unwrap())
        .collect();
    assert_eq!(old, ["call_5_1"], "{line}");
    assert_eq!(blocks[0]["type"], "hosted_tool", "{line}");
}
