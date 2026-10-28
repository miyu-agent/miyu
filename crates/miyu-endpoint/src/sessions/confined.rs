//! 会话关不关在沙盒里（施工 5-12；施工 SU-4 中起照 `session.created` 记的 `confined`）：造的时候算、之后照日志第一条读。

use miyu_kernel::event::Body;
use miyu_kernel::id::{AccountId, SessionId, VenueId};
use miyu_store::log::first_event;

use crate::Core;

/// 属主是 `owner` 的会话关不关起来（施工 5-12；SU-4 上起都照这一处算）：平台桥名下的（群、陌生人的私聊）关起来，主人本人的不关。
pub(crate) fn confined(core: &Core, owner: &AccountId) -> bool {
    *owner != core.admin
}

/// 会话 `id`（属主 `owner`）关没关着（施工 SU-4 中）：照日志第一条 `session.created` 的 `confined`；以前的日志没这一格，属主不是
/// 主人的算关着。读不了的算关着。
pub(crate) async fn confined_of(core: &Core, owner: &AccountId, id: &SessionId) -> bool {
    placed(core, owner, id)
        .await
        .is_none_or(|(_, confined)| confined)
}

/// 会话 `id`（属主 `owner`）的场所、关没关着（同 [`confined_of`]）：照日志第一条读，读不了的没有。
pub(crate) async fn placed(
    core: &Core,
    owner: &AccountId,
    id: &SessionId,
) -> Option<(VenueId, bool)> {
    let dir = core.root.session_dir(owner, id);
    let first = tokio::task::spawn_blocking(move || first_event(&dir)).await;
    match first {
        Ok(Ok(event)) => match event.body {
            Body::SessionCreated(created) => {
                let shut = created.confined || confined(core, &created.owner);
                Some((created.venue, shut))
            }
            _ => None,
        },
        _ => None,
    }
}
