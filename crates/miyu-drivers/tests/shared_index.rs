//! 两次工具调用写同一个块编号（施工 FX-20，同 AU-020 的 OpenAI Chat、LiteMiyu LM-36）：Anthropic 的 `index`、Responses 的
//! `output_index` 照协议每块不一样；有的兼容层并行的几次调用都写 0，靠各自的 `id` 分成两次，参数不拼在一起，两块都收。

use std::collections::BTreeMap;

use miyu_drivers::{anthropic, openai_responses};
use miyu_kernel::accumulate::{Delta, Kind};

/// 增量里的块：编号到种类（工具调用写工具名）和接起来的字，照编号排。
fn blocks(deltas: &[Delta]) -> Vec<(String, String)> {
    let mut found: BTreeMap<usize, (String, String)> = BTreeMap::new();
    for delta in deltas {
        match delta {
            Delta::Start { index, kind } => {
                let kind = match kind {
                    Kind::ToolCall { name } => format!("call {name}"),
                    other => format!("{other:?}"),
                };
                found.insert(*index, (kind, String::new()));
            }
            Delta::Text { index, text } => {
                if let Some((_, said)) = found.get_mut(index) {
                    said.push_str(text);
                }
            }
            _ => {}
        }
    }
    found.into_values().collect()
}

/// 收了几块（`End`）。
fn ended(deltas: &[Delta]) -> usize {
    deltas
        .iter()
        .filter(|delta| matches!(delta, Delta::End { .. }))
        .count()
}

/// 两次调用：`read` 读 a.txt、`shell` 跑 ls。
fn two_calls() -> Vec<(String, String)> {
    vec![
        ("call read".to_string(), r#"{"path":"a.txt"}"#.to_string()),
        ("call shell".to_string(), r#"{"command":"ls"}"#.to_string()),
    ]
}

/// Anthropic：两块都写 `index: 0`（一块收了再开下一块），靠 `id` 分。
#[test]
fn parallel_calls_on_index_zero_stay_apart_in_anthropic() {
    let stream = concat!(
        "event: content_block_start\n",
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_a\",\"name\":\"read\",\"input\":{}}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\\\"a.txt\\\"}\"}}\n\n",
        "event: content_block_stop\n",
        "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: content_block_start\n",
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_b\",\"name\":\"shell\",\"input\":{}}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"command\\\":\\\"ls\\\"}\"}}\n\n",
        "event: content_block_stop\n",
        "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: message_delta\n",
        "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":9}}\n\n",
        "event: message_stop\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    );
    let mut decoder = anthropic::Decoder::new();
    let mut deltas = decoder.feed(stream.as_bytes());
    deltas.extend(decoder.finish().deltas);
    assert_eq!(blocks(&deltas), two_calls());
    assert_eq!(ended(&deltas), 2, "两块都收了");
}

/// OpenAI Responses：两项都写 `output_index: 0`（一项完了再加下一项），靠项的 `id` 分。
#[test]
fn parallel_calls_on_index_zero_stay_apart_in_openai_responses() {
    let stream = concat!(
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"id\":\"fc_a\",\"type\":\"function_call\",\"arguments\":\"\",\"call_id\":\"call_a\",\"name\":\"read\"}}\n\n",
        "event: response.function_call_arguments.delta\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"item_id\":\"fc_a\",\"output_index\":0,\"delta\":\"{\\\"path\\\":\\\"a.txt\\\"}\"}\n\n",
        "event: response.output_item.done\n",
        "data: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"id\":\"fc_a\",\"type\":\"function_call\",\"arguments\":\"{\\\"path\\\":\\\"a.txt\\\"}\",\"call_id\":\"call_a\",\"name\":\"read\"}}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"id\":\"fc_b\",\"type\":\"function_call\",\"arguments\":\"\",\"call_id\":\"call_b\",\"name\":\"shell\"}}\n\n",
        "event: response.function_call_arguments.delta\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"item_id\":\"fc_b\",\"output_index\":0,\"delta\":\"{\\\"command\\\":\\\"ls\\\"}\"}\n\n",
        "event: response.output_item.done\n",
        "data: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"id\":\"fc_b\",\"type\":\"function_call\",\"arguments\":\"{\\\"command\\\":\\\"ls\\\"}\",\"call_id\":\"call_b\",\"name\":\"shell\"}}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":3,\"output_tokens\":9}}}\n\n",
    );
    let mut decoder = openai_responses::Decoder::new();
    let mut deltas = decoder.feed(stream.as_bytes());
    deltas.extend(decoder.finish().deltas);
    assert_eq!(blocks(&deltas), two_calls());
    assert_eq!(ended(&deltas), 2, "两块都收了");
}
