//! 只解要的种类（施工 V-39 上补，`docs/construction/V-39上补-判官只解析要的种类.md`）：判官读群会话日志时用
//! （[`Event::from_line_only`](crate::event::Event::from_line_only)）。外壳照旧全解（序号、时刻、种类、回合、谁、命令编号），`body` 分三样：
//!
//! - 调的一方要的种类，和有效历史照种类认的那几种（[`HISTORY`]）：全解，同 [`Event::from_line`](crate::event::Event::from_line)。
//! - `message.assistant`、`model.called`（[`SEEN`]）：有效历史撤销时查一轮里排着的话，只看它们的 `seen`、请求是不是暂停着
//!   没发出去（`error`）：只解这几格（`model.called` 另解 `messages`、`result`，都很小），别的格空着。
//! - 别的：`body` 只过一遍（是合法的 JSON 就行），读成 [`Body::Unknown`]，种类照写，`body` 是 `null`。
//!
//! 读不成的照 [`Event::from_line`](crate::event::Event::from_line) 再读一遍，报它的错（读成了的交它读成的）：报出来的错和全解的一字不差。全解读得成的，这里
//! 也读得成；不要的种类的 `body` 还是合法 JSON、只是不合那一种的写法的，全解报错，这里不报（2026-11-03 项目主人拍板接受）。
//!
//! 一行行交给留着一切的有效历史（[`crate::history::History::whole`]），撤销、恢复、撤回、压缩以后留下的事件（序号、先后、
//! 认的种类的内容）和全解的一样：有效历史照种类认的都全解了，查排着的话看的那几格都在。有效历史的规矩（`history.rs` 的
//! `append`、`history/undo.rs`）多认了别的种类、多看了别的格，这里要跟着改。

use serde::Deserialize;

use super::{Body, CallError, CallResult, MessageAssistant, ModelCalled};
use crate::id::{EventKind, Seq};
use crate::raw::RawJson;

/// 有效历史照种类认的（`history.rs` 的 `append`，`history/undo.rs` 的 `taken_over`、`left_queued`、`ended_before`、
/// `said_by_the_person`）：撤销、恢复、撤回、压缩、回合的开头和结尾、人话，和撤销认上一轮时看触发是哪一种的回报、子会话
/// 回报、等的会话空了。只解要的种类时这几种也全解。
pub(super) const HISTORY: &[&str] = &[
    "turn.started",
    "turn.ended",
    "turn.reverted",
    "turn.unreverted",
    "message.user",
    "message.withdrawn",
    "context.compacted",
    "job.reported",
    "child.reported",
    "peer.idle",
];

/// 有效历史撤销时只看几格的（`history/undo.rs` 的 `left_queued`）：只解要的种类时，不要的这几种只解这几格。
pub(super) const SEEN: &[&str] = &["message.assistant", "model.called"];

/// `body` 解多深。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Depth {
    /// 全解。
    Full,
    /// 只解有效历史看的那几格（[`SEEN`]）。
    Seen,
    /// 不解，只过一遍。
    Skip,
}

/// 种类 `kind` 照 `only` 解多深；`only` 是空的（不是只解要的种类）全解。
pub(super) fn depth(only: Option<&[&str]>, kind: &EventKind) -> Depth {
    let Some(only) = only else {
        return Depth::Full;
    };
    let kind = kind.as_str();
    if only.contains(&kind) || HISTORY.contains(&kind) {
        Depth::Full
    } else if SEEN.contains(&kind) {
        Depth::Seen
    } else {
        Depth::Skip
    }
}

/// 不解的种类读成的：种类照写，`body` 是 `null`。
pub(super) fn skipped(kind: EventKind) -> Body {
    Body::Unknown {
        kind,
        body: RawJson::null(),
    }
}

/// `message.assistant` 只解的几格。
#[derive(Deserialize)]
struct Replied {
    seen: Seq,
    #[serde(default)]
    interrupted: bool,
}

/// `model.called` 只解的几格。
#[derive(Deserialize)]
struct Called {
    seen: Seq,
    messages: u64,
    result: CallResult,
    #[serde(default)]
    error: Option<CallError>,
}

/// 照种类只解几格（[`Depth::Seen`]）：别的格空着。
pub(super) fn seen<'de, D: serde::Deserializer<'de>>(
    kind: &EventKind,
    d: D,
) -> Result<Body, D::Error> {
    if kind.as_str() == "message.assistant" {
        let Replied { seen, interrupted } = Replied::deserialize(d)?;
        return Ok(Body::MessageAssistant(MessageAssistant {
            blocks: Vec::new(),
            seen,
            interrupted,
            indexes: Vec::new(),
        }));
    }
    let Called {
        seen,
        messages,
        result,
        error,
    } = Called::deserialize(d)?;
    Ok(Body::ModelCalled(ModelCalled {
        seen,
        endpoint: None,
        model: None,
        request: None,
        messages,
        first_difference: None,
        usage: None,
        cost: None,
        first_token_ms: None,
        duration_ms: None,
        blocks: None,
        result,
        error,
        compaction: None,
        purpose: None,
        truncated: false,
        fell_back: false,
    }))
}

#[cfg(test)]
mod tests;
