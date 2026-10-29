//! 会话关不关在沙盒里（施工 5-12；施工 SU-4 中起照 `session.created` 记的 `confined`）：造的时候算、之后照日志第一条读。

use miyu_kernel::event::Body;
use miyu_kernel::id::{SessionId, VenueId};
use miyu_store::log::first_event;

use crate::Core;

/// 会话 `id` 关没关着（施工 SU-4 中）：照日志第一条 `session.created` 的 `confined`。读不了的算关着。
pub(crate) async fn confined_of(core: &Core, id: &SessionId) -> bool {
    placed(core, id).await.is_none_or(|(_, confined)| confined)
}

/// 会话 `id` 的场所、关没关着（同 [`confined_of`]）：照日志第一条读，读不了的没有。
pub(crate) async fn placed(core: &Core, id: &SessionId) -> Option<(VenueId, bool)> {
    let dir = core.root.session_dir(id);
    let first = tokio::task::spawn_blocking(move || first_event(&dir)).await;
    match first {
        Ok(Ok(event)) => match event.body {
            Body::SessionCreated(created) => {
                let shut = created.confined;
                Some((created.venue, shut))
            }
            _ => None,
        },
        _ => None,
    }
}
