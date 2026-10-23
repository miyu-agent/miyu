//! 哪次 `shell` 下面有脚本里调的在等人确认（施工 M-7 上）：照记进日志的事件算，交回要告诉哪次 `shell` 在等、等完了。一个
//! 父级同时只问一件，照调用编号记那一件的序号；同一个父级下面早先派出去的别的那件跑完了，不算等完。

use std::collections::BTreeMap;

use miyu_kernel::event::Body;
use miyu_kernel::id::{CallId, Parent};

/// 在等人的那几次 `shell`：调用编号到在问的那一件的序号。
#[derive(Debug, Default)]
pub(super) struct Asking(BTreeMap<CallId, u32>);

impl Asking {
    /// 看一条事件：带序号的请求是那次 `shell` 开始等人（`true`）；那一件的决定、了结，或者那次 `shell` 的结果，是等完了
    /// （`false`）。别的、没变的交回没有。
    pub(super) fn note(&mut self, body: &Body) -> Option<(CallId, bool)> {
        let (call_id, n, waiting) = match body {
            Body::ApprovalRequested(requested) => {
                (requested.call_id, Some(requested.nested?), true)
            }
            Body::ApprovalDecided(decided) => (decided.call_id, Some(decided.nested?), false),
            Body::ToolNested(nested) => match nested.id.parent() {
                Parent::Call(call_id) => (*call_id, Some(nested.id.n()), false),
                Parent::Job(_) => return None,
            },
            Body::ToolResult(result) => (result.call_id, None, false),
            _ => return None,
        };
        let changed = match (waiting, n) {
            (true, Some(n)) => self.0.insert(call_id, n).is_none(),
            _ if n.is_none_or(|n| self.0.get(&call_id) == Some(&n)) => {
                self.0.remove(&call_id).is_some()
            }
            _ => false,
        };
        changed.then_some((call_id, waiting))
    }
}

#[cfg(test)]
mod tests;
