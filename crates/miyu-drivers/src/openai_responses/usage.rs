//! 用量归成内核的四项（`docs/blueprint/drivers/openai-responses.md`「用量」）：取收尾那个事件的 `response.usage`。
//! `input_tokens` 含命中的，没命中的是减出来的；这一家不报写入，写入是 0；输出含思考，思考占了多少照
//! `output_tokens_details.reasoning_tokens`（施工 2-3 再补，0 的当没报）。供应商那边替模型跑的搜索、抓取的次数照
//! `server_tool_use.tool_search`、`server_tool_use.tool_fetch`（施工 DSR-4，DeepSeek 报；是次数，不是 token；0 的当没报）。

use miyu_kernel::event::Usage;
use serde_json::Value;

/// 一份用量。没有 `input_tokens` 的不算用量。
pub(super) fn usage(value: &Value) -> Option<Usage> {
    let number = |pointer: &str| value.pointer(pointer).and_then(Value::as_u64);
    // 报了、不是 0 的才记：0 和没报一样，日志里不多写一格。
    let reported = |pointer: &str| number(pointer).filter(|count| *count > 0);
    let input = number("/input_tokens")?;
    let cache_read = number("/input_tokens_details/cached_tokens").unwrap_or(0);
    Some(Usage {
        uncached: input.saturating_sub(cache_read),
        cache_read,
        cache_write: 0,
        output: number("/output_tokens").unwrap_or(0),
        reasoning: reported("/output_tokens_details/reasoning_tokens"),
        searches: reported("/server_tool_use/tool_search"),
        fetches: reported("/server_tool_use/tool_fetch"),
    })
}
