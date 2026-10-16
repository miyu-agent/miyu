//! 从日志里认出来的几样（施工 3-9 六补从 `load.rs` 挪来：那一份到了行数上限）：现在的快照、最近一次发给了谁。

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::ContentHash;
use miyu_kernel::origin::Model;

/// 现在的快照（施工 P-1 再补）：整份日志里最近一条带 `policy` 的 `session.policy_changed`，撤掉的回合里的也算（换快照不是
/// 对话的一部分）；没换过的没有，照 `session.created` 的。
pub(crate) fn current_policy(events: &[Event]) -> Option<&ContentHash> {
    events.iter().rev().find_map(|event| match &event.body {
        Body::PolicyChanged(changed) => changed.policy.as_ref(),
        _ => None,
    })
}

/// 最近一条发出去了的 `model.called` 发给了谁（施工 8-8）：钉住的池载入时照它认钉着的成员（「起草时定的」第 2 条）。
pub(super) fn last_sent(events: &[Event]) -> Option<Model> {
    events.iter().rev().find_map(|event| match &event.body {
        Body::ModelCalled(called) => Some(Model {
            endpoint: called.endpoint.clone()?,
            model: called.model.clone()?,
        }),
        _ => None,
    })
}
