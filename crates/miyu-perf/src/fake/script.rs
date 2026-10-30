//! 照剧本回工具调用（施工 V-29「工具一步」）：提到记号的请求，照请求体里已经有几次调用的结果，回下一次调用；都回完了回
//! 一句话收尾。一轮里一次接一次，像真模型一步步干活。
//!
//! 认「已经有几次的结果」只看调用编号：驱动把结果交回去时用的是供应商给的编号（`drivers/openai-chat.md`「tool」第 6 条），
//! 编号只在这一份剧本里出现，从头数到第一个请求体里没有的就是几次。不看请求体的别的写法：Miyu、别家都能这样认。

use serde_json::json;

use super::{mentions, tokens};

/// 一次工具调用：供应商给的编号、工具名、参数（JSON 写成字）。
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    /// 调用编号：整个剧本里不重复，也不会是请求体里本来就有的字。
    pub id: String,
    /// 工具名，照被量的那一家给模型的工具表。
    pub name: String,
    /// 参数，JSON 写成的字。
    pub arguments: String,
}

/// 一轮的剧本：提到 `marker` 的请求照 `calls` 一次次回。
#[derive(Debug, Clone)]
pub struct Script {
    /// 这一轮人说的话里的记号。
    pub marker: String,
    /// 照先后回的调用。
    pub calls: Vec<ToolCall>,
}

/// 调用前面的一句正文：「什么都不做」的对照回的就是这一句、不带调用（量尺照它比出核心收一轮的底子）。
pub const PREAMBLE: &str = "Checking.";

impl Script {
    /// 请求体 `body` 里已经有前几次调用的结果：从头数到第一个没出现的编号。不提记号的请求不归这份剧本，是 `None`。
    pub fn answered(&self, body: &[u8]) -> Option<usize> {
        if !mentions(body, self.marker.as_bytes()) {
            return None;
        }
        Some(
            self.calls
                .iter()
                .take_while(|call| mentions(body, call.id.as_bytes()))
                .count(),
        )
    }

    /// 回请求体 `body` 的事件流：还有没回的调用就回下一次，回完了收尾一句；用量照字节数估。
    pub fn answer(&self, body: &[u8], done: usize) -> String {
        let prompt = tokens(body.len());
        match self.calls.get(done) {
            Some(call) => tool_stream(PREAMBLE, call, prompt),
            None => super::stream_pieces(PREAMBLE, 1, prompt, tokens(PREAMBLE.len())),
        }
    }
}

/// 一次带工具调用的回答：正文 `text` 一块，调用一块（编号、名字、整份参数一次给全），收尾一块 `finish_reason` 是
/// `tool_calls`、带用量，最后 `[DONE]`。
pub fn tool_stream(text: &str, call: &ToolCall, prompt: usize) -> String {
    let said = json!({"choices": [{"index": 0, "delta": {"role": "assistant", "content": text},
        "finish_reason": null}]});
    let called = json!({"choices": [{"index": 0, "delta": {"tool_calls": [{"index": 0,
        "id": call.id, "type": "function",
        "function": {"name": call.name, "arguments": call.arguments}}]},
        "finish_reason": null}]});
    let completion = tokens(text.len() + call.arguments.len());
    let done = json!({"choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls"}],
        "usage": {"prompt_tokens": prompt, "completion_tokens": completion,
            "total_tokens": prompt + completion}});
    format!("data: {said}\n\ndata: {called}\n\ndata: {done}\n\ndata: [DONE]\n\n")
}
