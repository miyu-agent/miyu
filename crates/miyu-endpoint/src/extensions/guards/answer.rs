//! 一个守卫答了什么（施工 HK-1，设计 65 第三节第 3、5 条）：`tool.guard` 的回应 `{"verdict": "allow" | "deny" | "ask",
//! "reason"?}` 读成 [`Answer`]；没答的、答不了的也是一种答法，合的时候都算拒绝。

use serde::Deserialize;
use serde_json::Value;

/// 一个守卫的答。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Answer {
    /// 放行。
    Allow,
    /// 要问人，带着它写的理由（原文，还没截）。
    Ask(Option<String>),
    /// 拒绝，带着它写的理由（原文，还没截）。
    Deny(Option<String>),
    /// 没在时限里答。
    TimedOut,
    /// 开着却没在跑（正在起、刚崩、起不来），或者问着问着连接断了。
    Absent,
    /// 回了 JSON-RPC 错误。
    Failed,
    /// 回应的写法不对：多了格、少了格、`verdict` 不认识、`reason` 不是字。
    Malformed,
}

impl Answer {
    /// 合的时候算不算拒绝：拒绝的，和没答、答不了的（失效往严里走，第五条）。
    pub(super) fn denies(&self) -> bool {
        !matches!(self, Answer::Allow | Answer::Ask(_))
    }

    /// 没答、答不了的为什么，记进运行日志；答了的没有。
    pub(super) fn failure(&self) -> Option<&'static str> {
        match self {
            Answer::TimedOut => Some("timed_out"),
            Answer::Absent => Some("not_running"),
            Answer::Failed => Some("failed"),
            Answer::Malformed => Some("malformed"),
            Answer::Allow | Answer::Ask(_) | Answer::Deny(_) => None,
        }
    }
}

/// 回应的写法：只认这两格。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    verdict: String,
    #[serde(default)]
    reason: Option<String>,
}

/// 照写法读回应：不对的是 [`Answer::Malformed`]。
pub(super) fn read(result: Value) -> Answer {
    let Ok(reply) = serde_json::from_value::<Reply>(result) else {
        return Answer::Malformed;
    };
    match reply.verdict.as_str() {
        "allow" => Answer::Allow,
        "ask" => Answer::Ask(reply.reason),
        "deny" => Answer::Deny(reply.reason),
        _ => Answer::Malformed,
    }
}
