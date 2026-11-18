//! `/context`（施工 CTX-2，设计 68 第四节「命令」）：人设、去掉、看这个会话的上下文上限。
//!
//! - `/context <数值>`：设上限。数值收纯数字，也收 `128k`、`1m`（`k` 是千、`m` 是百万，大小写都行）；同 `session.configure`
//!   的 `context` 照压缩线查过（`models/context.rs`），太小的拒 `bad_params`，`data.min` 是最少要多少。
//! - `/context unset`（`off` 也认）：去掉，照模型的窗口。
//! - `/context`：不改，说这一刻的上限和压缩线。终端、网页拦下不带参数的照旧开自己的明细（设计 68 第二节第 4 条），这里是
//!   通讯平台这些没有明细的头用的。
//!
//! 回执照设好以后会话给头看的限额写（下一次请求起生效，交给内核时已经套上了），数照 [`about`] 写成「约 128k」。别的写法
//! `bad_params`。谁能用同 `/clear`。

use std::sync::Arc;

use miyu_kernel::session::ContextLimits;

use super::registry::{Call, Commands, Ran, Running, Slash};
use super::{Said, accepted, command};
use crate::models;
use crate::refusal::Refusal;

/// 登记 `/context`。
pub(super) fn register(commands: &mut Commands) {
    commands.register(Arc::new(ContextCommand));
}

/// `/context`。
struct ContextCommand;

impl Slash for ContextCommand {
    fn name(&self) -> &'static str {
        "context"
    }

    fn summary(&self) -> &'static str {
        "commands/summary/context"
    }

    fn argument(&self) -> Option<&'static str> {
        Some("commands/argument/context")
    }

    fn usage(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("", "commands/usage/context/show"),
            ("commands/placeholder/context", "commands/usage/context/set"),
            ("unset", "commands/usage/context/off"),
        ]
    }

    fn run<'a>(&'a self, call: Call<'a>) -> Running<'a> {
        Box::pin(async move {
            let handle = call.handle();
            if call.rest.is_empty() {
                return Ok(Ran {
                    events: Vec::new(),
                    said: said(None, &handle.limits()),
                });
            }
            let tokens = parsed(call.rest)?;
            let set = models::context::command(handle, tokens).await?;
            let outcome = command(call.core, call.session, handle, call.id, call.by, set).await?;
            Ok(Ran {
                events: accepted(outcome)?,
                said: said(Some(tokens), &handle.limits()),
            })
        })
    }
}

/// 命令后面跟的字：`unset`、`off` 是去掉（`None`，`unset` 是项目主人 11-10 加的），别的是数值。大小写都行。
///
/// # Errors
///
/// 不是 `unset`、`off`、纯数字、数字接 `k`、`m` 的，或者大得装不下：`bad_params`。
pub(super) fn parsed(rest: &str) -> Result<Option<u64>, Refusal> {
    let rest = rest.to_ascii_lowercase();
    if rest == "unset" || rest == "off" {
        return Ok(None);
    }
    let (digits, unit) = match rest.strip_suffix('k') {
        Some(digits) => (digits, 1_000),
        None => match rest.strip_suffix('m') {
            Some(digits) => (digits, 1_000_000),
            None => (rest.as_str(), 1),
        },
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Refusal::BAD_PARAMS);
    }
    digits
        .parse::<u64>()
        .ok()
        .and_then(|number| number.checked_mul(unit))
        .map(Some)
        .ok_or(Refusal::BAD_PARAMS)
}

/// 回执：`set` 是这一次设的（外层没有的是只看，里面没有的是去掉），`limits` 是这一刻会话给头看的限额。压缩线算不出的不说它。
fn said(set: Option<Option<u64>>, limits: &ContextLimits) -> Said {
    let context = |key, tokens| Said::with(key, vec![("context", about(tokens))]);
    let first = match (set, limits.context) {
        (Some(None), _) => Said::plain("commands/context-off"),
        (Some(Some(tokens)), _) => context("commands/context-set", tokens),
        (None, Some(tokens)) => context("commands/context-is", tokens),
        (None, None) => Said::plain("commands/context-none"),
    };
    let mut parts = vec![first];
    if let Some(line) = limits.compaction_line {
        parts.push(Said::with(
            "commands/context-line",
            vec![("line", about(line))],
        ));
    }
    Said {
        key: "commands/context-said",
        fields: Vec::new(),
        lists: vec![("parts", parts)],
    }
}

/// 给人看的约数，往下取：不到一千的照写，不到一百万的写成多少 `k`，再大的写成多少 `m`（留一位小数，`.0` 不写）。
pub(super) fn about(tokens: u64) -> String {
    if tokens < 1_000 {
        return tokens.to_string();
    }
    if tokens < 1_000_000 {
        return format!("{}k", tokens / 1_000);
    }
    let tenths = tokens / 100_000;
    match tenths % 10 {
        0 => format!("{}m", tenths / 10),
        digit => format!("{}.{digit}m", tenths / 10),
    }
}

#[cfg(test)]
mod tests;
