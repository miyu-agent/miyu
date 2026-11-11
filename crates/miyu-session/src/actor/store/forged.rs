//! 截了模型编的下一轮，记一行运行日志（施工 FT-1，`docs/blueprint/session/actor.md`「运行日志」）。截是内核在写回复之前做的
//! （`docs/blueprint/kernel/session.md`「截掉模型编的下一轮」），内核不记运行日志，这里照它记在 `model.called` 上的那一格记。

use miyu_kernel::event::{Body, Event};

use crate::TARGET;

/// 这一批要落盘的事件里，`model.called` 记着截过回复的，一处一行 `WARN forged turn cut`：哪一次请求、发给了谁、流里的第几块、
/// 那个角色词、截掉多少字节。会话编号在 span 上。截掉的原文不记：内核没留，运行日志也不进正文（`log.md`）。
pub(super) fn note(events: &[Event]) {
    for event in events {
        let Body::ModelCalled(called) = &event.body else {
            continue;
        };
        for cut in &called.cut {
            tracing::warn!(
                target: TARGET,
                seen = called.seen.get(),
                endpoint = called.endpoint.as_ref().map(|endpoint| endpoint.as_str()),
                model = called.model.as_ref().map(|model| model.as_str()),
                block = cut.block,
                role = cut.role.as_str(),
                bytes = cut.bytes,
                "forged turn cut"
            );
        }
    }
}
