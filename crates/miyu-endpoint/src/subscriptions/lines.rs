//! 事件流写出去的几种通知（施工 X-8 补从 `subscriptions.rs` 挪出来，那边到了行数上限）：落了盘的事件、瞬时事件、长期目标变了，
//! 和 `resync`。

use serde_json::json;

use miyu_kernel::id::SessionId;
use miyu_kernel::time::Timestamp;
use miyu_session::{Goal, Pushed};

use crate::Core;

/// 一份推送写成 `event` 通知：一条事件一个，落了盘的、瞬时的都照 `03-事件模型.md` 的写法原样放进去；长期目标变了的写成瞬时
/// 事件 `goal.changed`（施工 X-8 补，[`goal_changed`]）。
/// 金额照会话属主 `owner` 的币种排。
pub(super) fn lines(core: &Core, session: &SessionId, pushed: &Pushed) -> Vec<String> {
    match pushed {
        Pushed::Events(events) => events
            .iter()
            .map(|event| notification(session, &event.to_line()))
            .collect(),
        Pushed::Transient(transient) => vec![notification(session, &transient.to_line())],
        Pushed::Goal { at, goal } => {
            vec![notification(
                session,
                &goal_changed(core, *at, goal.as_ref()),
            )]
        }
    }
}

/// `goal.changed`（施工 X-8 补，`protocol.md`「订阅」）：瞬时事件的写法，`by` 是长期目标这个模块，`body.goal` 同会话状态的
/// `goal`（`view/status.rs`），没有目标的是 `null`。
fn goal_changed(core: &Core, at: Timestamp, goal: Option<&Goal>) -> String {
    let goal = goal.map(|goal| crate::view::status::goal_of(core, goal));
    json!({
        "at": at,
        "kind": "goal.changed",
        "by": {"kind": "module", "id": "goal"},
        "body": {"goal": goal},
    })
    .to_string()
}

/// `event` 通知。事件已经是一行紧凑的 JSON，原样嵌进去，不再解一遍、写一遍。
pub(super) fn notification(session: &SessionId, event: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","method":"event","params":{{"session":"{}","event":{event}}}}}"#,
        session.as_str()
    )
}

/// `resync` 通知：这个订阅掉了队，或者会话停了，这个订阅停了。
pub(super) fn resync(session: &SessionId) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","method":"resync","params":{{"session":"{}","stream":"events"}}}}"#,
        session.as_str()
    )
}
